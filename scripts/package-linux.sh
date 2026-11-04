#!/usr/bin/env bash
# Build dist/Glint-<version>-linux-<arch>.AppImage and a .deb. Mirrors the `linux` job in
# .github/workflows/ci.yml.
# Needs (Ubuntu 24.04+): rustup, libmpv-dev, libxkbcommon-dev, libfuse2, curl, xz-utils.
# libmpv and its libraries come from the system and are bundled by linuxdeploy;
# ffmpeg is a static BtbN build, so the AppImage doesn't depend on the distro's.
# The .deb (Debian/Ubuntu) is the same binary installed system-wide, using the distro's libmpv and
# ffmpeg; it adds the app-menu entry and "Open with" support. Needs dpkg-deb.
# Usage: scripts/package-linux.sh [version]   (SKIP_TESTS=1 to skip cargo test)
set -euo pipefail
cd "$(dirname "$0")/.."
version=${1:-dev}
arch=$(uname -m)
case "$arch" in
  x86_64) ff=linux64 ;;
  aarch64) ff=linuxarm64 ;;
  *) echo "unsupported architecture: $arch" >&2; exit 1 ;;
esac

deps=target/linux-deps
mkdir -p "$deps" dist
if [ ! -x "$deps/ffmpeg" ]; then
  curl -fsSL "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-$ff-gpl.tar.xz" |
    tar -xJ -C "$deps" --strip-components=2 --wildcards '*/bin/ffmpeg' '*/bin/ffprobe'
fi
if [ ! -x "$deps/linuxdeploy" ]; then
  curl -fsSLo "$deps/linuxdeploy" "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$arch.AppImage"
  chmod +x "$deps/linuxdeploy"
fi

# The export tests call ffmpeg/ffprobe from PATH.
[ "${SKIP_TESTS:-}" = 1 ] || PATH="$PWD/$deps:$PATH" cargo test
cargo build --release

app=$deps/AppDir
rm -rf "$app"
mkdir -p "$app/usr/bin"
cp target/release/glint "$deps/ffmpeg" "$app/usr/bin/"
"$deps/ffmpeg" -v error -y -i assets/glint.png -vf scale=256:256 "$deps/glint.png"

# Lets linuxdeploy run without FUSE (containers, CI).
export APPIMAGE_EXTRACT_AND_RUN=1
(cd "$deps" && ./linuxdeploy --appdir AppDir -e AppDir/usr/bin/glint -d ../../assets/glint.desktop -i glint.png --output appimage)
mv "$deps"/Glint*.AppImage "dist/Glint-$version-linux-$arch.AppImage"

# .deb: Debian versions must start with a digit, so it always carries Cargo.toml's version.
pkgver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
case "$arch" in
  x86_64) debarch=amd64 ;;
  aarch64) debarch=arm64 ;;
esac
deb=$deps/deb
rm -rf "$deb"
install -Dm755 target/release/glint "$deb/usr/bin/glint"
install -Dm644 assets/glint.desktop "$deb/usr/share/applications/glint.desktop"
install -Dm644 "$deps/glint.png" "$deb/usr/share/icons/hicolor/256x256/apps/glint.png"
mkdir -p "$deb/DEBIAN"
cat >"$deb/DEBIAN/control" <<CONTROL
Package: glint
Version: $pkgver
Section: video
Priority: optional
Architecture: $debarch
Depends: libmpv2, ffmpeg, libgl1, libxkbcommon0
Maintainer: zgv2 <dev@bogdanbpeterson.com>
Installed-Size: $(du -sk "$deb/usr" | cut -f1)
Homepage: https://github.com/zgv2-dev/qpe
Description: Quick video trim, crop and strip
 Glint trims, crops, retimes and strips inactive stretches from videos,
 stream-copying when nothing needs re-encoding. Playback uses libmpv,
 export uses ffmpeg.
CONTROL
dpkg-deb --root-owner-group --build "$deb" "dist/Glint-$version-linux-$debarch.deb" >/dev/null
ls -lh dist/*.AppImage dist/*.deb
