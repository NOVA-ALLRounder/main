# Phase 0 상세 설계서 (Natural Language Automation)

## 1) 범위 (MVP)
- 자연어 입력 → 의도/슬롯 → 플랜 → 실행 → 검증 → 승인 파이프라인 정의
- 3개 기능: 항공권/숙박 검색, 가격비교 쇼핑, 웹폼 자동작성
- 결제/제출은 **승인 후 수동**

## 2) 비범위
- 자동 결제/자동 제출
- 민감정보(카드/주민번호) 자동 입력
- 다중 브라우저/멀티프로필 동시 실행

## 3) 의도(Intent) 정의
- `FlightSearch`
- `ShoppingCompare`
- `FormFill`
- `GenericTask` (fallback)

## 4) 슬롯(Slot) 정의
### 공통
- `locale` (optional)
- `budget_max` (optional)
- `time_pref` (optional)

### 항공권/숙박
- 필수: `from`, `to`, `date_start`
- 선택: `date_end`, `trip_type`, `passengers`, `direct_only`, `time_window`

### 쇼핑 비교
- 필수: `product_name`
- 선택: `brand`, `variant`, `price_min`, `price_max`, `shipping_pref`

### 웹폼 자동작성
- 필수: `form_purpose`
- 선택: `profile_id`, `fields_override`

## 5) 플랜(Plan) 스키마
```json
{
  "plan_id": "uuid",
  "intent": "FlightSearch",
  "steps": [
    {"type": "navigate", "url": "https://www.google.com/travel/flights"},
    {"type": "fill", "selector": "...", "value": "ICN"},
    {"type": "fill", "selector": "...", "value": "NRT"},
    {"type": "click", "selector": "..."},
    {"type": "extract", "selector": "...", "fields": ["price", "time", "stops"]}
  ]
}
```

## 6) 실행 단계 타입
- `navigate` (URL 이동)
- `click` (요소 클릭)
- `fill` (입력)
- `select` (드롭다운)
- `wait` (대기)
- `extract` (데이터 추출)
- `screenshot` (검증용)

## 7) 승인 정책
- 결제/제출/계정 변경 → **항상 승인**
- 탐색/요약 → 자동 진행 가능
- 민감정보 입력 → 금지 또는 승인 필요

## 8) 검증 규칙
- 결과 리스트가 존재하는지
- 가격/시간 정보가 추출되는지
- 필수 슬롯이 실제로 반영되었는지

## 9) 에러 분류
- `permission_error`
- `timeout_error`
- `selector_not_found`
- `navigation_failed`
- `validation_failed`

## 10) API 초안
- `POST /api/agent/intent`
  - Input: `{ text: string }`
  - Output: `{ intent: string, confidence: number, slots: object }`

- `POST /api/agent/plan`
  - Input: `{ intent: string, slots: object }`
  - Output: `{ plan_id: string, steps: [] }`

- `POST /api/agent/execute`
  - Input: `{ plan_id: string }`
  - Output: `{ status: string, logs: [] }`

- `POST /api/agent/verify`
  - Input: `{ plan_id: string }`
  - Output: `{ ok: boolean, issues: [] }`

- `POST /api/agent/approve`
  - Input: `{ plan_id: string, action: string }`
  - Output: `{ status: string }`

## 11) DB 테이블 초안
- `sessions(id, created_at, goal, status)`
- `intents(session_id, intent_type, confidence, raw_text)`
- `slots(session_id, key, value, status)`
- `plans(session_id, steps_json, status)`
- `executions(session_id, step_id, action, result, logs)`
- `verifications(session_id, ok, issues_json)`
- `approvals(session_id, action, status, approved_at)`
- `summaries(session_id, text, recommendations_json)`

## 12) 완료 기준 (Phase 0)
- 의도/슬롯/플랜/승인 정책 문서화 완료
- API/DB 초안 합의
- 실패 분류와 재시도 전략 정의

