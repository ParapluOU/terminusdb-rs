# Contributing to terminusdb-rs

Thanks for contributing! This workspace is a Rust client for TerminusDB with an
embedded-server test harness. This guide keeps your first build fast and your
daily loop pleasant — everything here is **optional guidance**, not imposed
configuration.

## Prerequisites

- **Rust (nightly)** — the workspace pins a nightly toolchain via
  `rust-toolchain.toml`. Install with `rustup toolchain install nightly` (or
  use `mise` if you prefer). Nightly is required for the whole workspace: the
  client crate's dependency tree (`terminusdb-schema`) uses nightly features
  such as `specialization`.
  > **Troubleshooting:** if plain `cargo` fails with `#![feature(...)]` errors
  > (e.g. `specialization`), an environment pin is overriding the workspace
  > toolchain — e.g. mise exports `RUSTUP_TOOLCHAIN=1.97.1` (stable), which
  > silently beats `rust-toolchain.toml`. Run builds with
  > `RUSTUP_TOOLCHAIN=nightly cargo ...` (the benchmark script does this
  > automatically).
- **System dependencies** (for the embedded server used by integration tests):

  | OS | Packages |
  |----|----------|
  | Linux (Debian/Ubuntu) | `build-essential make git curl unzip ca-certificates openssl libssl-dev libgmp-dev clang libclang-dev protobuf-compiler` |
  | macOS | `brew install llvm protobuf gmp openssl` (Xcode Command Line Tools required) |
  | Windows | The **client/schema crates build and test natively**. The embedded-server tests need SWI-Prolog + a POSIX toolchain — use **WSL2** for those. |

  The embedded server (`terminusdb-bin`) builds TerminusDB from source on first
  test run and provisions its own SWI-Prolog runtime — this is the slowest step
  in the repo, and it's cached afterward.

## First build (expect this)

```bash
cargo check -p terminusdb-client        # fast — seconds
cargo build -p terminusdb-client        # compiles the client + deps
cargo build -p terminusdb-bin           # first run: builds the embedded server (minutes)
```

The first `terminusdb-bin` build clones and compiles the pinned TerminusDB
server (with SWI-Prolog). It is cached under `target/` and `crates/bin/.deps/`
— subsequent builds are fast. Prefer `cargo check` over `cargo build` for
iteration.

## Daily loop

```bash
cargo check -p terminusdb-client --tests     # type-check everything, fast
cargo test -p terminusdb-client --lib        # unit tests (no server needed)
cargo test -p terminusdb-client --tests      # integration tests (embedded servers)
```

Integration tests spawn **real per-process TerminusDB servers in memory mode**
(`TerminusDBServer::test_instance()`), each on a unique port, so they run in
parallel safely. If you ever see transaction contention between test processes,
serialize them:

```bash
RUST_TEST_THREADS=1 cargo test --workspace
```

Use `cargo test <name>` to run a single test, and see the embedded-server
helpers (`with_tmp_db`, `with_db_schema`, `with_db_seed`) for building focused
test fixtures — no `#[ignore]`, no external service, no Docker required.

## Optional speed-ups (all opt-in)

| Speed-up | Effect | How (your choice — no repo config changes) |
|----------|--------|---------------------------------------------|
| **sccache** | Caches compiled artifacts across workspaces/rebuilds | Install per OS — [macOS](https://www.google.com/search?q=how+install+sccache+on+macos), [Linux](https://www.google.com/search?q=how+install+sccache+on+linux), [Windows](https://www.google.com/search?q=how+install+sccache+on+windows) — then `CARGO_BUILD_RUSTC_WRAPPER=sccache cargo build` |
| **lld / mold linker** | Faster linking (Linux/macOS/Windows) | **Linux:** `sudo apt install mold` (or `lld`) · **macOS:** `brew install llvm` (lld included) · **Windows:** lld ships with recent LLVM / Visual Studio — then copy `.cargo/config.toml.example` to `.cargo/config.toml` (see below) |

### Per-developer cargo config

The repo ships **`.cargo/config.toml.example`** — a ready-made, per-OS-optimized baseline
(dev profile: `opt-level = 0`, `debug = 1`, `codegen-units = 256`, deps at
`opt-level = 2`; linkers: mold on Linux, lld on macOS/Windows; `split-debuginfo`;
`jobs`; `RUST_TEST_THREADS`/`RUST_TEST_NOCAPTURE`). Your **own** `.cargo/config.toml`
is **gitignored**, so each developer keeps their local settings without affecting
anyone else:

```bash
cp .cargo/config.toml.example .cargo/config.toml   # one-time setup
# edit to taste — jobs, threads, linker, debuginfo
```

Nothing imposes anything: without the copy, the workspace builds with defaults.
| **mise** | Pins the nightly toolchain per-workspace (no toolchain flips → no full rebuilds) | `mise install` after adding `.mise.toml` locally |
| **Profile overrides** | Tune dev builds without editing the repo | `CARGO_PROFILE_DEV_OPT_LEVEL`, `CARGO_PROFILE_DEV_CODEGEN_UNITS`, … |
| **Out-of-repo target dir** | Keeps the repo light and shares the cache across clones | `export CARGO_TARGET_DIR="$HOME/Library/Caches/Cargo/target"` (macOS) / `"$HOME/.cache/cargo/target"` (Linux) / `%LOCALAPPDATA%\Cargo\target` (Windows) |
| **Single-threaded tests** | Escape hatch under contention | `RUST_TEST_THREADS=1` (env var, per-invocation) |

The example's dev profile (`codegen-units = 256`, deps at `opt-level = 2`) trades
a slightly longer first dependency build for much faster test runtime, notably the
embedded server. Without the copy, rustc's defaults apply — nothing is imposed.

## CI reference

`.github/workflows/tests.yml` mirrors the Linux path above (system deps →
nightly → cached embedded-server build → `cargo test`). If you add system
dependencies or change the embedded-server pin, update that file and this
guide together.

## Benchmarking build/test speed

Two scripts, same methodology (nightly forced automatically, `CARGO_TARGET_DIR`
aware):

```bash
scripts/dx-benchmark-no-dep.sh                # rustc-native, zero installs
scripts/dx-benchmark-no-dep.sh wall 5         # wall-clock only, 5 runs
scripts/dx-benchmark-hyperfine.sh             # hardware-guided sweet spot (RAM/cores)
scripts/dx-benchmark-hyperfine.sh --full      # + clean-build & integration-test scaling
scripts/dx-benchmark-hyperfine.sh --no-bench  # suggestion only, no hyperfine needed
```

**`dx-benchmark-hyperfine.sh`** detects logical/physical cores and RAM, suggests
a `.cargo/config.toml` starting point, then uses
[hyperfine](https://github.com/sharkdp/hyperfine) to find the real sweet spot:

- `CARGO_BUILD_JOBS = min(logical cores, RAM_GB / 2.5)` — rustc is RAM-bound
  (each parallel compiler can use 1.5–4 GB during LLVM codegen); low-RAM
  machines must cut jobs or they swap and lose 90% of compile speed
- `RUST_TEST_THREADS` — **physical cores** for CPU-bound unit tests,
  **1.5–2× logical cores** for I/O-bound integration tests (network/disk/async
  wait), **1** for shared-resource tests (local DB, ports, global state — the
  embedded-server harness)

Missing linking dependencies (mold/lld) only emit **WARNING**s — the benchmark
still runs on the system linker.

Methodology (compare the same target on the base branch and the candidate
branch, same machine):

1. **Warm the cache first** — one build/test before measuring, so caches are
   populated and you measure incremental reality, not cold misses
2. **Use medians of ≥ 3 runs** — machine variance is real; single runs mislead
3. **Interpret correctly**:
   - *incremental build* and *unit-test runtime* are the **wins** the
     example's dev profile targets (256 codegen units; deps at `opt-level 2`
     make the embedded server and tests run much faster)
   - a **clean build is intentionally slower** on the optimized-deps profile —
     that is the documented trade-off for faster test runtime, not a regression

## Before opening a PR

- `cargo fmt -- --check` and `cargo clippy --all-targets` (pre-existing warnings
  on the base branch are not yours)
- Add tests with the embedded-server pattern; integration tests must not require
  an external service
- Keep the diff focused — one logical change per PR
