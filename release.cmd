#!/bin/sh

rm -r "target/macos"
mkdir -p "target/macos/Shove it!.app/Contents/MacOS"
mkdir -p "target/macos/Shove it!.app/Contents/Resources"
cp -R "assets" "target/macos/Shove it!.app/Contents/MacOS/"
cp "Info.plist" "target/macos/Shove it!.app/Contents/"
cp "bevy.icns" "target/macos/Shove it!.app/Contents/Resources/"

cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
lipo "target/x86_64-apple-darwin/release/shoveit" \
     "target/aarch64-apple-darwin/release/shoveit" \
     -create -output "target/macos/Shove it!.app/Contents/MacOS/shoveit"

rm -r "target/Shove it!.dmg"
create-dmg \
  --volname "Shove it!" \
  --volicon "bevy.icns" \
  --window-size 800 400 \
  --icon-size 128 \
  --icon "Shove it!.app" 200 200 \
  --hide-extension "Shove it!.app" \
  --app-drop-link 600 200 \
  --no-internet-enable \
  "target/Shove it!.dmg" \
  "target/macos/"
