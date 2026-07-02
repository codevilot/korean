#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

repo_dir="$(pwd)"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
bin_dir="$HOME/.local/bin"
component_dir="$data_home/ibus/component"
engine_name="korean-dev"
service_name="org.freedesktop.IBus.KoreanDev"
component_file="$component_dir/$engine_name.xml"
cli_bin="$repo_dir/target/debug/korean"
ibus_bin="$repo_dir/target/debug/korean-ibus"
ibus_wrapper="$repo_dir/target/debug/korean-ibus-dev"
debug_log="/tmp/korean-ibus.log"
render_mode="${KOREAN_DEV_RENDER_MODE:-delayed_preview}"
delete_mode="${KOREAN_DEV_DELETE_MODE:-surrounding}"
repeat_delay_ms="${KOREAN_DEV_REPEAT_DELAY_MS:-180}"
repeat_interval_ms="${KOREAN_DEV_REPEAT_INTERVAL_MS:-15}"
tmp_component="$(mktemp)"
trap 'rm -f "$tmp_component"' EXIT

select_gnome_source() {
  if ! command -v gsettings >/dev/null 2>&1; then
    return 0
  fi

  local schema="org.gnome.desktop.input-sources"
  local sources
  sources="$(gsettings get "$schema" sources 2>/dev/null || echo "[]")"

  if [[ "$sources" != *"('ibus', '$engine_name')"* ]]; then
    if [[ "$sources" == "[]" ]]; then
      sources="[('ibus', '$engine_name')]"
    else
      sources="${sources%]}, ('ibus', '$engine_name')]"
    fi
    gsettings set "$schema" sources "$sources" || return 0
  fi

  local index=0
  local part normalized
  normalized="${sources#[}"
  normalized="${normalized%]}"
  IFS=$'\n' read -rd '' -a parts < <(printf '%s\n' "$normalized" | sed "s/), (/)\n(/g") || true
  for part in "${parts[@]}"; do
    if [[ "$part" == *"$engine_name"* ]]; then
      gsettings set "$schema" current "$index" || return 0
      break
    fi
    index=$((index + 1))
  done

  gsettings set org.gnome.desktop.wm.keybindings switch-input-source "['Caps_Lock']" || return 0
  gsettings set org.gnome.desktop.wm.keybindings switch-input-source-backward "[]" || return 0
}

wait_for_ibus() {
  local i
  for i in {1..30}; do
    if ibus engine >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.1
  done
  return 1
}

tune_keyboard_repeat() {
  if [[ "${KOREAN_DEV_TUNE_KEYBOARD:-1}" == "0" ]]; then
    return 0
  fi
  if ! command -v gsettings >/dev/null 2>&1; then
    return 0
  fi
  gsettings set org.gnome.desktop.peripherals.keyboard delay "$repeat_delay_ms" || return 0
  gsettings set org.gnome.desktop.peripherals.keyboard repeat-interval "$repeat_interval_ms" || return 0
}

cargo build -p korean-cli -p korean-ibus

mkdir -p "$component_dir" "$bin_dir"
ln -sf "$cli_bin" "$bin_dir/korean"
: >"$debug_log"
cat >"$ibus_wrapper" <<WRAPPER
#!/usr/bin/env bash
export KOREAN_DEBUG_LOG="$debug_log"
export KOREAN_IBUS_SERVICE="$service_name"
export KOREAN_RENDER_MODE="$render_mode"
export KOREAN_DELETE_MODE="$delete_mode"
exec "$ibus_bin" "\$@"
WRAPPER
chmod +x "$ibus_wrapper"

cat >"$tmp_component" <<XML
<?xml version="1.0" encoding="utf-8"?>
<component>
  <name>$service_name</name>
  <description>Korean Input Method</description>
  <exec>$ibus_wrapper</exec>
  <version>0.1.0-dev</version>
  <author>BK</author>
  <license>MIT</license>
  <textdomain>korean</textdomain>
  <engines>
    <engine>
      <name>$engine_name</name>
      <language>ko</language>
      <license>MIT</license>
      <author>BK</author>
      <layout>us</layout>
      <longname>Korean Dev</longname>
      <description>macOS-like Korean input method for Linux</description>
      <rank>99</rank>
    </engine>
  </engines>
</component>
XML

cp "$tmp_component" "$component_file"

ibus write-cache || true
if ! ibus read-cache | grep -q "$engine_name"; then
  system_component_file="/usr/share/ibus/component/$engine_name.xml"
  if [[ ! -f "$system_component_file" ]] || ! cmp -s "$tmp_component" "$system_component_file"; then
    cat <<MSG
IBus did not load the user-local component.
Installing the dev component into /usr/share/ibus/component once.
MSG
    if ! sudo install -Dm644 "$tmp_component" "$system_component_file"; then
      cat <<MSG
Could not install the system IBus dev component.
Run this from a normal terminal so sudo can ask for your password:

  cd $repo_dir
  ./scripts/dev-apply.sh
MSG
      exit 1
    fi
  fi
  ibus write-cache || true
fi
ibus restart || true
wait_for_ibus || true
select_gnome_source
tune_keyboard_repeat
ibus engine "$engine_name" || true

cat <<MSG
Korean dev engine applied.

IBus exec:
  $ibus_wrapper

Debug log:
  $debug_log

Render mode:
  $render_mode

Delete mode:
  $delete_mode

Keyboard repeat:
  delay=${repeat_delay_ms}ms
  interval=${repeat_interval_ms}ms

Component:
  $component_file

During development, rerun:
  ./scripts/dev-apply.sh

Select 'Korean Dev' from the GNOME input source menu.

To test visible_tail explicitly:
  KOREAN_DEV_RENDER_MODE=visible_tail ./scripts/dev-apply.sh

To disable preedit preview while keeping delayed commits:
  KOREAN_DEV_RENDER_MODE=delayed ./scripts/dev-apply.sh

To tune Backspace/key-repeat speed:
  KOREAN_DEV_REPEAT_DELAY_MS=180 KOREAN_DEV_REPEAT_INTERVAL_MS=15 ./scripts/dev-apply.sh

To test the previous preedit path:
  KOREAN_DEV_RENDER_MODE=preedit ./scripts/dev-apply.sh
MSG

if [[ "$(command -v korean 2>/dev/null || true)" != "$bin_dir/korean" ]]; then
  cat <<MSG

Note: your shell resolves korean to:
  $(command -v korean 2>/dev/null || echo "not found")

For the dev CLI in this shell, run:
  $cli_bin status
MSG
fi
