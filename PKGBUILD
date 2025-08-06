# Maintainer: Petexy <https://github.com/Petexy>

pkgname=linexin-desktop-presets
pkgver=1.0.0.r
pkgrel=1
_currentdate=$(date +"%Y-%m-%d%H-%M-%S")
pkgdesc='Smart Installer for Affinity suite for Linux'
url='https://github.com/Petexy'
arch=(x86_64)
license=('GPL-3.0')
depends=(
  python-gobject
  gtk4
  libadwaita
  python
)
makedepends=(
)
install="${pkgname}.install"

package() {
   mkdir -p ${pkgdir}/usr/bin
   mkdir -p ${pkgdir}/usr/share/linexin/linexin-desktop
   mkdir -p ${pkgdir}/usr/share/icons
   mkdir -p ${pkgdir}/etc/skel/.local/share/linexin/linexin-desktop
   cp -rf ${srcdir}/usr/bin/${pkgname} ${pkgdir}/usr/bin/${pkgname}
   cp -rf ${srcdir}/usr/share/applications ${pkgdir}/usr/share/
   cp -rf ${srcdir}/usr/share/icons ${pkgdir}/usr/share/
   cp -rf ${srcdir}/etc/skel/.local/share/linexin/linexin-desktop ${pkgdir}/etc/skel/.local/share/linexin/
}
