# Product Specification

## 1. Project Overview
사용자의 로컬 OS(macOS)를 대상으로 자연어 명령을 해석하여 실제 실행(Execution), 검증(Verification), 복구(Recovery)를 수행하는 전역 권한 에이전트.

## 2. Core Value Proposition
* **Execution over Chat:** 단순 조언이 아닌 실제 OS 조작 수행.
* **Safety First:** LLM은 OS를 직접 제어하지 않으며, 엄격한 Broker를 통해서만 수행.
* **Closed Loop:** 실행 후 반드시 검증(Verify) 과정을 거침.

## 3. Mandatory Constraints
* **OS Independence (Logic):** 핵심 로직은 Rust로 작성하여 플랫폼 종속성 최소화.
* **OS Dependence (Action):** 실제 제어는 Native API(Swift/Accessibility)를 사용.
* **User Control:** 사용자는 언제든 에이전트의 동작을 강제 중단할 수 있어야 함.

## 4. MVP Roadmap
* **Phase 1:** Read-only Agent (Screen Capture, Accessibility Tree 조회) + Basic Mouse Move.
* **Phase 2:** Controlled Act (Click, Type) + Verify Loop + Write Lock.
* **Phase 3:** Full Policy Engine (Allowlist/Denylist) + Complex Planning.
