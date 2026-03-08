# Allvia Release Readiness Runbook

이 문서는 `프롬프트.md` 기준으로 정리한 한 달 출시 운영 런북이다.

기준일: `2026-03-08`

## 현재 상태

- 코드/게이트 기준 상태: `ready_for_launch: true`
- readiness 상태: `ready`
- 남은 advisory: 없음
- 대표 회귀셋: `launch_eval 44/44 passed`
- 실사용 후보 snapshot: `20`
- NL run metrics: `16 total / 13 completed / 3 approval_required / 0 error`

즉 현재 구조적 blocker는 없다. 남은 과제는 closed beta 동안 이 상태가 유지되는지 추세를 검증하는 것이다.

## 매일 해야 하는 것

### 1. readiness 리포트 생성

```bash
bash scripts/release_readiness_daily.sh
```

기본 daily run은 `read-only`다. 즉 baseline을 덮어쓰지 않는다.
Dashboard의 `Run release readiness`도 같은 원칙으로 `read-only`여야 한다.
또한 launch/release/http E2E 운영 API는 기본적으로 서버의 현재 workdir만 사용해야 한다. 클라이언트가 보낸 `workdir`는 내부 smoke/test override가 아닌 한 무시된다.
`Set release baseline` 역시 서버 고정 baseline request로만 저장되어야 한다. 클라이언트가 `config/path/limit`를 덮어쓰는 요청은 기본적으로 거절되어야 한다.
이 스크립트는 항상 먼저 `http_e2e`를 실행해 최신 live HTTP smoke 결과를 만든 뒤,
그 결과를 포함해 `release_readiness`를 계산한다.
또한 각 run은 자동으로 `reports/release_readiness/history/<timestamp>/`와
`reports/launch_eval/history/<timestamp>/`에 archive를 남긴다.

baseline 저장은 좋은 상태를 사람이 확인한 뒤에만 명시적으로 수행한다.

```bash
bash scripts/release_readiness_daily.sh --save-baseline
```

확인 파일:

- `reports/release_readiness/latest.json`
- `reports/release_readiness/latest.md`
- `reports/launch_eval/latest.json`
- `reports/launch_eval/latest.md`
- `reports/http_e2e/latest.json`
- `reports/http_e2e/latest.md`

### 1.5. live HTTP smoke 확인

```bash
cargo run --manifest-path core/Cargo.toml --bin http_e2e
```

이 smoke는 실제 Axum 서버를 띄우고 아래를 끝까지 검증한다.

- `/api/health`
- `/api/system/db-paths`
- `/api/chat` local help
- request memory 재사용
- `ai_digest` auto-route
- recommendation `later` review action
- recommendation review audit trail
- `/api/launch/ops` metrics

### 2. launch ops 확인

Settings 또는 Dashboard에서 아래를 확인한다.

- error rate
- low-confidence routes
- request memory hit
- execution memory hit
- ai digest fallback
- recommendation approval quality
- recommendation review friction
- visible auto recommendation queue
- release readiness trend summary
- HTTP E2E drift versus the previous run
- live HTTP checks for `http_e2e latest/history` and `release_readiness latest/history`

review friction은 release gate에서 warning으로도 반영된다. 즉 approve/reject/later/feedback 흐름이 악화되면 `ready`를 유지하더라도 운영 경고가 쌓이게 된다.
- recommendation review audit events

### 3. closed beta 로그 확인

매일 아래를 확인한다.

- 실제 `NL run total`
- 실패한 자연어 실행 수
- approval required backlog
- dislike/negative feedback가 붙은 cache reuse
- reject/refine 된 recommendation
- approve/reject/later/restore 이력과 actor가 서버 감사 로그에 남는지

## 주차별 계획

### 1주차: Closed Beta 시작

- 내부/가까운 사용자로 실제 업무 요청을 받는다.
- 목표:
  - `NL run total >= 10`
  - `error = 0` 또는 즉시 원인 파악 가능
  - `low-confidence route`가 반복되지 않음
- 매일 read-only `release_readiness` 리포트를 저장한다.

### 2주차: 로그 기반 정밀 보정

- `request_memory` 잘못된 재사용 사례 정리
- `execution_memory` freshness/param mismatch 사례 정리
- recommendation reject/refine 사유 확인
- `configs/launch_eval.generated.yaml` 후보 품질 점검

### 3주차: Work-only 베타 안정화

- 업무 패턴 추천만 실제 approve/reject 흐름으로 검증
- 목표:
  - 추천이 적고 설명 가능해야 함
  - weak recommendation이 pending queue를 오염시키지 않아야 함
- 승인된 workflow의 실제 사용 여부를 같이 본다.

### 4주차: Launch Freeze

- 새 기능 추가 중단
- regressions, ops drift, write-action safety만 본다
- launch day 전 마지막 3일은 `release_readiness`와 핵심 시나리오 재검증만 반복한다.

## 출시 게이트

출시 직전 아래를 모두 만족해야 한다.

- `ready_for_launch = true`
- `blockers = []`
- `launch_eval failed = 0`
- `http_e2e passed = total`
- `candidate snapshot >= 5`
- `NL run total >= 10`
- `launch ops error rate = 0%` 또는 원인/완화책이 명확
- `low-confidence routes`가 상위 문제로 남지 않음
- visible auto recommendation queue가 과도하지 않음

## 중단 조건

아래 중 하나라도 발생하면 출시를 중단한다.

- `release_readiness`가 `not_ready` 또는 `needs_data`로 회귀
- `launch_eval` 실패 발생
- `http_e2e` 실패 발생
- cross-scope memory reuse
- stale cache reuse로 잘못된 동적 응답 제공
- write action evidence 부족 상태에서 외부 전송 발생
- recommendation spam 또는 explainability 부족

## Launch Day 절차

1. `cargo test --manifest-path core/Cargo.toml`
2. `npm run build` in `web/`
3. `bash scripts/release_readiness_daily.sh`
4. `cargo run --manifest-path core/Cargo.toml --bin http_e2e`
5. `reports/release_readiness/latest.md` 확인
6. Dashboard `Quality & Gate` 확인
7. Settings `Launch Readiness Ops` 확인
   - `Recent Recommendation Review Events` 포함
8. 배포

## 배포 후 24시간 모니터링

- 2시간 간격으로 `bash scripts/release_readiness_daily.sh`
- launch ops error/low-confidence 확인
- recommendation feedback 급증 여부 확인
- write action backlog 확인

## 지금 가장 중요한 것

지금 Allvia의 남은 핵심 과제는 새 기능이 아니다.

- 실제 사용자 자연어 실행 표본을 `10개 이상`
- 그 과정에서 `launch_ops_events`, `request_memory`, `execution_memory`, `recommendation feedback`
- 이 네 축을 안정적으로 쌓는 것

이게 되면 현재 코드 베이스는 한 달 내 출시 가능한 수준으로 유지할 수 있다.
