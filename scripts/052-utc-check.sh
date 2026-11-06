#!/usr/bin/env bash
# Refuses a second clock (specs/052-unify-local-timezone, FR-004). Vizier runs in one zone, the
# host's, and every time it holds is a `time::Timestamp`. A new `Utc::now`, `DateTime<Utc>` or
# `with_timezone(&Utc)` brings the two-clock confusion back, and this names the file and the line.
#
#   scripts/052-utc-check.sh
#
# Exits non-zero on a finding. A text column that is sorted as text writes through
# `time::stored`/`stored_now`, not through a `Utc` of its own.
set -euo pipefail

cd "$(dirname "$0")/.."

ROOTS=(crates app/src-tauri/src)
PATTERN='Utc::now|DateTime<(chrono::)?Utc>|DateTime::<(chrono::)?Utc>|with_timezone\(&(chrono::)?Utc\)'

# The files allowed a UTC clock: the one module that defines the rest, and external formats whose
# protocol names UTC.
ALLOWED=(
  'crates/vizier-core/src/time.rs'    # the fixed sort form and epoch helpers
  'crates/vizier-core/src/llm/sigv4.rs' # AWS request signing
  'crates/vizier-core/src/llm/error.rs' # HTTP `Retry-After` dates
  'crates/vizier-core/src/storage/blob/s3.rs' # the S3 API's date format
)

allowed() {
  local file=$1 entry
  for entry in "${ALLOWED[@]}"; do
    [ "$file" = "$entry" ] && return 0
  done
  return 1
}

fail=0
while IFS=: read -r file line text; do
  [ -n "$file" ] || continue
  allowed "$file" && continue
  echo "$file:$line: a second clock: ${text#"${text%%[![:space:]]*}"}"
  fail=1
done < <(grep -rnE --include='*.rs' "$PATTERN" "${ROOTS[@]}" || true)

if [ "$fail" -ne 0 ]; then
  echo
  echo "Use vizier_core::time (Timestamp, now, stored) instead. See docs/src/configuration/time.md." >&2
  exit 1
fi

echo "one clock: no Utc outside ${#ALLOWED[@]} allowed files"
