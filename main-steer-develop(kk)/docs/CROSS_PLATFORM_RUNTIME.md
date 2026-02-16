# Cross Platform Runtime Notes

## Environment bootstrap
- Core now auto-loads environment values from:
1. process default `.env`
2. `core/.env`
3. parent-directory `.env` fallbacks
- Use `core/.env.example` as template.

## Path behavior
- Runtime data root:
1. `STEER_HOME` (if set)
2. platform home + `/.steer`
- Temporary files use platform temp directory (no hardcoded `/tmp`).

## Windows compatibility mode
- `windows/events` now emits `app_switch` events from active-window polling.
- `windows/actions` now retries click/type for transient focus issues.
- `monitor::spawn_app_watcher` is OS-aware:
1. macOS: AppleScript path
2. Windows: foreground-window polling path

## Smoke tests
- Windows:
`powershell -ExecutionPolicy Bypass -File scripts/smoke_core_windows.ps1`
- macOS:
`bash scripts/smoke_core_macos.sh`
