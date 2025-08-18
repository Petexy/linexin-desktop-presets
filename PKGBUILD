# Maintainer: Petexy <https://github.com/Petexy>

pkgname=linexin-desktop-presets
pkgver=1.0.15.r
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
)
makedepends=(
)
install="${pkgname}.install"

package() {
   mkdir -p ${pkgdir}/usr/bin
   mkdir -p ${pkgdir}/usr/share/linexin/linexin-desktop
   mkdir -p ${pkgdir}/usr/share/icons
   mkdir -p ${pkgdir}/etc/skel/.local/share/linexin/linexin-desktop
   mkdir -p ${pkgdir}/usr/share/gnome-shell/extensions
   cp -rf ${srcdir}/usr/bin/${pkgname} ${pkgdir}/usr/bin/${pkgname}
   cp -rf ${srcdir}/usr/share/applications ${pkgdir}/usr/share/
   cp -rf ${srcdir}/usr/share/icons ${pkgdir}/usr/share/
   cp -rf ${srcdir}/etc/skel/.local/share/linexin/linexin-desktop ${pkgdir}/etc/skel/.local/share/linexin/
}
