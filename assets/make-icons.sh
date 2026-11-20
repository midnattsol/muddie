#!/bin/sh
# Makes the application's icon in every size the desktop asks for, from
# icon.png, into icons/hicolor as the system keeps them. Up to 32 pixels
# the margin is trimmed, or the tower is too small to be made out.
# Needs ImageMagick 7.
set -eu
cd "$(dirname "$0")"
id=io.github.midnattsol.Muddie
tight=$(mktemp --suffix=.png)
trap 'rm -f "$tight"' EXIT
magick icon.png -trim +repage -background none -gravity center -extent 800x800 "$tight"
for size in 16 24 32 48 64 128 256 512; do
    dir=icons/hicolor/${size}x${size}/apps
    mkdir -p "$dir"
    from=icon.png
    [ "$size" -le 32 ] && from=$tight
    magick "$from" -filter Lanczos -resize "${size}x${size}" -strip "PNG32:$dir/$id.png"
done
