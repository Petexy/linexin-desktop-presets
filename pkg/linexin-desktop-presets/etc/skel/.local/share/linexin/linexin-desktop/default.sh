#!/usr/bin/env bash

dconf load /org/gnome/shell/extensions/dash-to-dock/ < $HOME/.local/share/linexin/linexin-desktop/dash-to-dock-linexin.ini
dconf reset -f /org/gnome/shell/extensions/blur-my-shell/
gsettings set org.gnome.shell disabled-extensions "[]"
gsettings set org.gnome.shell enabled-extensions "['accent-directories@taiwbi.com', 'blur-my-shell@aunetx', 'dash-to-dock@micxgx.gmail.com', 'appindicatorsupport@rgcjonas.gmail.com', 'arch-update@RaphaelRochet', 'pip-on-top@rafostar.github.com', 'quick-settings-audio-panel@rayzeq.github.io', 'rounded-window-corners@fxgn']"
gsettings set org.gnome.desktop.wm.preferences button-layout 'close,minimize,maximize:'
