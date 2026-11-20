#!/bin/sh
# Makes the application's icon in every size the desktop asks for, from
# icon.png, into icons/hicolor as the system keeps them. The tower is made
# to fill nine tenths of the height, as the icons beside it in a dock do;
# up to 32 pixels nearly all of it, or it is too small to be made out.
# Needs ImageMagick 7.
set -eu
cd "$(dirname "$0")"
id=io.github.midnattsol.Muddie
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
# The tower alone, then on a square canvas a little taller than it.
magick icon.png -trim +repage "$work/tower.png"
height=$(magick identify -format '%h' "$work/tower.png")
fit() {
    side=$(( height * 100 / $1 ))
    magick "$work/tower.png" -background none -gravity center \
        -extent "${side}x${side}" "$work/$2.png"
}
fit 90 large
fit 96 small
for size in 16 24 32 48 64 128 256 512; do
    dir=icons/hicolor/${size}x${size}/apps
    mkdir -p "$dir"
    from=$work/large.png
    [ "$size" -le 32 ] && from=$work/small.png
    magick "$from" -filter Lanczos -resize "${size}x${size}" -strip "PNG32:$dir/$id.png"
done
