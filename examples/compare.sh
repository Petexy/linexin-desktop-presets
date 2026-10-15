#!/bin/bash
# Renders the native widget (harness.rs) and the original Python widget
# (python_reference.py) with identical options in a private headless KWin and
# compares every pair: PNGs pixel by pixel, printed widget states and dialogs,
# and the commands the apply/logout flows ran.
#
#   cargo build --lib --examples
#   examples/compare.sh                 # ONLY=REGEX limits the scenarios
#
# Nothing touches the real session: everything runs in a clean environment
# (env -i) inside a private D-Bus session, HOME and the XDG directories point
# to .test-env/home (a copy of /etc/skel's presets whose scripts are replaced
# by harmless ones that log and sleep), and PATH holds only fake qdbus6, qdbus,
# loginctl, gnome-session-quit, kwriteconfig6, plasma-apply-*, dconf, ...
# (which log their arguments) plus a few coreutils. The real ones are not
# reachable.
set -u
examples=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$examples/.." && pwd)
env_dir=$repo/.test-env
home=$env_dir/home
out=$env_dir/compare
socket=${LINEXIN_TEST_SOCKET:-linexin-test-presets}
# Where cargo built this checkout (CARGO_TARGET_DIR and cargo's settings
# count), asked before the private session, which has no cargo on PATH.
target=${LINEXIN_COMPARE_TARGET:-$(cd "$repo" && cargo metadata --no-deps --format-version 1 |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')/debug}
harness=$target/examples/harness
library=$target/liblinexin_desktop_presets.so
python_widget=${PYTHON_WIDGET:-/usr/share/linexin/widgets/b-desktop_presets.py}
presets=$home/.local/share/linexin/linexin-desktop

if [ -z "${LINEXIN_PRESETS_SESSION:-}" ]; then
    [ -x "$harness" ] && [ -f "$library" ] || { echo "run: cargo build --lib --examples" >&2; exit 2; }
    [ -f "$python_widget" ] || {
        echo "No original Python widget at $python_widget: install it, or name its file in PYTHON_WIDGET" >&2
        exit 2
    }
    if [ -n "$(find "$repo/widget" "$repo/Cargo.toml" -newer "$library")$(find "$examples/harness.rs" -newer "$harness")" ]; then
        echo "compare.sh: stale build, run: cargo build --lib --examples" >&2
        exit 2
    fi
    rm -rf "$home" "$out" "$env_dir/bin" "$env_dir/safe-bin" "$env_dir"/bin-*
    mkdir -p "$out/png" "$out/logs" "$home/.config" "$home/.cache" "$home/.local/share/linexin" \
        "$env_dir/bin" "$env_dir/safe-bin"

    # Presets and previews from this checkout's copy of /etc/skel, scripts replaced.
    cp -r "$repo/data/etc/skel/.local/share/linexin/linexin-desktop" "$home/.local/share/linexin/"
    cat >"$env_dir/fake-preset.sh" <<'EOF'
#!/bin/bash
# Harmless stand-in for a Linexin preset script (examples/compare.sh).
name=${0##*/}
runs="$HOME/.preset-runs"
mkdir -p "$runs"
count=$(( $(cat "$runs/$name" 2>/dev/null || echo 0) + 1 ))
echo "$count" >"$runs/$name"
echo "script $name run $count args=[$*]" >>"$FAKE_LOG"
echo "harmless output of $name"
case $name in
    update-monitor.sh) sleep 1.5 ;;
    *) sleep 0.3 ;;
esac
if [ "${PRESET_FAIL:-}" = "$name" ] && [ "${PRESET_FAIL_RUN:-1}" = "$count" ]; then
    printf '  fake failure of %s (run %s)\n\n' "$name" "$count" >&2
    exit 3
fi
exit 0
EOF
    find "$presets" -name '*.sh' -exec cp "$env_dir/fake-preset.sh" {} \; -exec chmod 755 {} \;
    printf '#!/bin/sh\necho FAKE-MONITOR\n' >"$presets/get_primary_monitor.py"
    if grep -rlE 'dconf|gsettings|kwriteconfig|plasma-apply|cp -rf' --include='*.sh' --include='*.py' "$presets"; then
        echo "compare.sh: a real preset script is left in $presets" >&2
        exit 2
    fi

    # Fake desktop commands: log and succeed.
    cat >"$env_dir/fake-command" <<'EOF'
#!/bin/bash
echo "command ${0##*/} [$*] USER=${USER-unset}" >>"$FAKE_LOG"
EOF
    chmod 755 "$env_dir/fake-command" "$env_dir/fake-preset.sh"
    fakes=(qdbus6 qdbus loginctl gnome-session-quit kwriteconfig6 kreadconfig6 kquitapp6 kstart
        plasmashell plasma-apply-colorscheme plasma-apply-desktoptheme plasma-apply-lookandfeel
        plasma-apply-cursortheme plasma-apply-wallpaperimage lookandfeeltool dconf gsettings
        gnome-extensions systemctl busctl dbus-send gdbus xrandr kscreen-doctor python3 sudo pkexec)
    for name in "${fakes[@]}"; do ln -s ../fake-command "$env_dir/bin/$name"; done
    # Logout fallbacks: without qdbus6, and without qdbus6 and qdbus.
    for variant in noqdbus6 noqdbus; do
        mkdir -p "$env_dir/bin-$variant"
        for name in "${fakes[@]}"; do
            case $variant:$name in noqdbus6:qdbus6 | noqdbus:qdbus6 | noqdbus:qdbus) continue ;; esac
            ln -s ../fake-command "$env_dir/bin-$variant/$name"
        done
    done
    for tool in bash sh cat mkdir sleep date basename dirname env rm; do
        ln -s "$(command -v $tool)" "$env_dir/safe-bin/$tool"
    done

    cat >"$env_dir/bus.conf" <<'EOF'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=/tmp</listen>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/>
  </policy>
</busconfig>
EOF
    "$harness" --print-css >"$env_dir/center-style.css" || exit 2
    exec env -i LINEXIN_COMPARE_TARGET="$target" PYTHON_WIDGET="$python_widget" \
        LINEXIN_CENTER_CSS="$env_dir/center-style.css" LINEXIN_PRESETS_SESSION=1 ONLY="${ONLY:-}" HOME="$home" USER=linexin-test LANG=C.UTF-8 \
        PATH=/usr/bin:/bin XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
        XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" \
        XDG_CACHE_HOME="$home/.cache" GSETTINGS_BACKEND=memory ADW_DISABLE_PORTAL=1 \
        GDK_DEBUG=no-portals GTK_A11Y=none NO_AT_BRIDGE=1 \
        dbus-run-session --config-file="$env_dir/bus.conf" -- "$0"
fi

kwin_wayland --virtual --no-lockscreen --width 1600 --height 1000 --socket "$socket" \
    >"$out/kwin.log" 2>&1 &
kwin=$!
trap 'kill $kwin 2>/dev/null; wait $kwin 2>/dev/null' EXIT
for _ in $(seq 100); do [ -S "$XDG_RUNTIME_DIR/$socket" ] && break; sleep 0.1; done
[ -S "$XDG_RUNTIME_DIR/$socket" ] || { echo "compare.sh: kwin_wayland did not start" >&2; exit 2; }
export WAYLAND_DISPLAY=$socket GDK_BACKEND=wayland
unset DISPLAY

# pair TAG DESKTOP OPTIONS...: run both implementations. Environment knobs:
# BIN (fake command dir), PRESET_FAIL/PRESET_FAIL_RUN (failing script), PREP
# and UNDO (commands run around each implementation).
pair() {
    local tag=$1 desktop=$2
    shift 2
    [[ -z $ONLY || $tag =~ $ONLY ]] || return 0
    local impl
    for impl in rust py; do
        rm -rf "$home/.preset-runs"
        [ -n "${PREP:-}" ] && eval "$PREP"
        local args=() option
        for option in "$@"; do args+=("${option//@/$out/png/$impl-$tag}"); done
        (
            export XDG_CURRENT_DESKTOP=$desktop FAKE_LOG=$out/logs/$impl-$tag.commands
            export PATH=${BIN:-$env_dir/bin}:$env_dir/safe-bin
            : >"$FAKE_LOG"
            if [ $impl = rust ]; then
                "$harness" --library "$library" "${args[@]}"
            else
                /usr/bin/python3 "$examples/python_reference.py" --widget "$python_widget" "${args[@]}"
            fi
        ) >"$out/logs/$impl-$tag.out" 2>"$out/logs/$impl-$tag.err"
        echo "$impl $tag: exit $?" >>"$out/status.txt"
        [ -n "${UNDO:-}" ] && eval "$UNDO"
    done
    echo "$tag" >>"$out/pairs.txt"
}

: >"$out/pairs.txt"
: >"$out/status.txt"
for lang in en_US pl_PL; do
    clicks=""
    for page in 0 1 2 3 4; do
        pair "kde-$lang-page$page" KDE --lang $lang --clicks "$clicks" --output @.png
        [ $page -lt 4 ] && pair "gnome-$lang-page$page" GNOME --lang $lang --clicks "$clicks" --output @.png
        clicks+=n
    done
    pair "kde-$lang-back" KDE --lang $lang --clicks nnnnnppp --output @.png
    pair "kde-$lang-apply" KDE --lang $lang --clicks n --output @.png --apply \
        --running-output @-running.png --second-output @-second.png --dialog-output @-dialog.png \
        --after-output @-after.png --respond logout
    pair "gnome-$lang-apply" GNOME --lang $lang --clicks nnn --output @.png --apply \
        --running-output @-running.png --second-output @-second.png --dialog-output @-dialog.png \
        --after-output @-after.png --respond logout
    PRESET_FAIL=update-monitor.sh pair "gnome-$lang-monitor-fails" GNOME --lang $lang --clicks n \
        --apply --running-output @-running.png --dialog-output @-dialog.png --after-output @-after.png
    PRESET_FAIL=style3.sh PRESET_FAIL_RUN=2 pair "kde-$lang-second-run-fails" KDE --lang $lang \
        --clicks nn --apply --running-output @-running.png --dialog-output @-dialog.png \
        --after-output @-after.png --respond ok
    PREP="chmod 644 '$presets/gnome.sh'" UNDO="chmod 755 '$presets/gnome.sh'" \
        pair "gnome-$lang-script-not-executable" GNOME --lang $lang --clicks nnn --apply \
        --dialog-output @-dialog.png --after-output @-after.png
    pair "gnome-$lang-single" GNOME --lang $lang --hide-sidebar --output @.png
done
export PRESET_FAIL_RUN=1
PRESET_FAIL=style1.sh pair kde-first-run-fails KDE --apply --dialog-output @-dialog.png --after-output @-after.png
PREP="chmod 644 '$presets/update-monitor.sh'" UNDO="chmod 755 '$presets/update-monitor.sh'" \
    pair gnome-monitor-not-executable GNOME --clicks n --apply --dialog-output @-dialog.png
for desktop in GNOME KDE; do
    PREP="mv '$presets/windowish.png' '$presets/windowish.png.off'; mv '$presets/plasma/style5.png' '$presets/plasma/style5.png.off'" \
    UNDO="mv '$presets/windowish.png.off' '$presets/windowish.png'; mv '$presets/plasma/style5.png.off' '$presets/plasma/style5.png'" \
        pair "${desktop,,}-missing-preview" $desktop --lang pl_PL --clicks nnnn --output @.png
done
pair kde-dark KDE --dark --clicks nn --output @.png --apply --dialog-output @-dialog.png --respond later
pair gnome-dark GNOME --dark --lang pl_PL --clicks n --output @.png
pair kde-narrow KDE --width 520 --height 460 --clicks n --output @.png
pair gnome-narrow GNOME --width 520 --height 460 --lang pl_PL --output @.png
pair kde-single KDE --hide-sidebar --clicks nnnn --output @.png
BIN=$env_dir/bin-noqdbus6 pair kde-logout-qdbus KDE --apply --respond logout
BIN=$env_dir/bin-noqdbus pair kde-logout-loginctl KDE --apply --respond logout
pair unknown-desktop LineXinBar --clicks nnn --output @.png
pair gnome-destroy-during-apply GNOME --apply --destroy-during-apply

python3 - "$out" <<'EOF'
import pathlib, re, sys
from PIL import Image, ImageChops
out = pathlib.Path(sys.argv[1])
png, logs = out / "png", out / "logs"
problems = 0

def normalize(text, impl):
    lines = []
    for line in text.splitlines():
        # Lines only one side prints: the descriptor/instance summary, the
        # native finalization checks, and the reference's own diagnostics.
        if line.startswith(("widget_id=", "widgetname=", "widget finalized")):
            continue
        lines.append(line.replace(f"/{impl}-", "/IMPL-"))
    return lines

for tag in out.joinpath("pairs.txt").read_text().split():
    report = []
    for kind in ("out", "commands"):
        rust = normalize(logs.joinpath(f"rust-{tag}.{kind}").read_text(), "rust")
        py = normalize(logs.joinpath(f"py-{tag}.{kind}").read_text(), "py")
        if rust != py:
            report.append(f"{kind} differs")
            for a, b in zip(rust + [""] * len(py), py + [""] * len(rust)):
                if a != b:
                    report.append(f"    rust: {a}\n    py:   {b}")
                    break
    # stderr: the same warnings (GTK's, the Center CSS's `:insensitive`) on
    # both sides, compared without process names, pids and timestamps.
    def warnings(impl):
        text = logs.joinpath(f"{impl}-{tag}.err").read_text()
        text = re.sub(r"\((?:harness|python3):\d+\): ", "", text)
        text = re.sub(r"\d\d:\d\d:\d\d\.\d{3}: ", "", text)
        return [line for line in text.splitlines() if line.strip()]
    if warnings("rust") != warnings("py"):
        extra = [line for line in warnings("rust") if line not in warnings("py")]
        missing = [line for line in warnings("py") if line not in warnings("rust")]
        report.append(f"stderr differs: rust only {extra[:2]}, py only {missing[:2]}")
    images = sorted(png.glob(f"py-{tag}.png")) + sorted(png.glob(f"py-{tag}-*.png"))
    for py in images:
        native = py.with_name("rust-" + py.name[3:])
        if not native.exists():
            report.append(f"{native.name} missing")
            continue
        a, b = Image.open(py).convert("RGBA"), Image.open(native).convert("RGBA")
        if a.size != b.size:
            report.append(f"{py.name[3:]}: size {a.size} vs {b.size}")
            continue
        diff = ImageChops.difference(a, b)
        box = diff.getbbox(alpha_only=False)
        if box is not None:
            count = sum(1 for p in diff.getdata() if p != (0, 0, 0, 0))
            report.append(f"{py.name[3:]}: {count} pixels differ in {box}")
    problems += bool(report)
    print(f"{tag}: {len(images)} image pair(s), " + ("identical" if not report else "DIFFERENT"))
    for line in report:
        print("  " + line)
print(f"{len(out.joinpath('pairs.txt').read_text().split())} scenarios, {problems} with differences")
sys.exit(1 if problems else 0)
EOF
