#!/usr/bin/env bash
# server-release-watch.sh — watch any TerminusDB code source (default: the
# official terminusdb/terminusdb repo) for rc-line moves and pin-worthy rc
# tags, mirroring build.rs's TERMINUSDB_CODE_SOURCE semantics.
#
# The embedded server (crates/bin/build.rs) selects its source via
# TERMINUSDB_CODE_SOURCE:
#   https://github.com/owner/repo            → auto: highest <major>.<minor>-rc
#                                             branch, fallback highest version tag
#   https://github.com/owner/repo/tree/<ref> → explicit branch or tag
# This watcher resolves the same way and flags when the rc line moved past
# --expected-branch or a v<major>.<minor>-rc tag appeared.
#
# Output: human lines by default; --json for the workflow.
# Exit codes: 0 = ok, 1 = error, 2 = rc line moved or rc tag appeared.
#
# Usage:
#   scripts/server-release-watch.sh [--json] [--expected-branch 12.1-rc]
#       [--source https://github.com/terminusdb/terminusdb]

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

SOURCE="${TERMINUSDB_CODE_SOURCE:-https://github.com/terminusdb/terminusdb}"
EXPECTED_BRANCH="12.1-rc"
JSON=0
for arg in "$@"; do
  case "$arg" in
    --json) JSON=1 ;;
    --expected-branch=*) EXPECTED_BRANCH="${arg#--expected-branch=}" ;;
    --source=*) SOURCE="${arg#--source=}" ;;
    --help|-h) grep '^# ' "$0" | sed 's/^# //'; exit 0 ;;
    *) echo "unknown argument: $arg (--json|--expected-branch=X|--source=URL)" >&2; exit 2 ;;
  esac
done

# Parse SOURCE into (repo_url, explicit_ref) — same rules as build.rs.
case "$SOURCE" in
  *"/tree/"*)
    REPO_URL="${SOURCE%%/tree/*}"
    EXPLICIT_REF="${SOURCE##*/tree/}"
    ;;
  https://*|http://*|git@*)
    REPO_URL="$SOURCE"
    EXPLICIT_REF=""
    ;;
  *)
    echo "ERROR: invalid --source (expected https://github.com/owner/repo[/tree/<ref>] or ssh form): $SOURCE" >&2
    exit 1
    ;;
esac

# Highest <major>.<minor>-rc branch (same rule as build.rs auto-rc).
parse_rc_branches() { # stdin: ls-remote --heads output -> stdout: highest name
  awk '{ n=$2; sub(/refs\/heads\//,"",n); if (n ~ /^[0-9]+\.[0-9]+-rc$/) {
         v=n; sub(/-rc$/,"",v); split(v,p,"."); print p[1]*1000000+p[2], n } }' \
    | sort -n | tail -1 | cut -d' ' -f2
}

if [[ -n "$EXPLICIT_REF" ]]; then
  HIGHEST_BRANCH="$EXPLICIT_REF"
  MODE="explicit"
else
  HIGHEST_BRANCH=$(git ls-remote --heads "$REPO_URL" "refs/heads/*-rc" | parse_rc_branches)
  MODE="auto"
fi
RC_TAGS=$(git ls-remote --tags "$REPO_URL" "refs/tags/*-rc" | awk '{ n=$2; sub(/refs\/tags\//,"",n); if (n ~ /^v[0-9]+\.[0-9]+-rc[0-9]*$/) print n }' | tr '\n' ' ')

STATUS="ok"
if [[ -n "$HIGHEST_BRANCH" && "$HIGHEST_BRANCH" != "$EXPECTED_BRANCH" ]]; then
  STATUS="moved"
fi
if [[ -n "$RC_TAGS" ]]; then
  STATUS="${STATUS}-tags"
fi

if [[ "$JSON" -eq 1 ]]; then
  python3 - "$HIGHEST_BRANCH" "$RC_TAGS" "$STATUS" "$EXPECTED_BRANCH" "$MODE" <<'EOF'
import json, sys
branch, tags, status, expected, mode = sys.argv[1], sys.argv[2].split(), sys.argv[3], sys.argv[4], sys.argv[5]
print(json.dumps({"source_mode": mode, "ref": branch, "rc_tags": tags,
                  "expected_branch": expected, "status": status}))
EOF
else
  echo "source mode                            : $MODE"
  echo "ref ($MODE)                            : ${HIGHEST_BRANCH:-<none>}"
  echo "expected branch (pin baseline)         : $EXPECTED_BRANCH"
  echo "v<major>.<minor>-rc tags on upstream   : ${RC_TAGS:-<none>}"
  echo "status: $STATUS"
  if [[ "$STATUS" == "moved"* ]]; then
    echo "→ the rc line moved to $HIGHEST_BRANCH — re-run the client integration suite;"
    echo "  pin TERMINUSDB_CODE_SOURCE to .../tree/$HIGHEST_BRANCH for reproducibility."
  fi
  if [[ -n "$RC_TAGS" ]]; then
    echo "→ rc tag(s) published — pin TERMINUSDB_CODE_SOURCE=.../tree/$RC_TAGS for reproducible builds."
  fi
fi

if [[ "$STATUS" != "ok" ]]; then exit 2; fi
exit 0
