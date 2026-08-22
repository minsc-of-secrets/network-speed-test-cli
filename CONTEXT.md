# Context: nst (network speed test CLI)

A single-context project: a lightweight Rust CLI that measures download/upload
throughput and idle latency against Cloudflare's public speed-test endpoint.

## Glossary

**Measurement Run**
One full execution of `nst`. Produces exactly one Result, consisting of a
Download Throughput, an Upload Throughput, and an Idle Latency.

**Phase**
A fixed-duration window during which one direction is exercised: the
Download Phase or the Upload Phase. A Measurement Run has exactly one Download
Phase and one Upload Phase, run sequentially. Duration defaults to 10s,
overridable per Measurement Run via `--duration`.

**Stream**
A single concurrent HTTP transfer within a Phase. A Phase runs a fixed number
of Streams concurrently for its full duration — 4 by default, overridable per
Measurement Run via `--connections`.

**Throughput**
The aggregate transfer rate of a Phase, in Mbps — the sum of all of its
Streams' bytes transferred during the Phase, divided by the Phase duration.
Not a per-Stream average: Throughput represents the effective combined
bandwidth, matching what tools like Ookla/fast.com report as "your speed."

**Idle Latency**
The median round-trip time of several HTTP requests sent to the Cloudflare
endpoint before any Phase begins (i.e., with no Streams in flight). Reported
as a single number in the Result.

Distinct from *Loaded Latency* (RTT measured while a Phase is transferring
data, which reflects bufferbloat) — Loaded Latency is out of scope for now
and not measured by `nst`.

**Sample**
A single RTT measurement taken while measuring Idle Latency. Several Samples
are taken and reduced to one Idle Latency value via the median.

**Result**
The final output of a Measurement Run: Download Throughput, Upload
Throughput, and Idle Latency. Rendered as human-readable text by default, or
as JSON with `--json`.

## Conventions

- All rate/aggregation values (Throughput, Idle Latency) use the **median**
  where multiple raw measurements must be reduced to one number — chosen over
  mean/min for resistance to transient outliers.
- Throughput is always expressed in **Mbps** (megabits/second), matching ISP
  advertising and mainstream speed-test tooling.
