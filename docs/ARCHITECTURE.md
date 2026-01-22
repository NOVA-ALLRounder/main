# System Architecture

## 1. High-Level Design
`[LLM] <-> [Rust Core (Broker)] <-> [Swift Adapter] <-> [OS APIs]`

## 2. Component Responsibility

### A. Rust Core (The Brain)
* **Role:** 상태 관리, 보안 정책 집행, LLM 컨텍스트 관리.
* **Reasoning:** Swift Adapter는 로직을 갖지 않으며, 오직 Rust의 명령만 수행.
* **State Machine:**
  1. **Observe:** 현재 화면/상태 스냅샷 요청.
  2. **Decide:** LLM에 상황 전달 및 Plan 수립.
  3. **Act:** 승인된 Action을 Adapter로 전송.
  4. **Verify:** Action 결과 확인 (Diff/State Check).

### B. Swift Adapter (The Hands)
* **Role:** macOS Native API 래핑 및 실행.
* **Scope:**
  * `AXUIElement`: UI 요소 탐색 및 제어.
  * `CGEvent`: 마우스/키보드 하드웨어 이벤트 시뮬레이션.
  * `Quartz`: 스크린 캡처.

## 3. IPC Protocol
* JSON-RPC over Stdio (Standard Input/Output) 방식을 기본으로 함.
* **Request:** `{ "method": "ui.click", "params": { "id": "btn_submit" } }`
* **Response:** `{ "status": "success", "data": { ... } }`
