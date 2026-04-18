# Maintainer: Petexy <https://github.com/Petexy>

pkgname=linexin-desktop-presets
pkgver=3.3.0.r
pkgrel=3
pkgdesc='Change your style'
url='https://github.com/Petexy'
arch=('x86_64')
license=('GPL-3.0')
depends=(
  'python-gobject'
  'gtk4'
  'libadwaita'
  'python'
  'linexin-center'
)
install="${pkgname}.install"

package() {
    cd "${srcdir}"

    find usr etc -type f 2>/dev/null | while IFS= read -r _file; do
        if [[ "${_file}" == usr/bin/* || "${_file}" == *.sh ]]; then
            install -Dm755 "${_file}" "${pkgdir}/${_file}"
        else
            install -Dm644 "${_file}" "${pkgdir}/${_file}"
        fi
    done
}
