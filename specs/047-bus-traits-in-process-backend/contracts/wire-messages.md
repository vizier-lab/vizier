# Contract: Wire Messages

Kinds the engine puts in `Envelope.kind`, and the replies expected. All payloads are JSON of types that already derive `Serialize`/`Deserialize` unless noted.

## Turn path

- **`request`** on `agent.{id}.req`: `{ session, request, turn_id?, resumed }`. `reply_to` is a fresh `reply.{uuid}`. Headers: `traceparent`, `tracestate`; `queued_at_ms` set on first publish and preserved on re-send.
- **`response_frame`** on the reply subject: one `VizierResponse` per message, in order. The stream ends when the requester drops its subscription or the sender is dropped; there is no close frame (see research R5).
- Refusals: publishing to `agent.{id}.req` with no subscriber is `NoResponders`, surfaced to callers as the existing "agent not registered" error, immediately.

## Command channels (request/reply)

| Kind | Request | Reply |
|---|---|---|
| `agent_command` | `AgentCommand` minus its oneshot (variants: Create, Update, Delete, Restart, HealthCheck, …) | `AgentCommandResult` (`HealthCheck` replies `Vec<AgentHealthStatus>`) |
| `file_command` | `Upload { filename, content }` or `Resolve { attachment }` | `FileReply::Record(FileRecord)`, `FileReply::Bytes(Vec<u8>)` or `FileReply::Err { message }` |
| `memory_op` | `MemoryOpRequest` | `MemoryOpResponse` or `Err { message }` |
| `command_request` | `CommandRequest` | `CommandResponse` |

Replies carry errors as `{ message: String }`; the caller rebuilds a `VizierError` at the boundary. `HealthCheck` keeps its 5 s deadline and its two distinct messages ("manager not running" ← `NoResponders`, "did not answer in time" ← `Timeout`).

## Fire-and-forget

- **`dream_command`**: `DreamCommand { agent_id, cycle_id }` on `dream.cmd`.
- **`session_event`** on `session.{id}.events`: `SessionEvent { session, frame }`. Every `SessionFrame` variant must be serialisable (add serde derives where missing).
- **`integration_changed`** on `integration.changed`: `IntegrationChanged`.

## Compatibility rules

- `Envelope.v` is 1. A receiver that sees a higher `v` fails decoding with a clear error rather than guessing.
- Unknown JSON fields in a payload are ignored; adding an optional field is not a version bump.
- `turn_id` and `resumed` ride as headers (`x-vizier-turn-id`, `x-vizier-resumed`) so they survive a hop though the struct fields stay `#[serde(skip)]`.

## Not on the bus

- The process exit signal (`exit_signal`): a single-process lifecycle event (research R7).
- The client-tool live round trip: stays a local socket; only the binding is portable (research R10).
