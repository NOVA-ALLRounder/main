# Local OS Agent

A system-wide execution agent for macOS, designed with a strict "Observe-Decide-Act" loop.
Built with **Rust** (Core logic & Safety) and **Swift** (Native OS Control).

## 🚀 Getting Started

### Prerequisites
*   **macOS**: 12.0+
*   **Rust Toolchain**: `cargo` (Install via `rustup`)
*   **Swift Toolchain**: Xcode 14+ or Swift 5.7+
*   **Permissions**: Accessibility API access (System Settings -> Privacy & Security -> Accessibility)

### Buildup

1.  **Clone & Init**
    ```bash
    git clone <repo>
    cd local-os-agent
    ```

2.  **Build Swift Adapter**
    ```bash
    cd adapter
    swift build -c release
    cd ..
    ```

3.  **Run Rust Core**
    ```bash
    cd core
    cargo run
    ```

## 🏗 Project Structure

See [PROJECT_BIBLE.md](./PROJECT_BIBLE.md) for detailed architecture and specs.

*   `core/`: Rust logic (State Machine, Policy Engine, IPC).
*   `adapter/`: Swift executable (Accessibility, AXPress, CGEvent).
*   `docs/`: Specifications and Security Policies.

## 🔒 Security

*   **Write Lock**: Enabled by default. Actions like Click/Type are blocked unless explicitly unlocked.
*   **Kill Switch**: Press `Cmd + Option + Esc` to immediately kill the adapter process.
