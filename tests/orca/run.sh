#!/bin/bash
# Muddie with Orca, in a desktop of its own that nobody sees or hears: what
# the game says, what the client says, the keys and the dialogs are checked
# by what Orca says and puts on a braille display, as its debug log tells.
#
#     dbus-run-session -- tests/orca/run.sh target/release/muddie
#
# It needs Orca, Xvfb, xdotool, at-spi2-core and speech-dispatcher with its
# libao output. It touches nothing of the session it is run from: it has
# its own bus, display, runtime folder, configuration and home, and its
# speech is played to libao's null driver.
set -u
muddie=$(realpath "$1")
here=$(dirname "$(realpath "$0")")
# Its logs go to $ORCA_TEST_LOGS if given, and are kept there.
if [ "${ORCA_TEST_LOGS:-}" ]; then
    work=$ORCA_TEST_LOGS keep=1
    mkdir -p "$work"
else
    work=$(mktemp -d)
fi
# Sockets go in the runtime folder, and their paths may not be long.
runtime=$(mktemp -d /tmp/mo.XXXX)
chmod 700 "$runtime"
started=()
cleanup() {
    for pid in "${started[@]}"; do kill -9 "$pid" 2>/dev/null; done
    # What Orca started for itself: whatever runs in this runtime folder,
    # but this script.
    for proc in /proc/[0-9]*; do
        [ "${proc#/proc/}" = "$$" ] && continue
        grep -qzx "XDG_RUNTIME_DIR=$runtime" "$proc/environ" 2>/dev/null \
            && kill -9 "${proc#/proc/}" 2>/dev/null
    done
    rm -rf "$runtime"
    [ "${keep:-}" ] || rm -rf "$work"
}
trap cleanup EXIT

export XDG_RUNTIME_DIR=$runtime XDG_CONFIG_HOME=$work/config
export XDG_DATA_HOME=$work/data XDG_CACHE_HOME=$work/cache HOME=$work/home
export GSETTINGS_BACKEND=memory PULSE_SERVER=unix:/nonexistent PIPEWIRE_REMOTE=/nonexistent
export DISPLAY=:77 XDG_SESSION_TYPE=x11 LANG=C.UTF-8 LANGUAGE=en
# No virtual file systems: they would be mounted for the session it runs in.
export GIO_USE_VFS=local GVFS_DISABLE_FUSE=1
unset WAYLAND_DISPLAY
mkdir -p "$XDG_CONFIG_HOME/speech-dispatcher" "$HOME"
printf 'default_driver=null\n' > "$HOME/.libao"
sed 's/^#\? *AudioOutputMethod .*/AudioOutputMethod "libao"/' \
    /etc/speech-dispatcher/speechd.conf > "$XDG_CONFIG_HOME/speech-dispatcher/speechd.conf"

Xvfb :77 -screen 0 1280x800x24 -nolisten tcp > "$work/xvfb.log" 2>&1 &
started+=($!)
sleep 1
/usr/libexec/at-spi-bus-launcher --launch-immediately > "$work/atspi.log" 2>&1 &
started+=($!)
sleep 1
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled '<true>' > /dev/null
/usr/bin/python3 -I "$here/mud.py" "$work" > "$work/mud.log" 2>&1 &
started+=($!)
for _ in $(seq 50); do [ -s "$work/mud.port" ] && break; sleep 0.1; done
port=$(cat "$work/mud.port")

failures=0
runs=0
mark=0

# What Orca presented since the last mark: said, and put on the display.
since() { tail -c +$((mark + 1)) "$log" 2>/dev/null | tr -d '\0'; }
remember() { mark=$(stat -c %s "$log"); }

# Waits up to ten seconds for a line of Orca's log since the mark.
expect() {
    local what=$1 pattern=$2
    for _ in $(seq 100); do
        if since | grep -aqF -- "$pattern"; then
            echo "ok: $what"
            return 0
        fi
        sleep 0.1
    done
    echo "FAILED: $what: no \"$pattern\" in what Orca was given"
    failures=$((failures + 1))
}

start_orca() {
    # Each run of Orca logs to a file of its own: all are kept.
    runs=$((runs + 1))
    log=$work/orca-$runs.log
    orca --debug-file "$log" > "$work/orca.out" 2>&1 &
    orca_pid=$!
    started+=($orca_pid)
    mark=0
    expect "Orca speaks" "SPEECH OUTPUT"
}

keys() {
    window=$(xdotool search --name Muddie | head -1)
    xdotool windowfocus --sync "$window" 2>/dev/null
    xdotool key --delay 150 "$@"
}

type_text() {
    xdotool type --delay 40 "$1"
}

mud() { printf '%s\n' "$@" >> "$work/mud.txt"; }

start_orca
remember
"$muddie" 127.0.0.1 "$port" > "$work/muddie.log" 2>&1 &
started+=($!)
expect "what the client says is presented" "PresentMessage called with: 'Connecting to 127.0.0.1:$port"
expect "connecting is said" "PresentMessage called with: 'Connected.'"
sleep 1

remember
mud "An orc arrives."
expect "a line of the game is said" "SPEECH OUTPUT: 'An orc arrives.'"
expect "a line of the game is on the braille display" "BRAILLE LINE:  'An orc arrives.'"

remember
keys alt+Up
expect "Alt+Up reads the last line again" "PresentMessage called with: 'An orc arrives.'"

remember
keys ctrl+k
sleep 1
type_text "zzzz"
expect "the palette says it found nothing" "PresentMessage called with: 'No matches.'"
keys Escape
sleep 1

remember
keys ctrl+k
sleep 1
type_text "settings general"
sleep 1
keys Return
expect "a dialog says its name" "SPEECH OUTPUT: 'Settings dialog"
expect "a dialog has the keyboard" "SPEECH OUTPUT: 'Close button'"
keys Escape
expect "closing a dialog gives the command line the keyboard" "SPEECH OUTPUT: 'Command entry"

# The editor: it opens on its section, a new rule has its name ready to
# be typed, and its cards are groups with their names.
remember
keys ctrl+e
expect "the editor opens on its section" "SPEECH OUTPUT: 'Triggers"
for _ in $(seq 9); do keys Tab; sleep 0.3; done
keys Return
expect "a new rule has its name ready" "SPEECH OUTPUT: 'Name entry"
for _ in $(seq 5); do keys Tab; sleep 0.3; done
# The card a control is in is named, as the braille display shows: Orca
# says ancestors aloud only once it knows where the keyboard was before,
# which a display with no window manager does not let it know.
expect "a card says its name" "When panel Contains page tab"
# On to the editor of what the rule sends, which says how to leave it:
# Tab types in it, Escape and then Tab go on to its height's grip.
for _ in $(seq 25); do
    since | grep -aqF "Tab inserts a tab" && break
    keys Tab
    sleep 0.3
done
expect "an editor of code says how to leave it" "Tab inserts a tab"
keys Escape Tab
expect "Escape and Tab go on to the editor's grip" "SPEECH OUTPUT: 'separator Editor height'"
expect "the grip says how tall the editor is" "SPEECH OUTPUT: '3 lines'"
remember
keys Down
expect "the grip says, once, how tall the editor has been made" "SPEECH OUTPUT: '4 lines'"
keys ctrl+e
sleep 1

# A screen reader started while Muddie runs is found, and said.
kill -9 "$orca_pid"
sleep 2
mud "Nobody hears this."
sleep 1
start_orca
expect "a screen reader that starts is said" "PresentMessage called with: 'Speech through Orca.'"
mud "A troll arrives."
expect "speech goes on through it" "SPEECH OUTPUT: 'A troll arrives.'"
if since | grep -aqF "Nobody hears this."; then
    echo "FAILED: what came while no screen reader ran was said late"
    failures=$((failures + 1))
fi

if [ "$failures" -gt 0 ]; then
    keep=1
    echo "$failures failed; the logs are in $work"
    exit 1
fi
echo "all passed"
