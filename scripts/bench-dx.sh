#!/usr/bin/env bash
# bench-dx.sh — measure local build/test speed with rustc-native tooling.
#
# Methodology follows the rustc-dev-guide profiling page and the rustc-perf
# collector (scenarios x metrics, per-crate timings, self-profile):
#   - scenarios: full (clean) / incr-unchanged / incr-patched
#   - metrics:   wall time, cargo --timings per-unit durations, optional
#                rustc -Z self-profile (measureme summarize)
#
# Usage:
#   scripts/bench-dx.sh [target] [runs]
#     targets: wall | timings | selfprofile | all    (default: all)
#     runs:    repetitions per measurement (default: 3)
#
# Compare base vs candidate branch by running on each and diffing the tables
# (same machine, warm cache first).

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# The workspace requires nightly (rust-toolchain.toml). Some setups (mise) pin
# RUSTUP_TOOLCHAIN to a stable release in the environment, which overrides the
# workspace pin and silently breaks builds on `#![feature(...)]`. Force nightly.
export RUSTUP_TOOLCHAIN=nightly

TARGET="${1:-all}"
RUNS="${2:-3}"
TIMEFORMAT='%R'   # fractional seconds, bash builtin (macOS-safe)

median() { # stdin: newline-separated floats -> stdout: median
  sort -n | awk '{a[NR]=$1} END {if (NR%2) print a[(NR+1)/2]; else print (a[NR/2]+a[NR/2+1])/2}'
}

wall() { # cmd... -> stdout: wall seconds (stderr suppressed)
  local out t
  out=$({ time { "$@" >/dev/null 2>&1; }; } 2>&1) || return 1
  printf '%s\n' "$out" | tail -1
}

bench_scenario() { # name, cmd...
  local name="$1"; shift
  echo "== $name =="
  local -a times=()
  for i in $(seq 1 "$RUNS"); do
    local t
    t=$(wall "$@") || { echo "  run $i: FAILED"; return 1; }
    times+=("$t")
    printf '  run %d: %ss\n' "$i" "$t"
  done
  printf '  median: %ss\n' "$(printf '%s\n' "${times[@]}" | median)"
}

s_incr_unchanged() { cargo build -p terminusdb-client; }
s_incr_patched()   { touch crates/client/src/lib.rs; cargo build -p terminusdb-client; }
s_full()           { cargo clean -p terminusdb-client; cargo build -p terminusdb-client; }

bench_wall() {
  bench_scenario "incr-unchanged (no-op rebuild)" s_incr_unchanged
  bench_scenario "incr-patched (touch client lib)" s_incr_patched
  echo "== full (clean -p terminusdb-client) — WARNING: wipes client artifacts =="
  bench_scenario "full rebuild" s_full
}

bench_timings() {
  echo "== cargo --timings (per-unit durations) =="
  cargo build -p terminusdb-client --timings >/dev/null 2>&1
  local timings_dir="${CARGO_TARGET_DIR:-target}/cargo-timings"
  local f
  f="$(ls -t "$timings_dir"/cargo-timing-*.html 2>/dev/null | head -1)"
  if [[ -n "$f" ]]; then
    echo "report: $f  (open in a browser — units, durations, parallelism)"
    echo "total wall: $(wall s_incr_unchanged)s"
  else
    echo "no timings report produced in $timings_dir"
  fi
}

bench_selfprofile() {
  echo "== rustc -Z self-profile (per-query timings) =="
  if ! command -v summarize >/dev/null 2>&1; then
    echo "measureme 'summarize' not installed — run:"
    echo "  cargo install measureme  # provides summarize/flamegraph/crox"
    echo "  RUSTFLAGS='-Z self-profile' cargo build -p terminusdb-client"
    echo "  summarize profile-*.json   # aggregates per-query/per-phase times"
    return 0
  fi
  rm -f profile-*.json
  RUSTFLAGS='-Z self-profile' cargo build -p terminusdb-client >/dev/null 2>&1
  summarize profile-*.json
}

case "$TARGET" in
  wall)        bench_wall ;;
  timings)     bench_timings ;;
  selfprofile) bench_selfprofile ;;
  all)
    echo "# warm-up (populate caches)"
    cargo build -p terminusdb-client >/dev/null 2>&1 || true
    bench_wall
    bench_timings
    echo "# self-profile: scripts/bench-dx.sh selfprofile (needs measureme 'summarize')"
    ;;
  *) echo "unknown target: $TARGET (wall|timings|selfprofile|all)" >&2; exit 1 ;;
esac
