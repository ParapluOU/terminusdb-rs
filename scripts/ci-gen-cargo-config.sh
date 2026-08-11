#!/usr/bin/env bash
# ci-gen-cargo-config.sh — generic .cargo/config.toml generator: auto-discovers
# the machine's configuration (cores, RAM, OS, installed linkers) at runtime,
# with no knowledge of the runner or CI vendor (GitHub, act containers,
# self-hosted, anything). Container-aware on Linux (cgroup v2/v1 limits beat
# host-reported /proc values).
#
# The workspace ships NO active .cargo/config.toml (it is gitignored; see
# CONTRIBUTING.md). CI calls this script before the first cargo invocation so
# build jobs / test threads / per-target linker flags match the machine instead
# of rustc defaults.
#
# Sections are optional and emitted only when discoverable/requested (default:
# all three):
#   [build]   jobs = min(cores, RAM_GB / 2.5)          (rustc is RAM-bound)
#   [env]     RUST_TEST_THREADS = cores                (per-invocation override
#             via --threads; 1 for shared-resource tests)
#   [target]  per-OS linker/rustflags — emitted ONLY if the linker is present
#             (mold/lld on Linux, lld on macOS, lld-link on Windows)
#
# Usage:
#   scripts/ci-gen-cargo-config.sh [--build] [--env] [--target] [--jobs N]
#       [--threads N] [--cores N] [--ram N] [--os darwin|linux|windows]
#       [--out PATH] [--dry-run] [--force]
#
# Overrides (--jobs/--threads/--cores/--ram/--os) are for CI pipelines that
# know their runner topology better than detection, and for local testing.
# Refuses to overwrite an existing .cargo/config.toml unless --force.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

BUILD=0; ENV=0; TARGET=0
JOBS=""; THREADS=""; CORES=""; RAM=""; OS_OVERRIDE=""
OUT=".cargo/config.toml"; DRY=0; FORCE=0
HAVE_ALL=1
for arg in "$@"; do
  case "$arg" in
    --build)  BUILD=1; HAVE_ALL=0 ;;
    --env)    ENV=1;   HAVE_ALL=0 ;;
    --target) TARGET=1; HAVE_ALL=0 ;;
    --dry-run) DRY=1 ;;
    --force)  FORCE=1 ;;
    --jobs)   echo "missing value: $arg" >&2; exit 2 ;;
    --jobs=*) JOBS="${arg#--jobs=}" ;;
    --threads=*) THREADS="${arg#--threads=}" ;;
    --cores=*) CORES="${arg#--cores=}" ;;
    --ram=*) RAM="${arg#--ram=}" ;;
    --os=*) OS_OVERRIDE="${arg#--os=}" ;;
    --out=*) OUT="${arg#--out=}" ;;
    --help|-h)
      grep '^# ' "$0" | sed 's/^# //'
      exit 0 ;;
    *) echo "unknown argument: $arg (--build|--env|--target|--jobs=N|--threads=N|--cores=N|--ram=N|--os=...|--out=...|--dry-run|--force|--help)" >&2; exit 2 ;;
  esac
done
[[ "$HAVE_ALL" -eq 1 ]] && BUILD=1 && ENV=1 && TARGET=1

# --- machine detection (same rules as scripts/dx-benchmark-hyperfine.sh) -----
OS="${OS_OVERRIDE:-$(uname -s)}"
case "$OS" in
  Darwin|darwin) OS=darwin ;;
  Linux|linux)   OS=linux ;;
  *)             OS=windows ;;
esac

CORES_D=0; RAM_D=0
case "$OS" in
  darwin)
    CORES_D=$(sysctl -n hw.ncpu 2>/dev/null || true)
    MEM_BYTES=$(sysctl -n hw.memsize 2>/dev/null || true)
    [[ -n "$MEM_BYTES" ]] && RAM_D=$(awk "BEGIN {print int($MEM_BYTES/2^30)}")
    ;;
  linux)
    CORES_D=$(nproc 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null || true)
    [[ -r /proc/meminfo ]] && RAM_D=$(awk '/^MemTotal/ {print int($2/1024/1024)}' /proc/meminfo)
    # Container-aware tier: in containers (act/docker), /proc/meminfo and nproc
    # report the HOST, not the cgroup limits — prefer cgroup v2/v1 when present.
    if [[ -r /sys/fs/cgroup/cpu.max ]]; then
      read -r quota period < /sys/fs/cgroup/cpu.max
      [[ "$quota" =~ ^[0-9]+$ && "$period" =~ ^[0-9]+$ && "$period" -gt 0 ]] && \
        CGROUP_CORES=$((quota / period)) && [[ "$CGROUP_CORES" -gt 0 ]] && CORES_D=$CGROUP_CORES
    elif [[ -r /sys/fs/cgroup/cpu/cpu.cfs_quota_us ]]; then
      quota=$(</sys/fs/cgroup/cpu/cpu.cfs_quota_us)
      period=$(</sys/fs/cgroup/cpu/cpu.cfs_period_us)
      [[ "$quota" =~ ^[0-9]+$ && "$period" =~ ^[0-9]+$ && "$period" -gt 0 ]] && \
        CGROUP_CORES=$((quota / period)) && [[ "$CGROUP_CORES" -gt 0 ]] && CORES_D=$CGROUP_CORES
    fi
    if [[ -r /sys/fs/cgroup/memory.max ]]; then
      mem_max=$(</sys/fs/cgroup/memory.max)
      [[ "$mem_max" =~ ^[0-9]+$ && "$mem_max" -gt 0 ]] && RAM_D=$((mem_max / 2**30))
    elif [[ -r /sys/fs/cgroup/memory/memory.limit_in_bytes ]]; then
      mem_max=$(</sys/fs/cgroup/memory/memory.limit_in_bytes)
      [[ "$mem_max" =~ ^[0-9]+$ && "$mem_max" -gt 0 ]] && RAM_D=$((mem_max / 2**30))
    fi
    ;;
  windows)
    CORES_D=$(nproc 2>/dev/null || true)
    RAM_BYTES=$(powershell -NoProfile -Command '(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory' 2>/dev/null || true)
    [[ -n "$RAM_BYTES" ]] && RAM_D=$(awk "BEGIN {print int($RAM_BYTES/2^30)}")
    ;;
esac

[[ -n "$CORES" ]] || CORES="$CORES_D"
[[ -n "$RAM" ]] || RAM="$RAM_D"
[[ -n "$CORES" && "$CORES" -gt 0 ]] || { echo "WARNING: could not detect logical cores — skipping [build]/[env] sections" >&2; CORES=0; }

JOBS_D=""
if [[ "$CORES" -gt 0 ]]; then
  JOBS_D=$CORES
  if [[ "$RAM" -gt 0 ]]; then
    RAM_JOBS=$((RAM * 10 / 25))
    [[ "$RAM_JOBS" -lt "$JOBS_D" ]] && JOBS_D=$RAM_JOBS
  fi
  [[ "$JOBS_D" -lt 1 ]] && JOBS_D=1
fi
[[ -n "$JOBS" ]] || JOBS="$JOBS_D"
[[ -n "$THREADS" ]] || THREADS="$CORES"

# --- per-OS linker detection (optional [target] sections) ----------------------
LINKER_SECTION=""
case "$OS" in
  darwin)
    if command -v lld >/dev/null 2>&1 || [[ -x /opt/homebrew/opt/llvm/bin/lld ]]; then
      LINKER_SECTION=$'[target.\'cfg(target_os = "macos")\']\nrustflags = [\n    "-C", "split-debuginfo=unpacked",\n    "-C", "debuginfo=1",\n    "-C", "link-arg=-fuse-ld=lld",\n]'
    fi
    ;;
  linux)
    if command -v mold >/dev/null 2>&1; then
      LINKER_SECTION=$'[target.\'cfg(target_os = "linux")\']\nlinker = "clang"\nrustflags = [\n    "-C", "link-arg=-fuse-ld=mold",\n    "-C", "debuginfo=1",\n]'
    elif command -v ld.lld >/dev/null 2>&1 && command -v clang >/dev/null 2>&1; then
      LINKER_SECTION=$'[target.\'cfg(target_os = "linux")\']\nlinker = "clang"\nrustflags = [\n    "-C", "link-arg=-fuse-ld=lld",\n    "-C", "debuginfo=1",\n]'
    fi
    ;;
  windows)
    if command -v lld-link >/dev/null 2>&1; then
      LINKER_SECTION=$'[target.\'cfg(target_os = "windows")\']\nrustflags = [\n    "-C", "link-arg=-fuse-ld=lld",\n    "-C", "split-debuginfo=unpacked",\n    "-C", "debuginfo=1",\n]'
    fi
    ;;
esac

# --- emit ----------------------------------------------------------------------
GEN="# Generated by scripts/ci-gen-cargo-config.sh — $(date -u +%Y-%m-%dT%H:%M:%SZ), os=$OS cores=$CORES ram=${RAM:-?}GB"

if [[ "$BUILD" -eq 1 && -n "$JOBS" && "$JOBS" -gt 0 ]]; then
  GEN+=$'\n\n[build]\njobs = '"$JOBS"
fi
if [[ "$ENV" -eq 1 && -n "$THREADS" && "$THREADS" -gt 0 ]]; then
  GEN+=$'\n\n[env]\nRUST_TEST_THREADS = "'"$THREADS"'"'
fi
if [[ "$TARGET" -eq 1 && -n "$LINKER_SECTION" ]]; then
  GEN+=$'\n\n'"$LINKER_SECTION"
fi

if [[ "$DRY" -eq 1 ]]; then
  printf '%s\n' "$GEN"
  exit 0
fi

if [[ -f "$OUT" && "$FORCE" -ne 1 ]]; then
  echo "ERROR: $OUT already exists (refusing to overwrite) — pass --force or --out=..." >&2
  exit 1
fi
mkdir -p "$(dirname "$OUT")"
printf '%s\n' "$GEN" > "$OUT"
SECTIONS=()
[[ "$BUILD" -eq 1 ]] && SECTIONS+=("build")
[[ "$ENV" -eq 1 ]] && SECTIONS+=("env")
[[ "$TARGET" -eq 1 ]] && SECTIONS+=("target")
echo "wrote $OUT (sections: ${SECTIONS[*]:-none})"
