# Maintainer: Petexy <https://github.com/Petexy>

pkgname=linexin-desktop-presets
pkgver=3.2.1.r
pkgrel=1
_currentdate=$(date +"%Y-%m-%d%H-%M-%S")
pkgdesc='Change your style'
url='https://github.com/Petexy'
arch=(x86_64)
license=('GPL-3.0')
depends=(
  python-gobject
  gtk4
  libadwaita
  python
  linexin-center
)
makedepends=(
)
install="${pkgname}.install"

package() {
   mkdir -p ${pkgdir}/usr/share/linexin/widgets
   mkdir -p ${pkgdir}/usr/share/icons
   mkdir -p ${pkgdir}/etc/skel/.local/share/linexin/linexin-desktop
   mkdir -p ${pkgdir}/usr/share/applications
   cp -rf ${srcdir}/usr/ ${pkgdir}/
   cp -rf ${srcdir}/etc/skel/.local/share/linexin/linexin-desktop ${pkgdir}/etc/skel/.local/share/linexin/
}
