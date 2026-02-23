# Windows Port Notes

This `windows/` tree is a Windows-compatible mirror of `mac/` with the same top-level structure and core feature set.

## What Was Ported

- Full project structure cloned from `mac/` into `windows/`.
- Rust core (`windows/core`) now compiles on Windows:
  - macOS-only modules are gated and replaced with Windows stubs where needed.
  - shell execution uses PowerShell on Windows.
  - process termination uses `taskkill` on Windows.
  - clipboard/window/app helpers include Windows fallbacks.
- Tauri desktop (`windows/desktop/src-tauri`) now builds on Windows (`cargo check`).
- Windows entry scripts were added:
  - `windows/scripts/run_local.ps1`
  - `windows/scripts/run_collector.ps1`
  - `windows/scripts/run_pipeline_rs.ps1`
  - `windows/scripts/run_web.ps1`
  - `windows/scripts/build_release.ps1`

## Quick Start (Windows)

1. Collector/core run:

```powershell
powershell -ExecutionPolicy Bypass -File windows/scripts/run_local.ps1
```

2. Pipeline run:

```powershell
powershell -ExecutionPolicy Bypass -File windows/scripts/run_pipeline_rs.ps1
```

3. Web dev server:

```powershell
powershell -ExecutionPolicy Bypass -File windows/scripts/run_web.ps1 -Mode dev -Install
```

4. Desktop check:

```powershell
cd windows/desktop/src-tauri
cargo check
```

## Known Gaps

- macOS-native Accessibility tree depth and deterministic element binding are still stronger on macOS than on Windows fallback mode.
- Some scenario scripts are still shell-first (`.sh`) and may require PowerShell equivalents if you want full Windows-only operations end-to-end.
