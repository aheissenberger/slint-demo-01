#!/usr/bin/env bash
set -euo pipefail

base_url="${AGENT_API_URL:-http://127.0.0.1:8080}"
curl --fail --silent "$base_url/health" | grep -q '"status":"ok"'
curl --fail --silent "$base_url/agent/state" | grep -q '"screen":"main"'
curl --fail --silent "$base_url/agent/ui" | grep -q '"id":"main.submit"'
