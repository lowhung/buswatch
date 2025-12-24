# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **buswatch-tui**: CSV export format (#32)
  - Press `E` (uppercase) to export current view to CSV format
  - `--export-csv` CLI flag to export snapshot and exit
  - CSV includes module name, topic, type (Read/Write), count, backlog, pending duration, rate, and health status
  - Proper CSV escaping for special characters (commas, quotes, newlines)
- **buswatch-sdk**: Module unregistration support (#20)
  - `Instrumentor::unregister(name)` method to remove modules from internal state
  - `GlobalState::unregister_module(name)` for module cleanup
  - Returns `true` if module was found and removed, `false` otherwise
  - Enables clean lifecycle management for temporary or dynamic modules
  - Supports re-registration with fresh state after unregister
- **buswatch-sdk**: Prometheus exposition format export (`prometheus` feature)
  - HTTP server serving metrics at configurable endpoint
  - All metrics include `module` and `topic` labels
  - Health check endpoints (`/health`, `/healthz`) for Kubernetes probes
  - Metrics: read/write counts, backlog, pending seconds, rates

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
