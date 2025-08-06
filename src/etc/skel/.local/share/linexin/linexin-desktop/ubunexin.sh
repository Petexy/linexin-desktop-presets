#!/usr/bin/env bash

dconf load /org/gnome/shell/extensions/dash-to-dock/ < $HOME/.local/share/linexin/linexin-desktop/dash-to-dock-ubunexin-linexin.ini
dconf load /org/gnome/shell/extensions/blur-my-shell/ < $HOME/.local/share/linexin/linexin-desktop/blur-ubunexin-linexin.ini
gsettings set org.gnome.shell disabled-extensions "[]"
gsettings set org.gnome.shell enabled-extensions "['accent-directories@taiwbi.com', 'blur-my-shell@aunetx', 'gtk4-ding@smedius.gitlab.com', 'dash-to-dock@micxgx.gmail.com', 'appindicatorsupport@rgcjonas.gmail.com', 'arch-update@RaphaelRochet', 'quick-settings-audio-panel@rayzeq.github.io']"
gsettings set org.gnome.desktop.wm.preferences button-layout ':close,minimize,maximize'
