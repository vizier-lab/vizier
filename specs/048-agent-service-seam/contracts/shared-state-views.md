# Contract: the published views (shared state)

**Feature**: #163 | **Date**: 2026-11-03

What the host publishes so a gateway can read agent state without knowing any owner.

---

## Why shared state

A gateway cannot ask "which node owns agent X" - the seam never reveals the owner (FR-007). So a
fleet-wide view cannot be assembled by asking each owner. The bus has no snapshot either, so a
subscriber that connects after the last publish would see nothing. Shared state is the one place
both sides can meet: the owner writes, any gateway reads.

---

## The keys

| Key | Written by | Read by |
|---|---|---|
| `agent.{id}.health` | The host that holds the agent | `core::service` (the agent list, the health endpoint, the overview counts) |
| `agent.{id}.setup` | The host that holds the agent | `core::service` (the setup-state endpoint the WebUI polls) |

The id is one percent-encoded token, so it cannot widen the prefix a reader watches.

---

## The value

```json
{
  "expires_at_ms": 1730000000000,
  "data": { }
}
```

| Field | Type | Rules |
|---|---|---|
| `expires_at_ms` | integer | Epoch milliseconds. **At most 10 s** after the publish (FR-010). |
| `data` | JSON | The view's payload. |

**The deadline is in the value, not in the backend.** `SharedState` has no TTL: it is
`get`/`put`/`cas`/`delete`/`watch` over bytes with a revision
(`crates/vizier-schema/src/state.rs:33-49`). Carrying the deadline in the value is what lets the
same contract hold on the in-process backend and on a networked one, and it keeps the leaf contract
crate unchanged through the #162-#164 conflict window.

**A backend TTL is the better long-term shape** and is what NATS KV offers natively. It is #165's
work, when the networked backend lands: extend `SharedState` with a TTL-aware write, add the
conformance check, and let the backend expire the key.

---

## Reading a view

```rust
match state.get(&key).await {
    Ok(Some(versioned)) => match decode_view(&versioned.value) {
        Ok(view) if !view.is_expired(now_ms()) => view.data,
        // Expired: unknown, not a stale value.
        _ => Unknown,
    },
    _ => Unknown,
}
```

**Rules**

1. A missing entry reads as **unknown**.
2. An expired entry reads as **unknown**. It is never presented as a stale value (FR-010).
3. A view is never more than 10 s old when it is shown (SC-008).
4. A reader that finds unknown says so. It does not fall back to a stored row for a *live* value -
   the stored configuration is a different thing and is read from storage.

**When the host publishes** - the rule that keeps single node unchanged:

5. The host publishes **when it takes an agent** and **whenever that agent's state changes**.
   It also **refreshes** the views it holds, so a view is never older than its deadline. The
   refresh is needed for rule 3 to mean anything: an idle running agent's state does not
   change, so without it its view would expire and the WebUI would show it as unknown - a
   visible regression in one node, and SC-008 unachievable. A view is therefore a moment-old
   truth that is refreshed, not a poll of the agent's state.
6. The first read after startup must not be "unknown" for an agent that is running. The host
   publishes on take, so the window between the agent starting and its first publish is the
   publish itself, not the ten-second deadline.
7. The view carries **everything the reader used to get from the agent's memory**, including the
   channel-reader health the agent's health folds in today
   (`agents/mod.rs:251`, `AgentStatus::health(&process.status, &self.deps.channel_board, id)`). If
   the view dropped it, the WebUI's agent list would lose that column in single node.

**Why 5-7 matter.** Today `GET /agents/health` is a live query of the manager. After the change it
is a read of a published view. Without these rules the single node would show "unknown" for a
running agent, or a run state that is a poll out of date - a visible regression in the WebUI, and
the one place where this feature could change what a person sees.

---

## What each view carries

### `agent.{id}.health`

The agent's run state, as the supervisor holds it today
(`crates/vizier-core/src/agents/supervisor.rs`): the run state, the last error, the failure count,
the next retry time, the model and the warnings.

The reader adds what it already holds: whether the agent has an open turn (a stored read -
`storage.thinking_agents()`), and the channel-reader health it holds locally.

### `agent.{id}.setup`

The agent's build phase, message, error and download progress, as the setup board holds it today
(`crates/vizier-core/src/setup.rs:74-137`). The board keeps its in-memory form - it is the *live*
progress - and publishes a copy for the gateways that are not in this process.

**A gateway reads both.** The published view covers the agents another node holds; this
process's own board is still read as well, because an agent being *created* is not in storage
yet, so the view cannot be found by listing the stored agents. The board's own entries win, so
in one node what is shown is exactly what it was before. The read of the local board is an
audited exception in the mechanical check.

**The setup board is the publisher.** One publisher, one place: not a `put` call scattered over the
call sites that touch setup (Principle II).

---

## The channel-reader health

The channel board (`deps.channel_board`) holds each reader's health. It is **not** published as a
view in this release: a reader belongs to the `channels` role, and the audit records what happens to
it (see [agent-state-audit.md](agent-state-audit.md)). Publishing it here would need a per-reader
key and a publisher in the channels role - #167's work, not this issue's.
