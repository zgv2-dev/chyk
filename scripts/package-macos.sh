#!/usr/bin/env bash
# Build dist/Chyk.app with libmpv and ffmpeg bundled, plus a drag-to-Applications DMG.
# Needs: brew install mpv dylibbundler (ffmpeg comes with mpv).
set -euo pipefail
version=${1:-dev}
arch=$(uname -m)
cargo build --release

app=dist/Chyk.app
rm -rf dist
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/libs"
cp target/release/chyk "$app/Contents/MacOS/"
cp -L "$(command -v ffmpeg)" "$app/Contents/MacOS/"
chmod u+w "$app/Contents/MacOS/"*

iconset=dist/chyk.iconset
mkdir -p "$iconset"
for s in 16 32 128 256 512; do
  sips -z $s $s assets/chyk.png --out "$iconset/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) assets/chyk.png --out "$iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/chyk.icns"
rm -rf "$iconset"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>C H Y K</string>
  <key>CFBundleDisplayName</key><string>C H Y K</string>
  <key>CFBundleIdentifier</key><string>app.chyk.Chyk</string>
  <key>CFBundleExecutable</key><string>chyk</string>
  <key>CFBundleIconFile</key><string>chyk</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version#v}</string>
  <key>CFBundleVersion</key><string>${version#v}</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# Copy every non-system dylib into Contents/libs and rewrite load paths.
dylibbundler -of -cd -b -x "$app/Contents/MacOS/chyk" -x "$app/Contents/MacOS/ffmpeg" \
  -d "$app/Contents/libs" -p @executable_path/../libs/ >/dev/null

# dylibbundler can add the same LC_RPATH twice, which dyld refuses to load.
for f in "$app"/Contents/libs/*; do
  for p in $(otool -l "$f" | awk '/LC_RPATH/ { getline; getline; print $2 }' | sort | uniq -d); do
    while [ "$(otool -l "$f" | grep -c "path $p ")" -gt 1 ]; do
      install_name_tool -delete_rpath "$p" "$f"
    done
  done
done

# Rewriting load paths breaks signatures; arm64 refuses to run unsigned code, so ad-hoc sign.
find "$app/Contents/libs" -type f -exec codesign --force -s - {} \;
codesign --force -s - "$app/Contents/MacOS/ffmpeg"
codesign --force -s - "$app"

# Drag-to-Applications DMG with the Phosphor background (design/dmg/build.py): build a writable
# image, let Finder store the window layout in it, then compress. Built-in tools only.
deps=target/macos-deps
stage=$deps/dmg
rw=$deps/chyk-rw.dmg
rm -rf "$stage" "$rw"
mkdir -p "$stage/.background"
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
tiffutil -cathidpicheck design/dmg/background.png design/dmg/background@2x.png \
  -out "$stage/.background/background.tiff" >/dev/null
[ -d /Volumes/Chyk ] && hdiutil detach -quiet /Volumes/Chyk || true
hdiutil create -quiet -volname Chyk -srcfolder "$stage" -fs HFS+ -format UDRW -ov "$rw"
device=$(hdiutil attach -readwrite -noverify -noautoopen "$rw" | awk '/Apple_HFS/ {print $1}')
# Window: 640x400 content (+28 pt title bar); icon centers match design/dmg/build.py. Hidden items
# go below the window: for anyone who shows hidden files they would otherwise take the top row and
# push the icons down.
# Without Finder automation (some CI setups) the DMG still builds, just unstyled.
osascript >/dev/null <<'APPLESCRIPT' || echo "warning: Finder layout failed; the DMG will use the default window" >&2
tell application "Finder"
  tell disk "Chyk"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set bounds of container window to {200, 120, 840, 548}
    set opts to icon view options of container window
    set arrangement of opts to not arranged
    set icon size of opts to 128
    set text size of opts to 13
    set background picture of opts to file ".background:background.tiff"
    set extension hidden of item "Chyk.app" to true
    set position of item "Chyk.app" of container window to {160, 186}
    set position of item "Applications" of container window to {480, 186}
    repeat with extra in {".background", ".fseventsd"}
      if exists item extra then set position of item extra of container window to {320, 640}
    end repeat
    update without registering applications
    delay 1
    close
  end tell
end tell
APPLESCRIPT
rm -rf /Volumes/Chyk/.fseventsd
sync
# Finder or Spotlight can hold the volume a moment longer: detach fails "Resource busy" (exit 16).
for _ in 1 2 3 4 5; do hdiutil detach -quiet "$device" && break; sleep 3; done
[ -d /Volumes/Chyk ] && hdiutil detach -force -quiet "$device"
hdiutil convert -quiet "$rw" -format UDZO -ov -o "dist/Chyk-macos-$arch.dmg"
rm -f "$rw"
ls -lh dist
