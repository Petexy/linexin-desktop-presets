# Desktop Presets

<p align="center">
  <img src="data/usr/share/icons/customize.svg" alt="Desktop Presets" width="200" height="200"/>
</p>

**Desktop Presets** ("Change your style") gives your desktop a new look in one
click. It is a widget for [Linexin Center](https://github.com/Petexy/Linexin-Center)
that applies ready-made Linexin styles to GNOME and KDE Plasma: panels, docks,
menus, effects and layout.

This version is a rewrite in **Rust** (GTK4 + Libadwaita) and a drop-in
replacement for the Python widget: same package, same presets, same look.

## ✨ Features

* **Ready-Made Styles:** Four GNOME styles and five KDE Plasma styles, each with
  a preview picture.
* **Detects Your Desktop:** Shows the styles for the desktop you are running,
  GNOME or Plasma.
* **One-Click Apply:** Pick a style, press **Apply Style**, and the style is set
  up for your monitors.
* **Clean Finish:** Offers to log you out afterwards, so every part of the new
  style takes effect.
* **Translated:** Follows your system language like the rest of Linexin Center.

Desktop Presets has a GTK window only. It changes GNOME and Plasma settings, so
it is not shown in Linexin Center's LineXinBar interface.

## 📦 Installation

**On Linexin** Desktop Presets is already installed. Open **Change your style**
from Linexin Center.

**On other Arch-based distros** install [Linexin Center](https://github.com/Petexy/Linexin-Center)
first, then build and install this package:

```bash
sudo pacman -S --needed base-devel git
git clone https://github.com/Petexy/linexin-desktop-presets.git
cd linexin-desktop-presets
makepkg -si
```

To open it on its own:

```bash
linexin-center -w b-desktop_presets
```

## 📂 Where the Styles Live

The styles are installed to `/etc/skel/.local/share/linexin/linexin-desktop/`
and copied into your home folder, `~/.local/share/linexin/linexin-desktop/`,
which is where the widget reads them.

## 🛠️ Requirements

* Linexin Center 4.0 or newer
* GTK 4 and Libadwaita
* Python 3 and PyGObject (the style scripts use them)

## 🔧 Building from Source

```bash
cargo build --release --lib
```

Cargo fetches Linexin Center's crates from its repository while building.
Tests, how to build against a local Linexin Center checkout and how the widget
works inside are described in [DEVELOPMENT.md](DEVELOPMENT.md).

## Licence

Copyright (C) 2026 Petexy

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version. It is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the GNU General Public License in `LICENSE` for more
details.
