#!/usr/bin/env bash
set -euo pipefail

workspace="${DEVCONTAINER_WORKSPACE:-/workspace}"
target_dir="${CARGO_TARGET_DIR:-$workspace/target}"
retention_days="${CARGO_TARGET_RETENTION_DAYS:-14}"
max_gib="${CARGO_TARGET_MAX_GIB:-40}"
cleanup_interval_minutes=1440

if [[ ! "$retention_days" =~ ^[0-9]+$ ]] || [[ ! "$max_gib" =~ ^[1-9][0-9]*$ ]]; then
  echo "CARGO_TARGET_RETENTION_DAYS muss nicht negativ und CARGO_TARGET_MAX_GIB muss positiv sein." >&2
  exit 2
fi

workspace="$(realpath -m "$workspace")"
target_dir="$(realpath -m "$target_dir")"
expected_target_dir="$workspace/target"
if [[ "$target_dir" != "$expected_target_dir" ]]; then
  echo "Unsicheres CARGO_TARGET_DIR wird nicht bereinigt: $target_dir" >&2
  exit 2
fi

mkdir -p "$target_dir"
cache_tag="$target_dir/CACHEDIR.TAG"
cache_signature="Signature: 8a477f597d28d172789f06886806bc55"
if [[ -f "$cache_tag" ]] && [[ "$(<"$cache_tag")" != "$cache_signature"* ]]; then
  echo "Ungültiges CACHEDIR.TAG im Cargo-Target; die Bereinigung wird abgebrochen." >&2
  exit 2
fi
if [[ ! -e "$cache_tag" ]]; then
  printf '%s\n' "$cache_signature" >"$cache_tag"
fi

stamp_file="$target_dir/.devcontainer-cleanup-stamp"
if [[ -f "$stamp_file" ]] &&
  find "$stamp_file" -mmin "-$cleanup_interval_minutes" -print -quit | grep -q .; then
  exit 0
fi

exec 9>/tmp/slint-demo-cargo-target-cleanup.lock
if ! flock -n 9; then
  echo "Die Cargo-Target-Bereinigung läuft bereits; dieser Start überspringt sie."
  exit 0
fi

before_kib="$(du -sk "$target_dir" | cut -f1)"
max_kib=$((max_gib * 1024 * 1024))

if ((before_kib > max_kib)); then
  echo "Cargo-Target ist größer als ${max_gib} GiB; der Build-Cache wird vollständig bereinigt."
  while IFS= read -r -d '' cache_entry; do
    rm -rf -- "$cache_entry"
  done < <(
    find "$target_dir" \
      -mindepth 1 \
      -maxdepth 1 \
      ! -name CACHEDIR.TAG \
      -print0
  )
else
  echo "Release-Artefakte werden aus dem Cargo-Target entfernt."
  cargo clean \
    --manifest-path "$workspace/Cargo.toml" \
    --target-dir "$target_dir" \
    --release

  incremental_dir="$target_dir/debug/incremental"
  if [[ -d "$incremental_dir" ]]; then
    retention_minutes=$((retention_days * 24 * 60))
    while IFS= read -r -d '' stale_session; do
      if ! find "$stale_session" -type f -mmin "-$retention_minutes" -print -quit |
        grep -q .; then
        rm -rf -- "$stale_session"
      fi
    done < <(
      find "$incremental_dir" \
        -mindepth 2 \
        -maxdepth 2 \
        -type d \
        -mmin "+$retention_minutes" \
        -print0
    )
  fi
fi

touch "$stamp_file"
after_kib="$(du -sk "$target_dir" | cut -f1)"
reclaimed_kib=$((before_kib - after_kib))
printf 'Cargo-Target-Bereinigung abgeschlossen: %s freigegeben, %s verbleiben.\n' \
  "$(numfmt --to=iec-i --suffix=B $((reclaimed_kib * 1024)))" \
  "$(numfmt --to=iec-i --suffix=B $((after_kib * 1024)))"
