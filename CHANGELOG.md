# Changelog

All notable changes to the Steer project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Migration guide for Tool → Skill system (docs/MIGRATION_GUIDE.md)
- New SkillRegistry architecture with better extensibility
- Enhanced command parser with skill-based routing
- Deprecation warnings for legacy Tool system

### Changed
- **[BREAKING]** CommandParser now uses SkillRegistry instead of ToolRegistry
- JarvisOrchestrator simplified to only use SkillRegistry
- Command parser prompts updated to use skill-based syntax

### Deprecated
- `jarvis::tools::ToolRegistry` - Use `jarvis::skills::SkillRegistry`
- `jarvis::tools::Tool` trait - Use `jarvis::skills::Skill`
- `jarvis::tools::ToolParams` - Use `jarvis::skills::SkillContext`
- `jarvis::tools::ToolResult` - Use `jarvis::skills::SkillResult`
- All tool implementations (WindowsTool, TelegramTool, etc.) - Use corresponding skills

### Removed
- ToolRegistry from JarvisOrchestrator
- Tool-based command parsing logic

### Fixed
- Architectural consistency between orchestrator and command parser
- Reduced code duplication with unified skill interface

## [0.1.0] - 2025-01-XX

### Added
- Initial release
- JARVIS orchestration system
- Computer control skill (keyboard, mouse, window management)
- Email skill (SMTP/IMAP)
- Telegram bot skill
- Parallel execution engine (5x performance)
- Pattern detection and workflow recommendation
- n8n integration for visual automation
- Security features (Write Lock, Approval Gates, Kill Switch)
- Shadow analyzer for behavior learning
- Multi-platform support (Windows/macOS/Linux)

---

## Migration Timeline

- **v0.1.x**: Both Tool and Skill systems coexist
- **v0.2.0**: Tool system marked deprecated (current)
- **v0.3.0**: Deprecation warnings enforced
- **v0.4.0**: Tool system completely removed

## Upgrade Guide

See [MIGRATION_GUIDE.md](docs/MIGRATION_GUIDE.md) for detailed upgrade instructions.

---

[Unreleased]: https://github.com/yourusername/steer/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/steer/releases/tag/v0.1.0
