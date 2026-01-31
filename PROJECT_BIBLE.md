# PROJECT_BIBLE.md

## 1. Project Overview

* **Project Name:** Local OS Super Agent
* **Role:** System-wide Execution Agent (Local)
* **Core Philosophy:** "LLM plans, Rust brokers, Swift executes."
* **Critical Constraint:** LLM은 OS를 직접 제어하지 않으며, 모든 명령은 **상태 머신**과 **보안 정책**을 통과해야 한다.

---

## 2. Directory Structure

```text
local-os-agent/
├── docs/
│   ├── SPEC.md                 # 기능 명세
│   ├── ARCHITECTURE.md         # 시스템 설계
│   ├── SECURITY.md             # 보안 정책 (Kill Switch, Write Lock)
│   └── TOOL_INTERFACE.md       # JSON Schema 정의
├── core/ (Rust)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # Entry Point
│       ├── schema.rs           # IPC Data Models
│       ├── state_machine.rs    # Observe-Decide-Act Loop
│       ├── policy.rs           # Permission Check
│       └── ipc.rs              # Stdio Communication
└── adapter/ (Swift)
    ├── Package.swift
    └── Sources/
        ├── Main.swift          # Entry Point & Loop
        ├── Schema.swift        # JSON Decoding
        ├── ElementRegistry.swift # AXUIElement ID Management
        ├── AccessibilityService.swift # Screen Crawler
        ├── ActionExecutor.swift # AXPress & Fallback
        └── KillSwitch.swift    # Emergency Stop

```

---

## 3. Security Specification (`docs/SECURITY.md`)

### A. Zero Trust Architecture

* 모든 LLM의 출력은 기본적으로 "신뢰할 수 없음(Untrusted)"으로 간주한다.
* `Act` 단계 진입 전 반드시 `Authorizing` 상태를 거쳐야 한다.

### B. Write Lock & 2FA

* **Safe Actions:** `ui.snapshot`, `ui.find` (자동 승인)
* **Write Actions:** `ui.click`, `keyboard.type` (Write Lock 해제 필요)
* **Critical Actions:** `file.delete`, `app.terminate` (사용자 명시적 승인 필수)

### C. Kill Switch (Fail-Safe)

* **Trigger:** `Cmd + Option + Esc` (Global Hotkey)
* **Behavior:** Swift Adapter 프로세스 즉시 종료 (`exit(1)`).

---

## 4. Core Implementation (Rust)

### `core/Cargo.toml`

```toml
[package]
name = "core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"

```

### `core/src/schema.rs` (The Contract)

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum AgentAction {
    // Observe
    UiSnapshot { scope: Option<String> },
    UiFind { query: String },
    
    // Act
    UiClick { element_id: String, double_click: bool },
    KeyboardType { text: String, submit: bool },
    
    // System
    Terminate,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentCommand {
    pub id: String,
    #[serde(flatten)]
    pub action: AgentAction,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentResponse {
    pub request_id: String,
    pub status: String, // "success", "fail"
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

```

### `core/src/state_machine.rs` (The Brain)

```rust
use crate::schema::AgentAction;

#[derive(Debug)]
pub enum AgentState {
    Idle,
    Observing,
    Deciding { snapshot: serde_json::Value },
    Authorizing { pending_action: AgentAction },
    Acting { approved_action: AgentAction },
    Verifying { executed_action: AgentAction },
    Terminated { reason: String },
}

pub struct AgentCore {
    state: AgentState,
    // policy_engine: PolicyEngine,
}

impl AgentCore {
    pub async fn run_cycle(&mut self) {
        // Loop logic implementation (Refer to Step 3 logic)
        // Observe -> LLM Call -> Policy Check -> Act -> Verify -> Loop
    }
}

```

---

## 5. Adapter Implementation (Swift)

### `adapter/Sources/Schema.swift`

```swift
import Foundation

struct AgentRequest: Decodable {
    let id: String
    let action: String
    // Payload decoding logic requires manual implementing or AnyDecodable
}

struct AgentResponse: Encodable {
    let request_id: String
    let status: String
    let data: AnyCodable?
    let error: String?
}

```

### `adapter/Sources/ElementRegistry.swift`

```swift
import Cocoa

class ElementRegistry {
    static let shared = ElementRegistry()
    private var cache: [String: AXUIElement] = [:]
    
    func register(_ element: AXUIElement) -> String {
        let id = UUID().uuidString
        cache[id] = element
        return id
    }
    
    func getElement(by id: String) -> AXUIElement? {
        return cache[id]
    }
    
    func clear() { cache.removeAll() }
}

```

### `adapter/Sources/ActionExecutor.swift`

```swift
import ApplicationServices

class ActionExecutor {
    func executeClick(elementId: String) throws {
        guard let element = ElementRegistry.shared.getElement(by: elementId) else {
            throw NSError(domain: "Agent", code: 404, userInfo: [NSLocalizedDescriptionKey: "Element ID stale"])
        }
        
        // 1. Semantic Click (AXPress)
        let error = AXUIElementPerformAction(element, kAXPressAction as CFString)
        if error == .success { return }
        
        // 2. Fallback: Coordinate Click
        try fallbackClick(element)
    }
    
    private func fallbackClick(_ element: AXUIElement) throws {
        // Logic to get Position/Size and trigger CGEvent
    }
}

```

### `adapter/Sources/KillSwitch.swift`

```swift
import Cocoa

class KillSwitch {
    func startMonitoring() {
        NSEvent.addGlobalMonitorForEvents(matching: .keyDown) { event in
            // Cmd + Option + Esc (KeyCode 53)
            if event.modifierFlags.contains([.command, .option]) && event.keyCode == 53 {
                print("🚨 Kill Switch Triggered. Exiting...")
                exit(1)
            }
        }
    }
}

```

---

## 6. Implementation Checklist (Next Steps)

1. **Repository Setup:** `git init` 및 상기 디렉터리 구조 생성.
2. **Swift Sandbox:** 가상 머신(Tart/UTM) 설치 및 macOS 이미지 준비.
3. **Permissions:** 터미널 및 IDE에 "손쉬운 사용(Accessibility)" 권한 부여.
4. **Unit Test:** `ElementRegistry`가 UUID를 제대로 생성하고 반환하는지 테스트.
5. **Integration Test:** Rust에서 `UiSnapshot` 명령을 보내고 Swift가 JSON 트리를 반환하는지 확인.

---

### End of Document

**Vibe Coding AI Assistant V1.1 (Strict Mode)**
설계와 명세 작업이 모두 완료되었습니다.
