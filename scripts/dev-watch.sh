#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

watch_paths=(Cargo.toml Cargo.lock crates data)

restart_ibus() {
  ibus write-cache || true
  ibus restart || true
}

apply_change() {
  cargo build -p korean-cli -p korean-ibus
  restart_ibus
  printf '\nKorean rebuilt and IBus restarted.\n'
}

source_fingerprint() {
  find "${watch_paths[@]}" \
    -type f \
    \( -name '*.rs' -o -name '*.c' -o -name '*.h' -o -name '*.toml' -o -name '*.lock' -o -name '*.xml' \) \
    -not -path '*/target/*' \
    -print0 \
    | sort -z \
    | xargs -0 sha256sum \
    | sha256sum
}

watch_with_inotify() {
  while true; do
    inotifywait -qr \
      -e close_write,create,delete,move \
      --include '.*\.(rs|c|h|toml|lock|xml)$' \
      "${watch_paths[@]}" >/dev/null
    apply_change || true
  done
}

watch_with_polling() {
  local previous current
  previous="$(source_fingerprint)"

  while true; do
    sleep 1
    current="$(source_fingerprint)"
    if [[ "$current" != "$previous" ]]; then
      previous="$current"
      apply_change || true
    fi
  done
}

./scripts/dev-apply.sh

cat <<'MSG'

Korean dev watch is running.
Edit Rust/C/XML files; this script rebuilds and restarts IBus after each change.
Stop with Ctrl+C.
MSG

if command -v inotifywait >/dev/null 2>&1; then
  watch_with_inotify
else
  printf '\ninotifywait not found; using 1s polling.\n'
  watch_with_polling
fi
