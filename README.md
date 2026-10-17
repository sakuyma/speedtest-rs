# speedtest-rs

A clean-room **Rust reimplementation of the Ookla `Speedtest by Ookla` CLI**
(target `1.2.0.84`). It performs real latency, download, and upload
measurements against the public Speedtest infrastructure and reproduces the
reference CLI's output formats, license flow, and result persistence.

It is behaviourally faithful where it matters: `--help`, `--version`, and the
server list (`-L`) are **byte-identical** to the reference, as are all output
formats and the non-interactive license-decline text.

> This is an independent reimplementation for interoperability and study. It is
> not affiliated with, endorsed by, or derived from Ookla's source code.

## Highlights

- Pure Rust: `ureq` + `rustls` for HTTPS, `libc` for `uname`/`getifaddrs`/`isatty`.
  No libcurl, no OpenSSL, no C toolchain beyond the Rust toolchain itself.
- Multi-connection download and upload with a shared connection pool.
- Streaming `jsonl` progress events (testStart → ping → download/upload
  progress → stage-end → result).
- Byte-exact human / csv / tsv / json / json-pretty output for a given result.
- Result persistence to `results.speedtest.net` with share-URL parsing.
- Persisted license acceptance in `~/.config/ookla/speedtest-cli.json`.

## Build

Requires a Rust toolchain (edition 2021). Only pure-Rust dependencies.

```sh
cargo build --release
```

The binary is `target/release/speedtest`.

## Usage

```sh
./target/release/speedtest                     # run a test, human-readable output
./target/release/speedtest --accept-license    # accept the license non-interactively
./target/release/speedtest -L                  # list the closest servers
./target/release/speedtest -s 58664 -f json    # test a specific server, JSON output
./target/release/speedtest -f json-pretty
./target/release/speedtest -f csv --output-header
./target/release/speedtest -f jsonl            # streaming events
./target/release/speedtest -u auto-decimal-bits
```

## Optimizations

### Build profile (`Cargo.toml`)

```toml
[profile.release]
opt-level = 3
lto = "fat"          # whole-program (cross-crate) optimization
codegen-units = 1    # single codegen unit: more inlining, smaller/faster binary
panic = "abort"      # no landing pads / unwind tables on the hot path
strip = "symbols"    # smaller release binary
```

`lto = "fat"` plus `codegen-units = 1` lets the optimizer inline across the
`ureq`/`rustls` boundary, which is where most of the time is spent on fast
links. `panic = "abort"` removes unwinding overhead from the transfer loops.

### Runtime

- **Connection pooling.** A single `ureq::Agent` is shared by every worker
  thread, with up to 64 idle connections per host, so repeated 25 MB ranged
  downloads and upload POSTs reuse warm TLS/TCP connections instead of
  re-handshaking.
- **Large read buffer.** Downloads use a 256 KiB read buffer, cutting syscall
  count and improving throughput versus a small default buffer.
- **Zero-copy upload payload.** The 16 MiB upload buffer is built once and
  shared as an immutable `&[u8]` across all upload threads — no per-request
  allocation or copy on the send path.
- **Cheap buffer construction.** The upload payload is filled by repeatedly
  doubling the seeded prefix (O(log n) `memcpy`s) rather than appending a
  10-byte pattern per iteration.
- **Lock-free progress.** A per-run byte counter is a single `AtomicI64`
  updated with `Relaxed` ordering; worker threads never contend on a mutex, and
  the reporter samples it at 10 Hz.
- **Scoped threads, no allocation churn.** Workers are spawned with
  `std::thread::scope`, borrowing the shared agent and payload directly instead
  of `Arc`-boxing per-thread state.
- **Parallel server selection.** Latency probes for the closest servers run
  concurrently across `available_parallelism()` threads, cutting selection time.
- **TCP-connect latency.** Latency is measured with a raw TCP connect
  (SYN → SYN/ACK) via `libc`, matching the reference's approach and avoiding a
  full HTTP round trip per sample.
- **Hand-rolled ordered JSON.** A small ordered JSON value/parser/writer
  guarantees exact number formatting (3-decimal doubles, integers) without
  pulling in `serde`, keeping the dependency graph tiny and the output stable.

## Tests

```sh
cargo test
```

Offline unit tests cover the JSON writer/parser, unit scaling and formatting,
latency statistics, option parsing, and byte-exact human/CSV/TSV/JSON output for
a fixed sample result.

## Module layout

```
src/
  json.rs       minimal ordered JSON value, parser and writer
  units.rs      speed unit parsing / auto-scaling / formatting
  http.rs       pooled ureq agent (get/post, streaming download/upload)
  config.rs     CLI config fetch and parsing (cli.speedtest.net)
  engine.rs     server selection, latency, download/upload stages, result JSON
  output.rs     human / csv / tsv / json / json-pretty / jsonl formatters
  settings.rs   persistent settings and license banner handling
  options.rs    option parsing, help and version text
  main.rs       entry point and orchestration
```

## Fidelity notes / limitations

- `-I/--interface`, `-i/--ip` (bind to interface/address) and `--ca-certificate`
  are parsed and accepted but **not applied**, because `ureq` does not expose
  interface binding or a custom CA bundle. Everything else behaves the same.
- Verified byte-identical against the reference for `--help`, `--version`, `-L`,
  and the non-interactive license-decline text.

## License

MIT — see [LICENSE](LICENSE).
