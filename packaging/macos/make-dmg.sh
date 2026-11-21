#!/bin/sh
# Makes Muddie.app from the program built for macOS, and a disk image to
# drag it from into Applications. Run on macOS, from the top of the source:
#     packaging/macos/make-dmg.sh VERSION OUTPUT.dmg
# It is signed ad hoc, not by a developer known to Apple: the first time,
# it is opened with Open in the menu of a right click.
set -eu
version=$1
output=$2
id=io.github.midnattsol.Muddie
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

app=$work/disk/Muddie.app
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/muddie "$app/Contents/MacOS/muddie"

# The icon, from the sizes there are, as macOS names them.
set=$work/Muddie.iconset
mkdir "$set"
icon() { cp "assets/icons/hicolor/${1}x${1}/apps/$id.png" "$set/$2.png"; }
icon 16 icon_16x16
icon 32 icon_16x16@2x
icon 32 icon_32x32
icon 64 icon_32x32@2x
icon 128 icon_128x128
icon 256 icon_128x128@2x
icon 256 icon_256x256
icon 512 icon_256x256@2x
icon 512 icon_512x512
iconutil -c icns "$set" -o "$app/Contents/Resources/Muddie.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Muddie</string>
    <key>CFBundleDisplayName</key><string>Muddie</string>
    <key>CFBundleIdentifier</key><string>$id</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleExecutable</key><string>muddie</string>
    <key>CFBundleIconFile</key><string>Muddie</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.role-playing-games</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$app"

# The disk image: the application, and where to drag it.
ln -s /Applications "$work/disk/Applications"
hdiutil create -volname Muddie -srcfolder "$work/disk" -ov -format UDZO "$output"
