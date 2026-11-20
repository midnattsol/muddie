#!/bin/sh
# Takes away what ./install.sh put in place. The profiles, the settings and
# the maps are kept: they are in ~/.config/muddie and ~/.local/share/muddie.
#
#     ./uninstall.sh
#     PREFIX=/usr/local sudo -E ./uninstall.sh   if installed for everyone
set -eu
id=io.github.midnattsol.Muddie
prefix=${PREFIX:-$HOME/.local}
share=$prefix/share

rm -f "$prefix/bin/muddie" \
    "$share/applications/$id.desktop" \
    "$share/metainfo/$id.metainfo.xml" \
    "$share"/icons/hicolor/*/apps/$id.png

if command -v gtk-update-icon-cache >/dev/null; then
    gtk-update-icon-cache -q -f -t "$share/icons/hicolor" || true
fi
if command -v update-desktop-database >/dev/null; then
    update-desktop-database -q "$share/applications" || true
fi
echo "Muddie is uninstalled from $prefix; your profiles and maps are kept."
