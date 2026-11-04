#!/usr/bin/env bash
# Refuses a gateway-side read of a running agent's state (specs/163-agent-service-seam,
# FR-003). The agent host is the only component allowed to read a running agent's state;
# the gateway side reaches an agent through the seam (`AgentsClient`) or reads a published
# view. A read of the handles below is a defect, and this names the file and the line.
#
#   scripts/163-agent-state-read-check.sh
#
# Exits non-zero on a finding. The audit (`specs/163-agent-service-seam/contracts/
# agent-state-audit.md`) is the record of what each read was replaced by; the counts at the
# end say how many rows it has and how many handles this check covers, so a new handle
# without an audit row is visible.
#
# The exceptions below are the audited reads that stay where they are. Each one names the
# audit row that explains it, so a new read outside this list is refused.
set -euo pipefail

cd "$(dirname "$0")/.."

ROOTS=(crates/vizier-http/src crates/vizier-core/src/service)
AUDIT=specs/163-agent-service-seam/contracts/agent-state-audit.md

# The handles only the agent host may touch. Each one names a read of a running agent's
# state, or of the registry that held it.
HANDLES=(
  'agent_health\('
  'setup\.snapshot\('
  'channel_board'
  'DeferredClaims'
  'deferred_client_calls[[:space:]]*:'
)

PATTERN="$(IFS='|'; echo "${HANDLES[*]}")"

# The audited exceptions: file | the read | why it stays.
EXCEPTIONS=(
  'crates/vizier-core/src/service/integrations.rs|status_of\(&s\.deps\(\)\.channel_board|the channel-reader health: open, owned by the channels role (#167)'
  'crates/vizier-core/src/service/setup_state.rs|deps\.setup\.snapshot\(\)|this process own board: an agent being created is not in storage yet'
)

fail=0
found=0

# Whether this read is one of the audited exceptions.
audited() {
  local file=$1 text=$2
  local entry
  for entry in "${EXCEPTIONS[@]}"; do
    local except_file except_read
    except_file="${entry%%|*}"
    except_read="${entry#*|}"
    except_read="${except_read%%|*}"
    [ "$file" = "$except_file" ] || continue
    if printf '%s' "$text" | grep -qE "$except_read"; then
      return 0
    fi
  done
  return 1
}

for root in "${ROOTS[@]}"; do
  [ -d "$root" ] || continue
  while IFS= read -r file; do
    while IFS=: read -r line text; do
      # A comment may name a handle freely; only code counts.
      trimmed="${text#"${text%%[![:space:]]*}"}"
      case "$trimmed" in
        "//"*) continue ;;
      esac
      if audited "$file" "$trimmed"; then
        echo "audited exception: $file:$line $trimmed"
        continue
      fi
      echo "::error::$file:$line reads a running agent's state: $trimmed"
      found=$((found + 1))
      fail=1
    done < <(grep -nE "$PATTERN" "$file" || true)
  done < <(find "$root" -name '*.rs' -type f | sort)
done

# The audit's completeness: how many rows it has, and how many handles this check covers.
if [ -f "$AUDIT" ]; then
  rows=$(grep -cE '^\| ' "$AUDIT" || true)
  echo "agent-state audit: $rows rows, ${#HANDLES[@]} handles covered, ${#EXCEPTIONS[@]} audited exceptions"
else
  echo "::error::$AUDIT is missing"
  fail=1
fi

if [ "$fail" = 0 ]; then
  echo "agent-state read check: no un-audited direct read of a running agent's state"
fi
exit "$fail"
