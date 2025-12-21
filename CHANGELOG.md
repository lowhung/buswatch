# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-12-21

### Added

- Initial public release
- **Three monitoring views**:
  - Summary view: Module overview with health status, message rates, and sparklines
  - Bottleneck view: Topics with pending reads/writes for identifying issues
  - Data Flow view: Producer/consumer relationship matrix
- **Multiple data sources**:
  - File-based polling (`--file`)
  - TCP stream connection (`--address`)
  - RabbitMQ subscription (`--subscribe`, requires `subscribe` feature)
- **Health monitoring** with configurable thresholds for warning and critical states
- **Interactive TUI** with keyboard and mouse support
- Light/dark theme auto-detection
- Module detail overlay (press Enter or right-click)
- JSON export functionality
- Vim-style navigation keybindings

### Features

- `subscribe` - Optional RabbitMQ integration via lapin

[unreleased]: https://github.com/lowhung/monitor-tui/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/lowhung/monitor-tui/releases/tag/v0.1.0
