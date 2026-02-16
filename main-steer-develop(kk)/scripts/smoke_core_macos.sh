#!/usr/bin/env bash
set -euo pipefail

API_BASE="${1:-http://127.0.0.1:5680}"

echo "[smoke] checking ${API_BASE}/api/health"
health="$(curl -fsS "${API_BASE}/api/health")"
if [[ "${health}" != "ok" ]]; then
  echo "health check failed: ${health}"
  exit 1
fi
echo "[ok] health"

echo "[smoke] checking ${API_BASE}/api/context/selection"
selection="$(curl -fsS "${API_BASE}/api/context/selection")"
echo "${selection}" | grep -q "\"found\"" || { echo "selection payload missing 'found'"; exit 1; }
echo "[ok] selection endpoint"

echo "[smoke] checking ${API_BASE}/api/system/health"
curl -fsS "${API_BASE}/api/system/health" >/dev/null
echo "[ok] system health endpoint"

echo "[done] core api smoke passed"
