# nst

A lightweight CLI network speed test, written in Rust. Measures download
speed, upload speed, and latency against Cloudflare's public speed-test
endpoint — no account, API key, or server setup required.

```
$ nst
Download:    42.72 Mbps
Upload:      46.39 Mbps
Latency:      50.8 ms
```

## Install

No Rust toolchain required for any of these — prebuilt binaries are
published to [GitHub Releases](../../releases) for macOS (Apple Silicon
and Intel), Linux (x86_64), and Windows (x86_64).

### Homebrew (macOS)

```
brew install minsc-of-secrets/nst/nst
```

### Direct download (macOS / Linux / Windows)

Grab the archive for your platform from the
[latest release](../../releases/latest), extract it, and run the `nst`
binary inside — e.g. on macOS/Linux:

```
tar xzf nst-*-<target>.tar.gz
./nst-*-<target>/nst
```

On macOS, since the binary isn't notarized, Gatekeeper may refuse to run
it the first time ("cannot be opened because the developer cannot be
verified"). Clear the quarantine flag once with:

```
xattr -d com.apple.quarantine ./nst-*-<target>/nst
```

### From source

Requires a [Rust toolchain](https://rustup.rs/).

```
git clone <this repo>
cd network-speed-test-cli
cargo install --path .
```

This installs the `nst` binary to `~/.cargo/bin`.

## Usage

```
nst           # human-readable output
nst --json    # machine-readable JSON output
```

JSON output:

```json
{
  "download_mbps": 42.72,
  "upload_mbps": 46.39,
  "idle_latency_ms": 50.8
}
```

`nst` exits with a non-zero status and prints an error to stderr if it
can't reach the measurement endpoint — it doesn't retry.

## How it works

Each run:

1. Measures **idle latency**: 5 round trips to the endpoint, taken before
   any transfer begins, reduced to one value via the median.
2. Measures **download throughput**: 4 concurrent streams pull data for
   10 seconds; the combined bytes transferred, divided by elapsed time,
   is the reported Mbps.
3. Measures **upload throughput** the same way, in the opposite direction.

See [`CONTEXT.md`](./CONTEXT.md) for the full glossary (Phase, Stream,
Throughput, etc.) and [`docs/adr/`](./docs/adr/) for the reasoning behind
using Cloudflare's endpoint instead of Ookla's protocol.

### A note on rate limiting

Cloudflare's endpoint rate-limits large chunk requests fairly
aggressively — running `nst` back-to-back many times in a short window
can trigger a 429 that lasts up to roughly an hour. This isn't a bug in
`nst`; just space out repeated runs if you hit it.

## Development

```
cargo test              # unit tests (no network required — HTTP is mocked)
cargo clippy --all-targets
cargo fmt --check
cargo run --release      # a real measurement against the live endpoint
```

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE)
or [MIT license](./LICENSE-MIT) at your option.
