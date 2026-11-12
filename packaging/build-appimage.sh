#!/usr/bin/env bash
set -euo pipefail

ROOT="$(dirname "$(dirname "$(realpath "$0")")")"
cd "$ROOT"
VERSION="$(awk -F '"' '/^version = / { print $2; exit }' Cargo.toml)"
test -n "$VERSION"

docker build -t apui-builder:bionic -f packaging/Dockerfile .
docker run --rm \
  -v apui-cargo-registry:/root/.cargo/registry \
  -v "$ROOT:/src" -w /src apui-builder:bionic \
  bash -lc 'CARGO_TARGET_DIR=target/bionic cargo build --locked --release'

rm -rf AppDir
install -Dm755 target/bionic/release/apui AppDir/usr/bin/apui
install -Dm755 packaging/AppRun AppDir/AppRun
install -Dm644 packaging/io.github.gsanmartindougan.apui.desktop \
  AppDir/io.github.gsanmartindougan.apui.desktop
install -Dm644 packaging/io.github.gsanmartindougan.apui.desktop \
  AppDir/usr/share/applications/io.github.gsanmartindougan.apui.desktop
install -Dm644 packaging/apui.svg AppDir/apui.svg
install -Dm644 packaging/apui.svg \
  AppDir/usr/share/icons/hicolor/scalable/apps/apui.svg
ln -s apui.svg AppDir/.DirIcon

mkdir -p outputs
ARTIFACT="outputs/Apui-${VERSION}-x86_64.AppImage"
TEMP_ARTIFACT="${ARTIFACT}.tmp"
rm -f "$TEMP_ARTIFACT"
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp -e ARCH=x86_64 \
  -v "$ROOT:/src" -w /src apui-builder:bionic \
  appimagetool --no-appstream /src/AppDir "/src/${TEMP_ARTIFACT}"
mv -f "$TEMP_ARTIFACT" "$ARTIFACT"
chmod +x "$ARTIFACT"
echo "Built $ARTIFACT"
