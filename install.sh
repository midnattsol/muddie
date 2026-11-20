#!/bin/sh
# Installs Muddie for this user, in ~/.local: the program, its entry in the
# menu of applications, its icon and its description for software stores.
#
#     ./install.sh             builds it, then installs it
#     ./install.sh --no-build  installs what was last built
#     PREFIX=/usr/local sudo -E ./install.sh --no-build   for everyone
#
# ./uninstall.sh takes it all away again.
set -eu
cd "$(dirname "$0")"
id=io.github.midnattsol.Muddie
prefix=${PREFIX:-$HOME/.local}
share=$prefix/share

if [ "${1:-}" != --no-build ]; then
    # Without these the screen reader and the system's voice are left out
    # of the build with no word said, and Muddie is mute.
    missing=
    for lib in dbus-1 speech-dispatcher; do
        pkg-config --exists "$lib" 2>/dev/null || missing="$missing $lib"
    done
    if [ -n "$missing" ]; then
        echo "Missing the development files of:$missing" >&2
        echo "On Debian or Ubuntu: sudo apt install libdbus-1-dev libspeechd-dev pkg-config" >&2
        echo "On Fedora: sudo dnf install dbus-devel speech-dispatcher-devel pkgconf" >&2
        exit 1
    fi
    cargo build --release -p muddie
fi
[ -x target/release/muddie ] || { echo "Build Muddie first: cargo build --release -p muddie" >&2; exit 1; }

install -Dm755 target/release/muddie "$prefix/bin/muddie"
# With the program's whole path: the desktop leaves out an entry whose
# program it cannot find, and its PATH need not be the terminal's.
mkdir -p "$share/applications"
sed "s|^Exec=muddie|TryExec=$prefix/bin/muddie\nExec=$prefix/bin/muddie|" \
    assets/$id.desktop > "$share/applications/$id.desktop"
chmod 644 "$share/applications/$id.desktop"
install -Dm644 assets/$id.metainfo.xml "$share/metainfo/$id.metainfo.xml"
for icon in assets/icons/hicolor/*/apps/$id.png; do
    size=$(basename "$(dirname "$(dirname "$icon")")")
    install -Dm644 "$icon" "$share/icons/hicolor/$size/apps/$id.png"
done

# The desktop's lists of applications and of icons, brought up to date.
if command -v gtk-update-icon-cache >/dev/null; then
    gtk-update-icon-cache -q -f -t "$share/icons/hicolor" || true
fi
if command -v update-desktop-database >/dev/null; then
    update-desktop-database -q "$share/applications" || true
fi

case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *) echo "Note: $prefix/bin is not in your PATH; the menu will find Muddie, a terminal will not." ;;
esac
echo "Muddie is installed in $prefix."
