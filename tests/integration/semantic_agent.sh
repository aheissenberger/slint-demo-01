#!/usr/bin/env bash
set -euo pipefail

base_url="${AGENT_API_URL:-http://127.0.0.1:8080}"
curl --fail --silent "$base_url/health" | jq --exit-status '.status == "ok"' >/dev/null
curl --fail --silent \
  -H 'Content-Type: application/json' \
  -d '{"command":"new_note","arguments":{}}' \
  "$base_url/agent/command" >/dev/null
curl --fail --silent "$base_url/agent/state" | jq --exit-status \
  '.screen == "main" and .status == "bereit" and .busy == false and .error == null' >/dev/null
curl --fail --silent "$base_url/agent/ui" | jq --exit-status \
  '.screen == "main" and
   any(.elements[]; .id == "main.submit" and .role == "button") and
   any(.elements[]; .id == "notes.title" and .role == "textbox") and
   any(.elements[]; .id == "notes.save" and .role == "button" and .enabled == false)' >/dev/null

curl --fail --silent \
  -H 'Content-Type: application/json' \
  -d '{"action":"set_value","id":"notes.title","value":"E2E Notiz"}' \
  "$base_url/agent/ui/action" >/dev/null
curl --fail --silent \
  -H 'Content-Type: application/json' \
  -d '{"action":"set_value","id":"notes.body","value":"Temporärer Inhalt"}' \
  "$base_url/agent/ui/action" >/dev/null
curl --fail --silent \
  -H 'Content-Type: application/json' \
  -d '{"action":"click","id":"notes.save"}' \
  "$base_url/agent/ui/action" >/dev/null
curl --fail --silent "$base_url/agent/ui" | jq --exit-status \
  'any(.elements[]; .id == "notes.item.0" and .enabled == true and .accessible_label == "E2E Notiz") and
   any(.elements[]; .id == "notes.delete" and .enabled == true)' >/dev/null
curl --fail --silent \
  -H 'Content-Type: application/json' \
  -d '{"action":"click","id":"notes.delete"}' \
  "$base_url/agent/ui/action" >/dev/null
