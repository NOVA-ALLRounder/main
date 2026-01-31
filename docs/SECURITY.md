# Security Policy

## 1. Zero Trust Principle
* LLM이 생성한 명령은 기본적으로 "위험하다"고 간주한다.
* 모든 명령은 실행 전 Policy Engine을 통과해야 한다.

## 2. Write Lock Mechanism
* **Default State:** `LOCKED` (Read-only).
* **Unlock Trigger:** 사용자의 명시적 승인(CLI Confirm or UI Button).
* **Auto-Lock:** 마지막 Action 후 N초 경과 시 자동 잠금.

## 3. Action Classification
| Level | Actions | Requirement |
|-------|---------|-------------|
| **Safe** | `ui.find`, `screen.capture`, `app.list` | 자동 승인 |
| **Caution** | `ui.click`, `keyboard.type` | Write Lock 해제 필요 |
| **Critical** | `file.delete`, `cmd.exec`, `submit.payment` | **건별 사용자 승인 (2FA/Confirm)** |

## 4. Fail-Safe
* `Esc` 키 연타 또는 특정 핫키 입력 시 Swift Adapter 프로세스 즉시 종료 (Kill Switch).
