#!/usr/bin/env bash
# dx-benchmark-hyperfine.sh — find the sweet spot for .cargo/config.toml
# (CARGO_BUILD_JOBS / RUST_TEST_THREADS) using hyperfine, guided by the
# detected hardware (logical/physical cores, RAM) and OS.
#
# Sweet-spot rules (see CONTRIBUTING.md "Benchmarking build/test speed"):
#   CARGO_BUILD_JOBS  = min(logical cores, RAM_GB / 2.5)  (rustc is RAM-bound;
#                         ~1.5-4 GB per parallel compiler during LLVM codegen)
#   RUST_TEST_THREADS = physical cores (CPU-bound) |
#                       1.5-2x logical cores (I/O-bound: network/disk/async) |
#                       1 (shared resources: local DB, ports, global state)
#
# Usage:
#   scripts/dx-benchmark-hyperfine.sh [--full] [--no-bench]
#     --full      also benchmark clean builds and integration tests (slow)
#     --no-bench  print the hardware-based suggestion only (no hyperfine)
#
# Requirements: hyperfine (https://github.com/sharkdp/hyperfine). Missing
# linking dependencies (mold/lld/clang) only emit WARNINGs — the benchmark
# still runs (the no-dep alternative is scripts/dx-benchmark-no-dep.sh).

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# The workspace requires nightly (rust-toolchain.toml). mise/stable pins in the
# environment override the workspace pin and silently break builds on
# `#![feature(...)]` — force nightly for every measured command.
export RUSTUP_TOOLCHAIN=nightly

FULL=0
NO_BENCH=0
for arg in "$@"; do
  case "$arg" in
    --full)     FULL=1 ;;
    --no-bench) NO_BENCH=1 ;;
    --help|-h)
      grep '^# ' "$0" | sed 's/^# //'
      exit 0 ;;
    *) echo "unknown argument: $arg (--full|--no-bench|--help)" >&2; exit 1 ;;
  esac
done

OS="$(uname -s)"

# --- hardware detection (best-effort; defaults + notice on failure) ---------
LOGICAL=0; PHYSICAL=0; RAM_GB=0
case "$OS" in
  Darwin)
    LOGICAL=$(sysctl -n hw.ncpu 2>/dev/null || true)
    PHYSICAL=$(sysctl -n hw.physicalcpu 2>/dev/null || true)
    MEM_BYTES=$(sysctl -n hw.memsize 2>/dev/null || true)
    [[ -n "$MEM_BYTES" ]] && RAM_GB=$(awk "BEGIN {print int($MEM_BYTES/2^30)}")
    ;;
  Linux)
    LOGICAL=$(nproc 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null || true)
    if command -v lscpu >/dev/null 2>&1; then
      PHYSICAL=$(lscpu -p=core,socket 2>/dev/null | grep -v '^#' | sort -u | wc -l | tr -d ' ')
    elif [[ -r /proc/cpuinfo ]]; then
      PHYSICAL=$(grep -c '^processor' /proc/cpuinfo | tr -d ' ')   # fallback: logical
    fi
    [[ -r /proc/meminfo ]] && RAM_GB=$(awk '/^MemTotal/ {print int($2/1024/1024)}' /proc/meminfo)
    ;;
  *)
    # Windows (git-bash/MSYS2) or unknown — best-effort
    LOGICAL=$(nproc 2>/dev/null || true)
    RAM_BYTES=$(powershell -NoProfile -Command '(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory' 2>/dev/null || true)
    [[ -n "$RAM_BYTES" ]] && RAM_GB=$(awk "BEGIN {print int($RAM_BYTES/2^30)}")
    ;;
esac

[[ -n "$LOGICAL" && "$LOGICAL" -gt 0 ]] || { LOGICAL=4; LOGICAL_ESTIMATED=1; }
[[ -n "$PHYSICAL" && "$PHYSICAL" -gt 0 ]] || { PHYSICAL="$LOGICAL"; PHYSICAL_ESTIMATED=1; }
[[ -n "$RAM_GB" && "$RAM_GB" -gt 0 ]] || { RAM_GB=0; }

echo "== hardware ($(uname -srm)) =="
echo "logical cores : $LOGICAL${LOGICAL_ESTIMATED:+ (estimated)}"
echo "physical cores: $PHYSICAL${PHYSICAL_ESTIMATED:+ (estimated — fallback to logical)}"
if [[ "$RAM_GB" -gt 0 ]]; then
  echo "system RAM    : ${RAM_GB} GB"
else
  echo "system RAM    : unknown — jobs suggestion assumes enough RAM for all cores"
fi

# --- linker availability: WARNING only, never blocks -------------------------
echo
echo "== linking dependencies =="
find_tool() { command -v "$1" >/dev/null 2>&1 || { [[ -n "${2:-}" && -x "$2" ]]; }; }
WARNED=0
case "$OS" in
  Darwin)
    find_tool lld /opt/homebrew/opt/llvm/bin/lld || { echo "  WARNING: lld not found — linking uses the system linker (slower). Install: brew install llvm"; WARNED=1; }
    ;;
  Linux)
    find_tool mold || { echo "  WARNING: mold not found — linking uses the system linker (slower). Install: sudo apt install mold (fallback: lld)"; WARNED=1; }
    find_tool ld.lld || { echo "  WARNING: lld not found — linking uses the system linker (slower). Install: sudo apt install lld"; WARNED=1; }
    ;;
  *)
    find_tool lld-link || { echo "  WARNING: lld-link not found — linking uses MSVC link.exe (slower). lld ships with recent LLVM / Visual Studio"; WARNED=1; }
    ;;
esac
[[ "$WARNED" -eq 0 ]] && echo "  fast linking available (mold/lld)"

# --- sweet-spot suggestion ----------------------------------------------------
JOBS_SUG=$LOGICAL
if [[ "$RAM_GB" -gt 0 ]]; then
  RAM_JOBS=$((RAM_GB * 10 / 25))          # floor(RAM_GB / 2.5)
  [[ "$RAM_JOBS" -lt "$JOBS_SUG" ]] && JOBS_SUG=$RAM_JOBS
fi
[[ "$JOBS_SUG" -lt 1 ]] && JOBS_SUG=1
IO_LOW=$((LOGICAL * 3 / 2))
IO_HIGH=$((LOGICAL * 2))

echo
echo "== suggested sweet spot (hardware-based) =="
echo "  CARGO_BUILD_JOBS  = $JOBS_SUG   (min(logical=$LOGICAL, RAM_GB/2.5=$([[ "$RAM_GB" -gt 0 ]] && echo "$((RAM_GB * 10 / 25))" || echo '?')))"
echo "  RUST_TEST_THREADS = $PHYSICAL   (CPU-bound: physical cores)"
echo "                      $IO_LOW-$IO_HIGH (I/O-bound: 1.5-2x logical)"
echo "                      1          (shared resources: local DB / ports / global state)"

print_config_snippet() {
  cat <<EOF

== suggested .cargo/config.toml (edit your local copy of .cargo/config.toml.example) ==
[build]
jobs = $JOBS_SUG

[env]
# CPU-bound unit tests: physical cores ($PHYSICAL). I/O-bound tests: $IO_LOW-$IO_HIGH.
# Shared-resource tests (DB/ports/global state): 1. The hyperfine results above
# refine these starting points — pick the value that plateaus best.
RUST_TEST_THREADS = "$PHYSICAL"
EOF
}

if [[ "$NO_BENCH" -eq 1 ]]; then
  print_config_snippet
  echo
  echo "suggestion only (--no-bench) — nothing was measured."
  exit 0
fi

if ! command -v hyperfine >/dev/null 2>&1; then
  echo
  echo "ERROR: hyperfine not installed — https://github.com/sharkdp/hyperfine" >&2
  echo "       brew install hyperfine | apt install hyperfine | cargo install hyperfine" >&2
  echo "       (or use scripts/dx-benchmark-no-dep.sh, which needs no installs)" >&2
  exit 1
fi

# --- benchmark helpers --------------------------------------------------------
JOBS_CANDIDATES=()
for j in 4 6 8 12 16; do
  [[ "$j" -le $((LOGICAL * 2)) ]] && JOBS_CANDIDATES+=("$j")
done
contains() { # needle, haystack...
  local n="$1"; shift
  for v in "$@"; do [[ "$v" == "$n" ]] && return 0; done
  return 1
}
contains "$JOBS_SUG" "${JOBS_CANDIDATES[@]}" || JOBS_CANDIDATES+=("$JOBS_SUG")
contains 1 "${JOBS_CANDIDATES[@]}" || JOBS_CANDIDATES+=("1")

THREAD_CANDIDATES=()
for t in 1 2 4 8 16; do
  [[ "$t" -le $((LOGICAL * 2)) ]] && THREAD_CANDIDATES+=("$t")
done
contains "$PHYSICAL" "${THREAD_CANDIDATES[@]}" || THREAD_CANDIDATES+=("$PHYSICAL")

hyperfine_args=("--warmup" "1" "--style" "basic" "--shell" "sh")

echo
echo "== warm-up: populate caches =="
cargo build -p terminusdb-client >/dev/null 2>&1 || true
cargo test -p terminusdb-client --lib >/dev/null 2>&1 || true

echo
echo "== CARGO_BUILD_JOBS scaling (incremental: touch client lib per run) =="
HF_ARGS=("${hyperfine_args[@]}")
for j in "${JOBS_CANDIDATES[@]}"; do
  HF_ARGS+=("--command-name" "jobs=$j" "CARGO_BUILD_JOBS=$j cargo build -p terminusdb-client")
done
HF_ARGS+=("--prepare" "touch crates/client/src/lib.rs")
hyperfine "${HF_ARGS[@]}"

if [[ "$FULL" -eq 1 ]]; then
  echo
  echo "== CARGO_BUILD_JOBS scaling (clean build of the client crate) — WARNING: wipes client artifacts each run =="
  HF_ARGS=("${hyperfine_args[@]}")
  for j in "${JOBS_CANDIDATES[@]}"; do
    HF_ARGS+=("--command-name" "jobs=$j" "CARGO_BUILD_JOBS=$j cargo build -p terminusdb-client")
  done
  HF_ARGS+=("--prepare" "cargo clean -p terminusdb-client")
  hyperfine "${HF_ARGS[@]}"
fi

echo
echo "== RUST_TEST_THREADS scaling (unit tests — CPU-bound workload) =="
HF_ARGS=("${hyperfine_args[@]}")
for t in "${THREAD_CANDIDATES[@]}"; do
  HF_ARGS+=("--command-name" "threads=$t" "RUST_TEST_THREADS=$t cargo test -p terminusdb-client --lib")
done
hyperfine "${HF_ARGS[@]}"

if [[ "$FULL" -eq 1 ]]; then
  echo
  echo "== RUST_TEST_THREADS scaling (integration tests — I/O-bound: embedded servers) =="
  echo "   NOTE: first run builds the embedded server (minutes)"
  HF_ARGS=("${hyperfine_args[@]}")
  for t in "${THREAD_CANDIDATES[@]}"; do
    HF_ARGS+=("--command-name" "threads=$t" "RUST_TEST_THREADS=$t cargo test -p terminusdb-client --tests")
  done
  hyperfine "${HF_ARGS[@]}"
fi

echo
echo "== interpretation =="
echo "  jobs:     the value where wall time stops improving (RAM plateau) wins."
echo "  threads:  unit tests — physical cores ($PHYSICAL) usually plateaus;"
echo "            integration tests (I/O-bound) — 1.5-2x logical ($IO_LOW-$IO_HIGH) wins;"
echo "            if you see contention/flakiness with a local DB/ports, drop to 1."
print_config_snippet
