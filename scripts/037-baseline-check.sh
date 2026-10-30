#!/usr/bin/env bash
# Diffs the server's observable surface against the baselines recorded on master
# (specs/037-core-extraction/quickstart.md §1, §4 and §5): every `--help` output,
# `GET /openapi.json`, and the resolved dependency triples (the workspace's own crates
# filtered out). Exits non-zero on any difference.
#
#   scripts/037-baseline-check.sh            # debug build (help and OpenAPI don't depend on it)
#   PROFILE=release scripts/037-baseline-check.sh
#
# Baselines live in target/037-baseline/ (gitignored, recorded by hand per quickstart §1).
set -euo pipefail

cd "$(dirname "$0")/.."
BASE=target/037-baseline
PROFILE=${PROFILE:-debug}
PORT=${PORT:-19999}

if [ ! -f "$BASE/openapi.json" ]; then
  echo "no baseline in $BASE; record it on master first (quickstart §1)" >&2
  exit 2
fi

if [ "$PROFILE" = release ]; then cargo build --release -p vizier 2>/dev/null || cargo build --release; else cargo build -p vizier 2>/dev/null || cargo build; fi
B=target/$PROFILE/vizier
failed=0

for c in "" run shutdown onboard skill agent "agent ps" "skill install"; do
  name="help-${c// /-}.txt"
  # shellcheck disable=SC2086
  if ! diff <($B $c --help 2>&1) "$BASE/$name"; then
    echo "DIFF: vizier $c --help" >&2
    failed=1
  fi
done

# A short path: the command socket must fit in SUN_LEN.
WS=$(mktemp -d /tmp/037-check.XXXXXX)
VIZIER_JWT_SECRET=baseline-check VIZIER_DATA_DIR="$WS" "$B" run --port "$PORT" >"$WS/log.txt" 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null || true; rm -rf "$WS"' EXIT
for _ in $(seq 1 120); do
  curl -sf "localhost:$PORT/openapi.json" -o "$WS/openapi.json" && break
  sleep 1
done
if ! cmp -s "$WS/openapi.json" "$BASE/openapi.json"; then
  echo "DIFF: /openapi.json" >&2
  diff <(jq -S . "$WS/openapi.json" 2>/dev/null || cat "$WS/openapi.json") <(jq -S . "$BASE/openapi.json") | head -40 >&2 || true
  failed=1
fi

workspace_crates='^vizier(-core|-http|-derive)? '
if ! diff <(grep -vE "$workspace_crates" "$BASE/deps.txt") \
  <(cargo metadata --format-version 1 --locked \
      | jq -r '.packages[] | "\(.name) \(.version) \(.source)"' | sort | grep -vE "$workspace_crates"); then
  echo "DIFF: resolved dependencies" >&2
  failed=1
fi

if [ "$failed" = 0 ]; then echo "baseline check: no differences"; fi
exit "$failed"
