# Contract: Subjects

## Grammar

```text
subject  = token *( "." token )
token    = 1*( ALPHA / DIGIT / "_" / "-" / "%" / "~" )
pattern  = ptoken *( "." ptoken ) [ "." ">" ]  /  ">"
ptoken   = token / "*"
```

- Maximum length 255 bytes; no empty tokens.
- `*` matches exactly one token. `>` matches one or more tokens and is valid only as the last token of a pattern.
- Identifier encoding: every byte outside `[A-Za-z0-9_-~]` is written `%XX` (uppercase hex), so `my.agent` → `my%2Eagent` and the result is always a single token. Decoding is the inverse.

## Subject table

| Subject | Direction | Payload kind | Owner today |
|---|---|---|---|
| `agent.{id}.req` | request into an agent | `request` | engine (agent manager) |
| `reply.{uuid}` | reply frames of one request | `response_frame` | requester subscribes first |
| `agent.{id}.memop` | memory operation request/reply | `memory_op` | agent process |
| `agents.cmd` | agent lifecycle request/reply | `agent_command` | agent manager |
| `files.cmd` | upload / resolve request/reply | `file_command` | file service |
| `dream.cmd` | dream trigger | `dream_command` | scheduler |
| `process.cmd` | command-socket request/reply | `command_request` | process command handler |
| `session.{id}.events` | session-event fan-out | `session_event` | any publisher; WebUI / channel readers subscribe `session.>` |
| `integration.changed` | integration write fan-out | `integration_changed` | integration service |
| `lease.{name}` | lease name space (e.g. `lease.leader`, `lease.agent.{id}`) | n/a (Leases trait) | role contracts |
| `gateway.{node}.client` | **reserved**, created by the HTTP gateway work (#169) | n/a | not in this feature |

`{id}` and `{name}` segments that are identifiers are encoded per the grammar; `{uuid}` is a v4 UUID in simple (no-dash) form.

## Role contract skeletons

Each of `gateway_http`, `gateway_grpc`, `worker`, `channels`, `control` has a `RoleContract` constant listing the patterns it subscribes to, the subjects it publishes, and the leases it takes. All are empty in this feature; their sub-issues (#166–#170) fill them in, and `validate()` already checks any entries added (valid patterns, no duplicates, leases are valid names).
