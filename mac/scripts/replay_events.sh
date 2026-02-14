#!/usr/bin/env bash
set -euo pipefail

API_URL="${API_URL:-http://localhost:5680/events}"
COUNT="${COUNT:-5}"

ts_now() {
  date -u +"%Y-%m-%dT%H:%M:%SZ"
}

uuid() {
  if command -v uuidgen >/dev/null 2>&1; then
    uuidgen
  else
    # Fallback UUID (not cryptographically strong)
    python3 - <<'PY'
import uuid
print(uuid.uuid4())
PY
  fi
}

send_event() {
  local payload="$1"
  curl -s -X POST -H "Content-Type: application/json" -d "$payload" "$API_URL" >/dev/null || true
}

echo "[replay] sending app switch flow..."
for i in $(seq 1 "$COUNT"); do
  send_event "{
    \"schema_version\":\"1.0\",
    \"event_id\":\"$(uuid)\",
    \"ts\":\"$(ts_now)\",
    \"source\":\"replay\",
    \"app\":\"Slack\",
    \"event_type\":\"app_switch\",
    \"priority\":\"P2\",
    \"resource\":{\"type\":\"app\",\"id\":\"Slack\"},
    \"payload\":{\"app\":\"Slack\",\"window_title\":\"Inbox\",\"browser_url\":\"https://mail.google.com\"}
  }"
  send_event "{
    \"schema_version\":\"1.0\",
    \"event_id\":\"$(uuid)\",
    \"ts\":\"$(ts_now)\",
    \"source\":\"replay\",
    \"app\":\"Chrome\",
    \"event_type\":\"app_switch\",
    \"priority\":\"P2\",
    \"resource\":{\"type\":\"app\",\"id\":\"Chrome\"},
    \"payload\":{\"app\":\"Chrome\",\"window_title\":\"Docs\",\"browser_url\":\"https://docs.google.com\"}
  }"
done

echo "[replay] sending file activity..."
for i in $(seq 1 "$COUNT"); do
  send_event "{
    \"schema_version\":\"1.0\",
    \"event_id\":\"$(uuid)\",
    \"ts\":\"$(ts_now)\",
    \"source\":\"replay\",
    \"app\":\"Finder\",
    \"event_type\":\"file_created\",
    \"priority\":\"P2\",
    \"resource\":{\"type\":\"file\",\"id\":\"/Users/test/Downloads/report${i}.pdf\"},
    \"payload\":{\"path\":\"/Users/test/Downloads/report${i}.pdf\",\"filename\":\"report${i}.pdf\"}
  }"
done

echo "[replay] sending keyword activity..."
for i in $(seq 1 "$COUNT"); do
  send_event "{
    \"schema_version\":\"1.0\",
    \"event_id\":\"$(uuid)\",
    \"ts\":\"$(ts_now)\",
    \"source\":\"replay\",
    \"app\":\"Mail\",
    \"event_type\":\"key_input\",
    \"priority\":\"P2\",
    \"resource\":{\"type\":\"input\",\"id\":\"keyboard\"},
    \"payload\":{\"text\":\"invoice follow-up\"}
  }"
done

echo "[replay] done. (API_URL=$API_URL)"
