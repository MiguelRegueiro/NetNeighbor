# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- README restructured for faster scanning with quick-start-first layout and collapsible detail sections.
- Primary run instructions now use `cargo run --release` instead of direct `target/release/netneighbor` commands.
- Release/CI/changelog workflow guidance moved from README to `CONTRIBUTING.md`.
- Release workflow publish step made idempotent (supports updating existing tag releases and replacing artifacts).

## [1.0.1] - 2026-03-02

### Added
- GitHub Actions CI workflow enforcing format, lint, tests, and release-build checks.
- GitHub Actions Release workflow for tag-based publishing to GitHub Releases.
- Changelog section extractor script to generate release notes from `CHANGELOG.md`.

### Changed
- README updated with accurate Rust toolchain requirements and network probing behavior.
- README expanded with CI quality gates, release process, and changelog workflow documentation.
- Rust source cleaned to satisfy strict clippy settings (`-D warnings`) in CI.

## [1.0.0] - 2026-03-02

### Added
- Real-time device connect/disconnect detection from Linux `ip neigh` output.
- Routed-interface-first monitoring defaults with optional all-interface mode.
- Active subnet probing with configurable interval, timeout, and host limits.
- MAC-first device tracking with best-IP selection to avoid duplicate entries.
- Dashboard mode and plain output mode for terminal monitoring workflows.
- CLI controls for interval, disconnection hysteresis, probing, IPv6, and verbosity.

### Changed
- Improved disconnection reliability using timeout + minimum missed polls + state refresh.
- Reduced noisy detections by filtering common virtual interfaces by default.
