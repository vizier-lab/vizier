#!/bin/bash
# Translate container env vars to `vizier run` CLI flags, then exec the binary
# so signals (SIGTERM, SIGINT) propagate correctly to the daemon.
#
# Resolution order for the workspace:
#   1. VIZIER_DATA_DIR or VIZIER_WORKSPACE (env var)
#   2. $HOME/.vizier (default)
#
# For subcommands other than `run` (e.g. `shutdown`, `agent ps`), env-var
# translation is skipped and args are passed through to `vizier` directly.
set -euo pipefail

SUBCOMMAND="${1:-run}"

if [ "$SUBCOMMAND" != "run" ]; then
  exec vizier "$@"
fi

# Hardcoded fallback for local/dev use. Set VIZIER_JWT_SECRET in production
# (e.g. via -e, secrets manager, or compose file) to a strong random string.
export VIZIER_JWT_SECRET="${VIZIER_JWT_SECRET:-vizier-default-secret-change-me}"

# Env vars act as defaults; explicit user args appended LAST so they win via
# clap's last-wins behavior. data-dir wins over workspace if both are set
# (matches CLI precedence).
DATA_DIR="${VIZIER_DATA_DIR:-${VIZIER_WORKSPACE:-}}"

args=(run)
[ -n "${VIZIER_CONFIG:-}" ]          && args+=(-c "$VIZIER_CONFIG")
[ -n "$DATA_DIR" ]                   && args+=(--data-dir "$DATA_DIR")
[ -n "${VIZIER_PORT:-}" ]            && args+=(--port "$VIZIER_PORT")
[ -n "${VIZIER_STORAGE:-}" ]         && args+=(--storage "$VIZIER_STORAGE")
[ -n "${VIZIER_WORKERS:-}" ]         && args+=(--workers "$VIZIER_WORKERS")
[ -n "${VIZIER_WS_IDLE_TIMEOUT:-}" ] && args+=(--ws-idle-timeout "$VIZIER_WS_IDLE_TIMEOUT")
[ -n "${VIZIER_EXTRA_ARGS:-}" ]      && args+=($VIZIER_EXTRA_ARGS)

# Append explicit user args (everything after "run", if any).
# Guarded: `${@:2}` is empty when $# is 0 or 1, but checking avoids any
# "set -e" issues on edge cases across bash versions.
if [ "$#" -ge 2 ]; then
  args+=("${@:2}")
fi

exec vizier "${args[@]}"
