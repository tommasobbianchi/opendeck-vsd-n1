#!/usr/bin/env bash
# Builds the two things a user actually installs:
#   dist/opendeck-vsd-n1.plugin.zip  -> OpenDeck "Install from file"
#   dist/opendeck-vsd-n1_<ver>_amd64.deb -> the udev rule (the only part needing root)
#
# The plugin itself is deliberately NOT installed by the .deb: OpenDeck keeps plugins per-user
# under ~/.config/opendeck/plugins, so a system package would have to guess at users' home
# directories. The .deb ships the zip under /usr/share and tells you to import it.
set -euo pipefail

cd "$(dirname "$0")"
ID="dev.native.plugins.opendeck-vsd-n1.sdPlugin"
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
ARCH=amd64

rm -rf build dist
mkdir -p dist

cargo build --release

# ---- OpenDeck plugin bundle ----------------------------------------------------------
mkdir -p "build/$ID"
cp -r assets manifest.json "build/$ID/"
cp target/release/opendeck-vsd-n1 "build/$ID/opendeck-vsd-n1-linux"
chmod +x "build/$ID/opendeck-vsd-n1-linux"
(cd build && zip -qr "../dist/opendeck-vsd-n1.plugin.zip" "$ID")

# ---- .deb: udev rule + the zip -------------------------------------------------------
PKG="build/deb"
mkdir -p "$PKG/DEBIAN" "$PKG/usr/lib/udev/rules.d" "$PKG/usr/share/opendeck-vsd-n1"
cp 40-opendeck-vsd-n1.rules "$PKG/usr/lib/udev/rules.d/"
cp dist/opendeck-vsd-n1.plugin.zip "$PKG/usr/share/opendeck-vsd-n1/"
cp README.md "$PKG/usr/share/opendeck-vsd-n1/"

cat > "$PKG/DEBIAN/control" <<EOF
Package: opendeck-vsd-n1
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Depends: udev
Recommends: opendeck
Maintainer: Tommaso Bianchi <tommaso.b.bianchi@gmail.com>
Description: VSD Stream Dock N1 support for OpenDeck
 Grants userspace access to the VSDinside / TreasLin Stream Dock N1 (USB 5548:1002)
 and ships the OpenDeck device plugin that drives it.
 .
 This is not a kernel driver: the device is a standard USB HID peripheral already
 bound by usbhid. The package installs a udev rule so a normal user can open its
 hidraw node, plus the OpenDeck plugin bundle to import from OpenDeck's plugin
 manager.
EOF

cat > "$PKG/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = "configure" ]; then
    udevadm control --reload-rules || true
    udevadm trigger --subsystem-match=usb || true
    cat <<'MSG'

opendeck-vsd-n1: udev rule installed. Replug the Stream Dock N1, then import the
plugin in OpenDeck: Plugins -> Install from file ->
  /usr/share/opendeck-vsd-n1/opendeck-vsd-n1.plugin.zip

MSG
fi
EOF
chmod 0755 "$PKG/DEBIAN/postinst"

dpkg-deb --build --root-owner-group "$PKG" "dist/opendeck-vsd-n1_${VERSION}_${ARCH}.deb" >/dev/null

echo "built:"
ls -1 dist/
