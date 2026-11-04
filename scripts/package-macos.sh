#!/usr/bin/env bash
# Build dist/Glint.app with libmpv and ffmpeg bundled, plus a DMG.
# Needs: brew install mpv dylibbundler (ffmpeg comes with mpv).
set -euo pipefail
version=${1:-dev}
arch=$(uname -m)
cargo build --release

app=dist/Glint.app
rm -rf dist
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/libs"
cp target/release/glint "$app/Contents/MacOS/"
cp -L "$(command -v ffmpeg)" "$app/Contents/MacOS/"
chmod u+w "$app/Contents/MacOS/"*

iconset=dist/glint.iconset
mkdir -p "$iconset"
for s in 16 32 128 256 512; do
  sips -z $s $s assets/glint.png --out "$iconset/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) assets/glint.png --out "$iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/glint.icns"
rm -rf "$iconset"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>G L I N T</string>
  <key>CFBundleDisplayName</key><string>G L I N T</string>
  <key>CFBundleIdentifier</key><string>app.glint.Glint</string>
  <key>CFBundleExecutable</key><string>glint</string>
  <key>CFBundleIconFile</key><string>glint</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version#v}</string>
  <key>CFBundleVersion</key><string>${version#v}</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# Copy every non-system dylib into Contents/libs and rewrite load paths.
dylibbundler -of -cd -b -x "$app/Contents/MacOS/glint" -x "$app/Contents/MacOS/ffmpeg" \
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

hdiutil create -volname Glint -srcfolder "$app" -ov -format UDZO "dist/Glint-$version-macos-$arch.dmg" >/dev/null
ls -lh dist
