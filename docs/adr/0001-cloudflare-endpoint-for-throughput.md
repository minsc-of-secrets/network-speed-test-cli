# 1. Measure throughput against Cloudflare's public speed-test endpoint

## Status

Accepted

## Context

`nst` needs a server to transfer data against in order to measure Download
and Upload Throughput. Three realistic options existed:

1. **Ookla's speedtest.net protocol/servers** — the de facto industry
   standard, with the widest geographic server distribution. However,
   Ookla's terms of service explicitly prohibit automated access to their
   servers without prior written consent; existing unofficial Rust clients
   that speak this protocol operate in a legal grey area.
2. **Cloudflare's public speed-test endpoint**
   (`speed.cloudflare.com/__down`, `/__up`) — unauthenticated, publicly
   documented, and simple: a GET/POST of a caller-specified byte count, with
   throughput derived from transfer size and elapsed time.
3. **iperf3 wire protocol**, e.g. via the `riperf3` crate — connects to any
   iperf3-compatible server, but requires the user (or `nst`) to stand up a
   server first; no ready-made public server network exists for iperf3.

## Decision

`nst` measures throughput exclusively against Cloudflare's public
`speed.cloudflare.com` endpoint.

## Consequences

- No ToS risk: the endpoint is designed for exactly this kind of public,
  unauthenticated use.
- No server-provisioning burden on the user — works out of the box.
- Server selection/geography is Cloudflare's edge network, not a
  user-chosen or Ookla-style nearest-server list — `nst` cannot offer
  "pick your test server" the way Ookla-based tools do.
- Ties `nst`'s throughput measurement to Cloudflare's endpoint continuing to
  exist and remain publicly accessible in its current unauthenticated form.
- Switching to a different backend later (Ookla, iperf3, self-hosted) would
  mean replacing the transfer/measurement implementation, though the
  domain model (Phase, Stream, Throughput) is written to be
  backend-agnostic enough to survive that.
