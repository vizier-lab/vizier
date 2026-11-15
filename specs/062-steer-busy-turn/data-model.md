# Data Model: Steer a Busy Turn

No new tables. Two request fields, one in-memory structure, one response frame, one storage result type.

## `VizierRequest` — new fields (`schema/request.rs`)

| Field | Type | Serde | Set by | Meaning |
|---|---|---|---|---|
| `queue` | `bool` | `default`, `skip_serializing_if = is_false` | a client (WebUI Ctrl/Cmd+Enter), or `steer::strip_queue_command` on receipt | Run as a turn of its own after the current one; never steer. Rides the `session_turn` row, so it survives a restart. |
| `mid_turn` | `bool` | `default`, `skip_serializing_if = is_false` | the engine only, when the message is saved into a running turn | This `Request` history entry was read by a turn already running. Cleared from client input on receipt (`stamp_on_receipt`) and forced `false` before queuing. |

New helper: `VizierRequest::opens_turn(&self) -> bool` = `!(matches!(content, DetachedDelivery(_)) || mid_turn)`. Every place that looks for "where this turn started" uses it.

Frontmatter (`generate_frontmatter`): a `mid_turn` request keeps `sender: <person>` and adds `via: mid_turn`.

## `Mode` and `Queued` (`agents/steer.rs`, in memory)

```text
enum Mode { Steer, Absorb, Queue }   // Absorb: a SilentRead, taken at every boundary (R12)

struct Queued {
    request:     VizierRequest,
    response_tx: Option<ResponseSender>,
    turn:        TurnRef,          // journal row id + resumed flag, as today
    mode:        Mode,             // decided once by steer::mode() when queued
}

type SessionQueue = Arc<std::sync::Mutex<VecDeque<Queued>>>;   // shared: loop + running turn
struct SteerInbox(SessionQueue);   // the turn's clone; ToolContext.steer: Option<SteerInbox>
```

`mode(request, session)` → `Queue` when any of: `request.queue`; `request.transient`; content is not `Chat`; the text parses as a skill command; the session has no person (`Task`, `Dream`, `Subagent`, inter-agent activation). A `SilentRead` in a person session (not transient) → `Absorb`. Otherwise `Steer`.

`SteerInbox::take() -> Vec<Queued>` removes every `Steer` and `Absorb` item, keeping the others in place and in order.

### Lifecycle of a waiting message

```text
                    ┌────────── queued (mode = Queue) ─────────────┐
receipt ── busy? ──►│                                              ├─► popped by the loop ─► own turn
   │                └────────── queued (mode = Steer) ──┬──────────┘        (turn ended first)
   │ idle                                               │ boundary: take + steer_turns
   ▼                                                    ▼
own turn                                          steered into the running turn
                     /abort (any mode) ─► dropped, row cancelled (as today)
```

Each message ends in exactly one of: *own turn*, *steered*, *dropped by abort / agent delete*.

## `SteeredTurn` (`storage/turn.rs`)

```text
fn steer_turns(&self, session: &VizierSession, items: &[(Option<i64>, VizierRequest)])
    -> Result<Vec<SteeredTurn>>;

struct SteeredTurn { index: usize, history_uid: String }   // index into `items`
```

One fenced transaction. For each item: `DELETE FROM session_turn WHERE id = ? AND state = 'queued'`; if one row went (or the item has no id), insert its `Request` history entry with `mid_turn = true` via `write_history_entry(tx, …)` and return it; otherwise skip it (abort won). Refused with `OwnershipLost` under a newer owner.

## `VizierResponseContent::Steered` (`schema/response.rs`)

A unit variant sent once on a steered message's own `response_tx`, then the sender is dropped. `ends_turn()` is `true`. Serialised `"steered"`.

## History (`session_history`)

A steered message is a `SessionHistoryContent::Request` row:

| Part | Value |
|---|---|
| row `timestamp` | when the turn read it (orders it after the round's last `ToolResult`) |
| `request.timestamp` | when the server received it (`sent_at`) |
| `request.content` | `Chat(text)`, `/queue` never present |
| `request.attachments` | as sent (moved into session files at read, as at a turn start) |
| `request.mid_turn` | `true` |

An absorbed silent read is the same, with `SilentRead` content. In a Workspace session, the boundary's catch-up copies unread entries as `SilentRead` rows with the entry's own time (as an activation does).

## WebUI types (`webui/app/lib/trail.ts`)

```text
type Block = … | { kind: 'steer'; entry: ChatMessage }   // inside Turn.blocks
```

`groupHistory`: a `Request` with `mid_turn` and `chat` content while a turn is open → push a `steer` block to that turn. Rendering splits `Turn.blocks` at each `steer` block into trail parts.
