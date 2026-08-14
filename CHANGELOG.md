# Changelog

All notable changes to Brewake are documented here. The project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-08-14

### Added

- Native Windows tray application with active, timed, pause, display, and startup controls.
- `brewakectl` companion with human-readable and JSON status output.
- Single-instance behavior, local JSON configuration, and clean shutdown handling.
- Re-application of the Windows execution-state request after power events.
- Windows CI, release packaging, SHA-256 checksums, and an interactive tray regression test.
- English and Vietnamese documentation.

### Fixed

- Prevented a right-click crash caused by Win32's modal popup-menu loop re-entering the tray window procedure while application state was mutably borrowed.
- Rejected unsafe timer durations instead of allowing deadline arithmetic to overflow.
- Exposed whether the Windows power request actually succeeded.

[Unreleased]: https://github.com/thanhbican/brewake/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/thanhbican/brewake/releases/tag/v0.1.0
