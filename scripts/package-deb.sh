#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

version="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)}"
arch="${ARCH:-amd64}"
out_dir="${OUT_DIR:-dist}"
pkg_root="$(mktemp -d)"
trap 'rm -rf "$pkg_root"' EXIT
chmod 0755 "$pkg_root"

if [[ -z "$version" ]]; then
  version="0.1.0"
fi

cargo build --release --workspace

install -Dm755 target/release/korean "$pkg_root/usr/bin/korean"
install -Dm755 target/release/korean-ibus "$pkg_root/usr/lib/korean/korean-ibus"
install -Dm755 target/release/korean-capsd "$pkg_root/usr/lib/korean/korean-capsd"
install -Dm644 data/ibus/korean.xml "$pkg_root/usr/share/ibus/component/korean.xml"
install -Dm644 data/xdg/korean-setup.desktop "$pkg_root/etc/xdg/autostart/korean-setup.desktop"
install -Dm644 data/systemd/korean-capsd.service "$pkg_root/usr/lib/systemd/user/korean-capsd.service"
install -Dm644 data/udev/90-korean.rules "$pkg_root/etc/udev/rules.d/90-korean.rules"
install -Dm644 README.md "$pkg_root/usr/share/doc/korean/README.md"

mkdir -p "$pkg_root/DEBIAN"
cat >"$pkg_root/DEBIAN/control" <<CONTROL
Package: korean
Version: $version
Section: utils
Priority: optional
Architecture: $arch
Maintainer: codevilot <codevilot@users.noreply.github.com>
Depends: ibus, libibus-1.0-5, libglib2.0-0, fonts-noto-cjk | fonts-nanum
Recommends: gsettings-desktop-schemas
Description: macOS-like Korean input method for Linux
 Korean is a small IBus input method for two-beolsik Hangul composition.
 It provides a Korean CLI, an IBus engine, and Caps Lock input-source setup.
CONTROL

cat >"$pkg_root/DEBIAN/postinst" <<'POSTINST'
#!/usr/bin/env bash
set -e
if command -v ibus >/dev/null 2>&1; then
  ibus write-cache >/dev/null 2>&1 || true
fi
exit 0
POSTINST

cat >"$pkg_root/DEBIAN/postrm" <<'POSTRM'
#!/usr/bin/env bash
set -e
if command -v ibus >/dev/null 2>&1; then
  ibus write-cache >/dev/null 2>&1 || true
fi
exit 0
POSTRM

chmod 0755 "$pkg_root/DEBIAN/postinst" "$pkg_root/DEBIAN/postrm"

mkdir -p "$out_dir"
deb="$out_dir/korean_${version}_${arch}.deb"
dpkg-deb --build --root-owner-group "$pkg_root" "$deb"

echo "$deb"
