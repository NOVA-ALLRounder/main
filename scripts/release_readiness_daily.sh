#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

SAVE_BASELINE=false
if [[ "${1:-}" == "--save-baseline" ]]; then
  SAVE_BASELINE=true
fi

TIMESTAMP="$(date '+%Y%m%d-%H%M%S')"

echo "==> Running live HTTP E2E"
cargo run --manifest-path core/Cargo.toml --bin http_e2e

echo "==> Running release readiness"
CMD=(cargo run --manifest-path core/Cargo.toml --bin release_readiness --)
if [[ "$SAVE_BASELINE" == true ]]; then
  CMD+=(--save-baseline)
fi

"${CMD[@]}"

python - <<'PY'
import json
from pathlib import Path

report = json.loads(Path("reports/release_readiness/latest.json").read_text())
http_e2e = report.get("http_e2e") or {}
print(f"status={report['status']}")
print(f"ready_for_launch={report['ready_for_launch']}")
print(f"candidate_snapshot={report['candidate_snapshot']['scenario_count']}")
print(f"launch_eval={report['launch_eval']['passed']}/{report['launch_eval']['total']}")
print(f"http_e2e={http_e2e.get('passed','n/a')}/{http_e2e.get('total','n/a')}")
print(f"release_readiness_history={report.get('archived_history_json_path') or 'n/a'}")
print(f"launch_eval_history={report.get('archived_launch_eval_json_path') or 'n/a'}")
trend = report.get("history_trend") or {}
print(f"trend_summary={trend.get('summary') or 'n/a'}")
PY
