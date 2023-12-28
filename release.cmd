#!/bin/sh
set -e

bundle="target/macos/Shove it!.app"
package="target/Shove it!.dmg"
certificate="Developer ID Application: Thomas Castiglione (Q9GND772LL)"

########################
# create app structure #
########################
[ -e "target/macos" ] && rm -r "target/macos"
mkdir -p "$bundle/Contents/MacOS"
mkdir -p "$bundle/Contents/Resources"
cp -R "assets" "$bundle/Contents/MacOS/"
cp "AppInfo.plist" "$bundle/Contents/Info.plist"
cp "AppIcon.icns" "$bundle/Contents/Resources/"

############################
# compile universal binary #
############################
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
lipo "target/x86_64-apple-darwin/release/shoveit" \
     "target/aarch64-apple-darwin/release/shoveit" \
     -create -output "$bundle/Contents/MacOS/shoveit"

#################
# sign binaries #
#################
codesign --deep --force \
    --options=runtime \
    --entitlements AppEntitlements.entitlements \
    --sign "$certificate" \
    --timestamp \
    "$bundle"

##################
# create package #
##################
[ -e "$package" ] && rm -r "$package"
create-dmg \
  --volname "Shove it!" \
  --volicon "AppIcon.icns" \
  --window-size 800 400 \
  --icon-size 128 \
  --icon "Shove it!.app" 200 200 \
  --hide-extension "Shove it!.app" \
  --app-drop-link 600 200 \
  --no-internet-enable \
  "$package" \
  "target/macos/"

####################
# notarise package #
####################
codesign --deep --force \
    --options=runtime \
    --entitlements AppEntitlements.entitlements \
    --sign "$certificate" \
    --timestamp \
    "$package"

# must xcrun store-credentials first
xcrun notarytool submit \
    --keychain-profile notarytool \
    --wait \
    "$package"

xcrun stapler staple "$package"

mv "$package" "target/macos/"