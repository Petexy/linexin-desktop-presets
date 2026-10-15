# Maintainer: Petexy <https://github.com/Petexy>
#
# Release package built from the checkout this file lives in:
#   makepkg -si
#
# The crate takes the Linexin Center's crates from its repository; Cargo
# fetches them, and Cargo.lock pins the commit.
#
# Same name and files as the Python package it replaces, except that
# b-desktop_presets.py becomes b-desktop_presets.so: the Center keeps the
# widget's place and hidden state across the upgrade.

pkgname=linexin-desktop-presets
pkgver=4.0.0
pkgrel=1
pkgdesc='Desktop Presets widget for Linexin Center'
url='https://github.com/Petexy/linexin-desktop-presets'
arch=('x86_64')
license=('GPL-3.0-or-later')
depends=('linexin-center>=4.0.0' 'gtk4' 'libadwaita' 'python' 'python-gobject')
makedepends=('cargo')
# No makepkg LTO: it compiles the C code some Rust crates build (ring, the
# TLS library's crypto) into GCC bytecode the Rust linker cannot use, leaving
# those symbols undefined in the built library, which then fails to load.
# Rust's own optimisation is set in Cargo.toml.
options=('!lto')
install=linexin-desktop-presets.install

_root="$(realpath "$startdir")"


build() {
  cd "$_root"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$_root/target}"
  # Distribution builds must run on any x86-64 CPU: drop a host-specific
  # target-cpu from makepkg.conf (e.g. CachyOS's "native") and pin the baseline.
  export RUSTFLAGS="${RUSTFLAGS//-C target-cpu=native/} -C target-cpu=x86-64"
  cargo build --release --locked --lib
}

package() {
  cd "$_root"
  local release="${CARGO_TARGET_DIR:-$_root/target}/release"

  install -Dm644 "$release/liblinexin_desktop_presets.so" \
    "$pkgdir/usr/share/linexin/widgets/b-desktop_presets.so"

  # Everything else the Python package shipped, with its modes.
  local file
  while IFS= read -r -d '' file; do
    if [[ -x "data/$file" ]]; then
      install -Dm755 "data/$file" "$pkgdir/$file"
    else
      install -Dm644 "data/$file" "$pkgdir/$file"
    fi
  done < <(cd data && find . -type f -printf '%P\0')

  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
