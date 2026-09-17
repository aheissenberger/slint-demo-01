#!/usr/bin/env bash
set -euo pipefail

base_url="${AGENT_API_URL:-http://127.0.0.1:8080}"
initial="$(curl --fail --silent "$base_url/agent/ui")"
printf '%s' "$initial" | grep -q '"id":"main.input"'
printf '%s' "$initial" | grep -q '"id":"main.submit"'

curl --fail --silent \
  -H 'Content-Type: application/json' \
  -d '{"action":"set_value","id":"main.input","value":"UI smoke"}' \
  "$base_url/agent/ui/action" >/dev/null
updated="$(curl --fail --silent "$base_url/agent/ui")"
printf '%s' "$updated" | grep -q '"id":"main.submit","role":"button","enabled":true'
