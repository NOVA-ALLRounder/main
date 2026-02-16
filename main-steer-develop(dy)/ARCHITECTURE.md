# Steer OS Agent Architecture

This document provides a high-level overview of the Steer Local OS Agent's architecture.

## Core Technology Stack

| Layer        | Technology | Purpose                               |
|--------------|------------|---------------------------------------|
| **Core Engine** | Rust       | Main application logic, LLM integration, database, OS automation |
| **Desktop UI**  | Tauri + HTML/JS | Lightweight native window hosting the web UI |
| **Legacy Scripts** | Python | Auxiliary utilities (e.g., `launch_desktop.py`) |

## Project Structure

```
local-os-agent/
├── core/               # Rust library (main logic)
│   └── src/
│       ├── lib.rs          # Module exports
│       ├── llm_gateway.rs  # LLM API communication
│       ├── db.rs           # SQLite persistence
│       ├── executor.rs     # Shell/Goal execution
│       ├── privacy.rs      # PII masking
│       ├── policy.rs       # Action security classification
│       ├── api_server.rs   # Axum REST API
│       └── ...
├── desktop/            # Deprecated/Legacy (Electron, no longer primary)
├── tauri/              # Tauri App (Rust + WebView)
│   └── src/
│       └── main.rs     # Tauri commands, integrates `core`
├── web/                # Web UI (HTML/CSS/JS or React)
├── scripts/
│   └── install_mac.sh  # macOS setup script
└── tests/              # Integration/Unit tests
```

## Rust vs Python

- **Rust (`core/`)**: The **Single Source of Truth (SSOT)** for all application logic. All new features should be implemented here.
- **Python (`scripts/`, root `.py` files)**: Used only for legacy utilities or one-off scripting tasks. Python code should NOT duplicate logic present in Rust.

## Security Model

Security is enforced at multiple layers:

1.  **`tool_policy.rs`**: Allow/Denylist for tool categories (env vars: `TOOL_ALLOWLIST`, `TOOL_DENYLIST`).
2.  **`policy.rs`**: `PolicyEngine` classifies actions as `Safe`, `Caution`, or `Critical`. A `write_lock` flag (default ON) blocks `Caution` actions until unlocked.
3.  **`security.rs`**: `CommandClassifier` for shell commands (e.g., `rm -rf /` -> `Critical`).
4.  **`api_server.rs`**: `auth_middleware` checks for `STEER_API_KEY` if set.
5.  **`privacy.rs`**: `PrivacyGuard` masks PII (emails, credit cards) before database storage.

## Data Flow

1.  **User Input**: Web UI -> Tauri IPC -> `core::api_server` OR direct LLM interaction.
2.  **LLM Interaction**: `llm_gateway.rs` handles all OpenAI API calls with retry logic.
3.  **Action Execution**: `executor.rs` runs shell/UI actions, checked by `policy.rs`.
4.  **Persistence**: All events/routines stored in `~/.local/share/steer/steer.db` via `db.rs`.
