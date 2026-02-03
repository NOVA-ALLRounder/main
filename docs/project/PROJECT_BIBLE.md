# PROJECT_BIBLE.md

## 1. Project Overview

* **Project Name:** Local OS Super Agent
* **Role:** System-wide Execution Agent (Local)
* **Core Philosophy:** "LLM plans, Rust brokers, Windows Adapter executes."
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
├── apps/core/ (Rust)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # Entry Point
│       ├── schema.rs           # IPC Data Models
│       ├── state_machine.rs    # Observe-Decide-Act Loop
│       ├── policy.rs           # Permission Check
│       └── ipc.rs              # Stdio Communication
└── adapter/ (Windows)
    ├── Adapter.csproj
    └── Sources/
        ├── Main.cs             # Entry Point & Loop
        ├── Schema.cs           # JSON Decoding
        ├── ElementRegistry.cs  # UIA Element ID Management
        ├── AutomationService.cs # Screen Crawler
        ├── ActionExecutor.cs   # InvokePattern & Fallback
        └── KillSwitch.cs       # Emergency Stop

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

* **Trigger:** `Ctrl + Shift + Esc` (Global Hotkey)
* **Behavior:** Windows Adapter 프로세스 즉시 종료 (`Environment.Exit(1)`).

---

## 4. Core Implementation (Rust)

### `apps/core/Cargo.toml`

```toml
[package]
name = "local_os_agent"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"

```

### `apps/core/src/schema.rs` (The Contract)

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

### `apps/core/src/state_machine.rs` (The Brain)

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

## 5. Adapter Implementation (Windows/C#)

### `adapter/Sources/Schema.cs`

```csharp
using System.Text.Json;

public record AgentRequest(string id, string action, JsonElement? payload);
public record AgentResponse(string request_id, string status, JsonElement? data, string? error);
```

### `adapter/Sources/ElementRegistry.cs`

```csharp
using System.Collections.Generic;
using System.Windows.Automation;

public sealed class ElementRegistry {
    private readonly Dictionary<string, AutomationElement> _cache = new();
    public string Register(AutomationElement element) {
        var id = Guid.NewGuid().ToString();
        _cache[id] = element;
        return id;
    }

    public AutomationElement? Get(string id) => _cache.TryGetValue(id, out var el) ? el : null;
    public void Clear() => _cache.Clear();
}
```

### `adapter/Sources/ActionExecutor.cs`

```csharp
using System.Windows.Automation;

public sealed class ActionExecutor {
    private readonly ElementRegistry _registry;

    public ActionExecutor(ElementRegistry registry) {
        _registry = registry;
    }

    public void ExecuteClick(string elementId) {
        var element = _registry.Get(elementId) ?? throw new Exception("Element ID stale");
        if (element.TryGetCurrentPattern(InvokePattern.Pattern, out var pattern)) {
            ((InvokePattern)pattern).Invoke();
            return;
        }
        // Fallback: SendInput / mouse click by coordinates
        throw new Exception("InvokePattern not available");
    }
}
```

### `adapter/Sources/KillSwitch.cs`

```csharp
// P/Invoke RegisterHotKey and exit on Ctrl+Shift+Esc
public sealed class KillSwitch {
    public void StartMonitoring() {
        // Register global hotkey and call Environment.Exit(1) on trigger
    }
}
```

---

## 6. Implementation Checklist (Next Steps)

1. **Repository Setup:** `git init` 및 상기 디렉터리 구조 생성.
2. **Windows Sandbox:** 가상 머신(Hyper-V/VMware) 설치 및 Windows 이미지 준비.
3. **Permissions:** 관리자 권한 실행 및 UI Automation 접근 확인.
4. **Unit Test:** `ElementRegistry`가 UUID를 제대로 생성하고 반환하는지 테스트.
5. **Integration Test:** Rust에서 `UiSnapshot` 명령을 보내고 Windows Adapter가 JSON 트리를 반환하는지 확인.

---

### End of Document

**Vibe Coding AI Assistant V1.1 (Strict Mode)**
설계와 명세 작업이 모두 완료되었습니다.
