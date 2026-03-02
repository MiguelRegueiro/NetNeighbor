# Contributing

Thanks for contributing to NetNeighbor.

## Development Workflow

1. Create a feature branch from `main`.
2. Make focused changes with clear commit messages.
3. Update `CHANGELOG.md` under `## [Unreleased]` for user-visible changes.
4. Run local quality checks before opening a PR.

## Local Quality Checks

Run the same gates used by CI:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release --locked
```

## CI

GitHub Actions CI runs on:
- pushes to `main`
- pull requests

CI enforces:
- formatting (`cargo fmt --all -- --check`)
- linting (`cargo clippy --all-targets --all-features -- -D warnings`)
- tests (`cargo test --all-targets --all-features`)
- release build (`cargo build --release --locked`)

## Changelog Rules

`CHANGELOG.md` follows Keep a Changelog + SemVer.

- Add user-visible changes under `## [Unreleased]`
- Before release, move entries to `## [X.Y.Z] - YYYY-MM-DD`
- Keep entries concise and grouped (`Added`, `Changed`, `Fixed`, etc.)

## Release Process

CD publishes a GitHub Release when a tag `vX.Y.Z` is pushed.

Pre-release checklist:
1. Ensure `Cargo.toml` version is `X.Y.Z`.
2. Ensure `Cargo.lock` reflects the same package version.
3. Ensure `CHANGELOG.md` contains `## [X.Y.Z] - YYYY-MM-DD` with release notes.
4. Ensure `## [Unreleased]` is reset for future work.
5. Run local quality checks.

Release commands:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

Release workflow behavior:
- validates tag/version match against `Cargo.toml`
- re-runs quality gates
- builds release binary
- generates `SHA256SUMS.txt`
- extracts release notes from `CHANGELOG.md`
- publishes GitHub Release artifacts
