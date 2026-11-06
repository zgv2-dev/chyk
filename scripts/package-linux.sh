#!/usr/bin/env bash
# Build dist/Chyk-<version>-linux-<arch>.AppImage, .deb and .rpm. Mirrors the `linux` job in
# .github/workflows/ci.yml.
# Needs (Ubuntu 24.04+): rustup, libmpv-dev, libxkbcommon-dev, libfuse2, curl, xz-utils.
# libmpv and its libraries come from the system and are bundled by linuxdeploy;
# ffmpeg is a static BtbN build, so the AppImage doesn't depend on the distro's.
# The .deb (Debian/Ubuntu) is the same binary installed system-wide, using the distro's libmpv and
# ffmpeg; it adds the app-menu entry and "Open with" support. Needs dpkg-deb.
# The .rpm (Fedora, openSUSE) installs to /usr/lib/chyk with the static ffmpeg beside the binary:
# Fedora's stock ffmpeg has no H.264/HEVC encoders. Needs rpmbuild (Ubuntu package `rpm`).
# Arch: packaging/arch/PKGBUILD repackages the release .deb for the AUR.
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
cp target/release/chyk "$deps/ffmpeg" "$app/usr/bin/"
"$deps/ffmpeg" -v error -y -i assets/chyk.png -vf scale=256:256 "$deps/chyk.png"

# Lets linuxdeploy run without FUSE (containers, CI).
export APPIMAGE_EXTRACT_AND_RUN=1
(cd "$deps" && ./linuxdeploy --appdir AppDir -e AppDir/usr/bin/chyk -d ../../assets/chyk.desktop -i chyk.png --output appimage)
mv "$deps"/Chyk*.AppImage "dist/Chyk-$version-linux-$arch.AppImage"

# .deb: Debian versions must start with a digit, so it always carries Cargo.toml's version.
pkgver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
case "$arch" in
  x86_64) debarch=amd64 ;;
  aarch64) debarch=arm64 ;;
esac
deb=$deps/deb
rm -rf "$deb"
install -Dm755 target/release/chyk "$deb/usr/bin/chyk"
install -Dm644 assets/chyk.desktop "$deb/usr/share/applications/chyk.desktop"
install -Dm644 "$deps/chyk.png" "$deb/usr/share/icons/hicolor/256x256/apps/chyk.png"
mkdir -p "$deb/DEBIAN"
cat >"$deb/DEBIAN/control" <<CONTROL
Package: chyk
Version: $pkgver
Section: video
Priority: optional
Architecture: $debarch
Depends: libmpv2, ffmpeg, libgl1, libxkbcommon0
Maintainer: zgv2 <dev@bogdanbpeterson.com>
Installed-Size: $(du -sk "$deb/usr" | cut -f1)
Homepage: https://github.com/zgv2-dev/chyk
Description: Quick video trim, crop and strip
 Chyk trims, crops, retimes and strips inactive stretches from videos,
 stream-copying when nothing needs re-encoding. Playback uses libmpv,
 export uses ffmpeg.
CONTROL
dpkg-deb --root-owner-group --build "$deb" "dist/Chyk-$version-linux-$debarch.deb" >/dev/null

# .rpm: /usr/bin/chyk links to /usr/lib/chyk/chyk, which finds its ffmpeg next to itself.
rpmtop=$PWD/$deps/rpm
stage=$rpmtop/stage
rm -rf "$rpmtop"
install -Dm755 target/release/chyk "$stage/usr/lib/chyk/chyk"
install -Dm755 "$deps/ffmpeg" "$stage/usr/lib/chyk/ffmpeg"
mkdir -p "$stage/usr/bin"
ln -s ../lib/chyk/chyk "$stage/usr/bin/chyk"
install -Dm644 assets/chyk.desktop "$stage/usr/share/applications/chyk.desktop"
install -Dm644 "$deps/chyk.png" "$stage/usr/share/icons/hicolor/256x256/apps/chyk.png"
install -Dm644 LICENSE "$stage/usr/share/licenses/chyk/LICENSE"
cat >"$rpmtop/chyk.spec" <<SPEC
Name: chyk
Version: $pkgver
Release: 1
Summary: Quick video trim, crop and strip
License: GPL-3.0-or-later
URL: https://github.com/zgv2-dev/chyk
# Prebuilt binaries: no debuginfo split, and keep the host's (Ubuntu) brp scripts out of it.
%define debug_package %{nil}
%define __os_install_post %{nil}
# Only libmpv and the system libraries; the static ffmpeg needs nothing.
AutoProv: no

%description
Chyk trims, crops, retimes and strips inactive stretches from videos,
stream-copying when nothing needs re-encoding. Playback uses libmpv;
export uses the bundled ffmpeg.

%install
cp -a $stage/. %{buildroot}/

%files
/usr/bin/chyk
/usr/lib/chyk
/usr/share/applications/chyk.desktop
/usr/share/icons/hicolor/256x256/apps/chyk.png
%license /usr/share/licenses/chyk/LICENSE
SPEC
rpmbuild -bb --quiet --target "$arch" --define "_topdir $rpmtop" "$rpmtop/chyk.spec" >/dev/null
mv "$rpmtop"/RPMS/*/chyk-*.rpm "dist/Chyk-$version-linux-$arch.rpm"
ls -lh dist/*.AppImage dist/*.deb dist/*.rpm
