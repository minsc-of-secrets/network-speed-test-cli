# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `Cargo.toml` metadata: `repository`, `readme`, `keywords`, `categories`,
  and `rust-version` (MSRV 1.88, verified in CI).
- `CHANGELOG.md`.
- `--duration <SECONDS>` and `--connections <N>` flags, overriding the
  default Phase duration (10s) and Stream count (4) per Measurement Run.

## [0.1.1] - 2026-08-22

### Added

- Cross-platform prebuilt binaries published to GitHub Releases on every
  `v*` tag (macOS aarch64/x86_64, Linux x86_64, Windows x86_64), so
  installing no longer requires a Rust toolchain.
- `cargo audit` CI job and a `.github/dependabot.yml` (cargo +
  github-actions ecosystems, weekly), with Dependabot alerts and
  automated security-fix PRs enabled on the repo.
- `LICENSE-MIT` and `LICENSE-APACHE`, backing the `MIT OR Apache-2.0`
  license declared in `Cargo.toml`.
- `README.md` covering install, usage, JSON output, and how measurement
  works.
- Homebrew tap ([`homebrew-nst`](https://github.com/minsc-of-secrets/homebrew-nst)),
  switched partway through this version's development from a
  source-build formula to one that installs the prebuilt macOS binaries
  directly.
- `main` branch protection: force-push and branch deletion disabled.

### Changed

- Reduced the transport's chunk size (`CHUNK_BYTES`) from 10MB to 1MB
  after observing that Cloudflare's speed-test endpoint rate-limits
  large `bytes` requests aggressively (a `429` with `Retry-After: 3217`
  was reproduced during testing).
- `run_phase_bytes` now uses `tokio::task::JoinSet` instead of a
  `Vec<JoinHandle>`, so that if one Stream errors, the remaining Streams
  are aborted immediately instead of continuing to transfer data for
  the rest of the Phase with their results discarded.
- `run_phase_with_progress` (main.rs) now always aborts its progress
  ticker task before propagating a Phase's result, previously only on
  the success path.
- Bumped `indicatif` 0.17 → 0.18, dropping the unmaintained
  `number_prefix` transitive dependency (RUSTSEC-2025-0119).

### Fixed

- `CloudflareTransport::run_download_stream`'s initial request send is
  now raced against the Phase deadline (matching the upload path),
  so a stalled connection can no longer hang past the Phase's 10s
  budget.

## [0.1.0] - 2026-08-22

Initial release: measures download throughput, upload throughput, and
idle latency against Cloudflare's public speed-test endpoint
(`speed.cloudflare.com`).

### Added

- Core measurement domain (`Phase`, `Stream`, `Throughput`, `Idle
  Latency`) in `src/measurement.rs`, transport-agnostic via a
  `Transport` trait implemented by `src/transport.rs`
  (`tokio` + `reqwest`, Cloudflare's `/__down` and `/__up` endpoints).
- CLI (`src/main.rs`): human-readable output by default, `--json` for
  machine-readable output, live `indicatif` progress bars per Phase.
- Unit tests: `wiremock`-backed HTTP tests plus a fake `Transport` for
  logic-only tests (no network required).
- `CONTEXT.md` glossary and `docs/adr/0001-cloudflare-endpoint-for-throughput.md`
  documenting the choice of Cloudflare's endpoint over Ookla's protocol.
- GitHub Actions CI: `fmt`, `clippy`, and `build + test` across a
  macOS/Linux/Windows matrix.

[Unreleased]: https://github.com/minsc-of-secrets/network-speed-test-cli/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/minsc-of-secrets/network-speed-test-cli/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/minsc-of-secrets/network-speed-test-cli/releases/tag/v0.1.0
