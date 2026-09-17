#!/usr/bin/env bash
set -euo pipefail

base_url="${AGENT_API_URL:-http://127.0.0.1:8080}"
curl --fail --silent "$base_url/health" | jq --exit-status '.status == "ok"' >/dev/null
curl --fail --silent "$base_url/agent/state" | jq --exit-status \
  '.screen == "main" and .status == "bereit" and .busy == false and .error == null' >/dev/null
curl --fail --silent "$base_url/agent/ui" | jq --exit-status \
  '.screen == "main" and any(.elements[]; .id == "main.submit" and .role == "button")' >/dev/null
