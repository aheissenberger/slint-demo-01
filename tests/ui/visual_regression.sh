#!/usr/bin/env bash
set -euo pipefail

export DISPLAY="${DISPLAY:-:1}"
baseline_dir="tests/ui/baselines"
output_dir="artifacts/screenshots/visual"
mkdir -p "$output_dir"

capture() {
  local name="$1"
  sleep 0.25
  ./scripts/screenshot "visual-${name}" >/dev/null
}

compare_baseline() {
  local name="$1"
  local baseline="$baseline_dir/${name}.png"
  local current="artifacts/screenshots/visual-${name}.png"
  [[ -f "$baseline" ]] || {
    echo "Missing visual baseline: $baseline" >&2
    return 1
  }
  compare -metric AE "$baseline" "$current" null: 2>"$output_dir/${name}.metric" || true
  local differing_pixels
  differing_pixels="$(tr -dc '0-9' < "$output_dir/${name}.metric")"
  if [[ -z "$differing_pixels" || "$differing_pixels" -gt 10000 ]]; then
    echo "Visual regression detected for ${name}" >&2
    cat "$output_dir/${name}.metric" >&2
    return 1
  fi
}

./scripts/agent command reset >/dev/null
capture main

./scripts/agent click help.about >/dev/null
capture about
./scripts/agent click about.close >/dev/null

./scripts/agent click file.settings >/dev/null
capture settings
./scripts/agent click settings.close >/dev/null

compare_baseline main
compare_baseline about
compare_baseline settings
echo "PASS visual regression baselines"
