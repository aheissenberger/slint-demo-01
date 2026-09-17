#!/usr/bin/env bash
set -euo pipefail

base_url="${AGENT_API_URL:-http://127.0.0.1:8080}"
curl --fail --silent \
  -H 'Content-Type: application/json' \
  -d '{"command":"reset","arguments":{}}' \
  "$base_url/agent/command" >/dev/null
curl --fail --silent \
  -H 'Content-Type: application/json' \
  -d '{"command":"new_note","arguments":{}}' \
  "$base_url/agent/command" >/dev/null
initial="$(curl --fail --silent "$base_url/agent/ui")"
printf '%s' "$initial" | jq --exit-status \
  'any(.elements[]; .id == "main.input" and .role == "textbox") and
   any(.elements[]; .id == "main.submit" and .role == "button" and .enabled == false)' >/dev/null

disabled_submit_response="$(curl --silent --show-error \
  -H 'Content-Type: application/json' \
  -d '{"action":"click","id":"main.submit"}' \
  -w '\n%{http_code}' \
  "$base_url/agent/ui/action")"
disabled_submit_status="${disabled_submit_response##*$'\n'}"
disabled_submit_body="${disabled_submit_response%$'\n'*}"
[[ "$disabled_submit_status" == "400" ]]
printf '%s' "$disabled_submit_body" | jq --exit-status \
  '.error.code == "invalid_payload" and (.error.message | contains("Element ist deaktiviert"))' >/dev/null

curl --fail --silent \
  -H 'Content-Type: application/json' \
  -d '{"action":"set_value","id":"main.input","value":"UI smoke"}' \
  "$base_url/agent/ui/action" >/dev/null
updated="$(curl --fail --silent "$base_url/agent/ui")"
printf '%s' "$updated" | jq --exit-status \
  'any(.elements[]; .id == "main.submit" and .role == "button" and .enabled == true)' >/dev/null

curl --fail --silent \
  -H 'Content-Type: application/json' \
  -d '{"action":"click","id":"help.about"}' \
  "$base_url/agent/ui/action" >/dev/null
opened="$(curl --fail --silent "$base_url/agent/ui")"
printf '%s' "$opened" | jq --exit-status \
  '.screen == "about" and any(.elements[]; .id == "about.dialog" and .role == "dialog") and
   any(.elements[]; .id == "about.close" and .role == "button" and .enabled == true) and
   any(.elements[]; .id == "main.input" and .enabled == false)' >/dev/null

blocked_input_response="$(curl --silent --show-error \
  -H 'Content-Type: application/json' \
  -d '{"action":"set_value","id":"main.input","value":"Nicht verfügbar"}' \
  -w '\n%{http_code}' \
  "$base_url/agent/ui/action")"
blocked_input_status="${blocked_input_response##*$'\n'}"
blocked_input_body="${blocked_input_response%$'\n'*}"
[[ "$blocked_input_status" == "400" ]]
printf '%s' "$blocked_input_body" | jq --exit-status \
  '.error.code == "invalid_payload" and (.error.message | contains("Element ist deaktiviert"))' >/dev/null

curl --fail --silent \
  -H 'Content-Type: application/json' \
  -d '{"action":"click","id":"about.close"}' \
  "$base_url/agent/ui/action" >/dev/null
closed="$(curl --fail --silent "$base_url/agent/ui")"
printf '%s' "$closed" | jq --exit-status \
  '.screen == "main" and all(.elements[]; .id != "about.dialog")' >/dev/null
