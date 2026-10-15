# Developing Desktop Presets

How this repository is built, tested and put together. What it is and how
to install and use it: [README.md](README.md).

The Desktop Presets widget of Linexin Center, rewritten in Rust as a native
widget (`b-desktop_presets.so`) for the
[Rust Linexin Center](https://github.com/Petexy/Linexin-Center). It replaces the Python
`b-desktop_presets.py` with the same look, the same presets and the same
apply flow: it picks a style from `~/.local/share/linexin/linexin-desktop/`
(Plasma or GNOME, detected the way the Python widget did), runs the monitor
script and the style's script, and offers to log out afterwards.

It has a GTK interface only: it configures Plasma or GNOME, so it is not listed
by the Center's LineXinBar front end, only by its GTK window (which a LineXinBar
session without lxb-toolkit gets too).

## Build

The crate takes the Linexin Center's crates (`linexin-plugin-api`, `linexin-localization`)
from [its repository](https://github.com/Petexy/Linexin-Center), so a clone builds on its own. To build
against a local checkout of the Center instead, to change both at once, say so
in `.cargo/config.toml` (not committed):

```toml
[patch."https://github.com/Petexy/Linexin-Center"]
linexin-plugin-api = { path = "/path/to/Linexin-Center/crates/plugin-api" }
linexin-localization = { path = "/path/to/Linexin-Center/crates/localization" }
```

```sh
cargo build --release --lib
makepkg -si                          # the linexin-desktop-presets package
```

## Install layout

```text
/usr/share/linexin/widgets/b-desktop_presets.so
/usr/share/linexin/widgets/localization/<lang>/desktop_presets_dictionary.py
/etc/skel/.local/share/linexin/linexin-desktop/...   presets, copied into the
                                                     user's home on install
/usr/share/applications/github.petexy.linexin-desktop-presets.desktop
/usr/share/icons/{customize,linexin}.svg
```

`data/` holds every file of the Python package except the widget itself.

## Tests

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --lib --examples && examples/compare.sh
```

`examples/compare.sh` renders this widget (`examples/harness.rs`) and the
original Python widget (`examples/python_reference.py`) in a private headless
KWin and compares every pair pixel by pixel, together with the dialogs shown
and the commands the apply and logout flows ran: Plasma and GNOME, English and
Polish, every preset, failures, dark mode, a narrow window and single-widget
mode. It never touches the real session: it runs in a clean environment with a
scratch home whose preset scripts are harmless stand-ins, and only fake
`qdbus`, `loginctl`, `kwriteconfig6`, `sudo` and friends on `PATH`.

The comparison needs the original Python widget, installed where its package
puts it or named with `PYTHON_WIDGET=FILE`; it finds this checkout's build
wherever cargo put it (`CARGO_TARGET_DIR` counts), and the Center's stylesheet
comes with the Center's crates.
