# Contract: Choosing "queue as a new turn"

Steering is the default. These are the only ways to choose the old behaviour.

## Q1 — The request field

`VizierRequest.queue: true` (JSON `"queue": true` on the chat request a client sends). Absent or `false` = steer when busy. Ignored when the agent is idle (the message starts a turn either way).

## Q2 — `/queue` text

Recognised by core on receipt for every channel, in `Chat` text only:

```text
^/queue(@<bot>)?(\s+<message>)?$        case-insensitive; leading whitespace ignored
```

| Input | Result |
|---|---|
| `/queue check the logs too` | `queue = true`, text becomes `check the logs too` |
| `/QUEUE@vizier_bot check` (Telegram) | same |
| `/queue` or `/queue   ` | answered directly with a usage line; no turn, nothing saved |
| `/queueing is fun` | ordinary text (no word boundary) |
| `/skill queue …` | the skill named `queue`, as today |

`queue` is a reserved command name: a skill called `queue` gets no `/queue` command of its own on any platform.

## Q3 — Per channel

| Channel | How |
|---|---|
| WebUI (desktop) | **Ctrl+Enter** (Windows/Linux) / **Cmd+Enter** (macOS) sends with `queue: true`. Enter sends normally. Neither fires while a mention or suggestion list is open (Enter picks the item there). While the agent is idle both keys simply send. |
| WebUI (any size, incl. phone) | `/queue <message>` typed in the composer. |
| Discord | slash command `/queue` with a required `message` option, registered with the other builtins; turned back into `Chat("/queue <message>")`. Typing `/queue …` as plain text works too. |
| Telegram | `/queue <message>` (listed in the bot's command menu). |
| Workspaces | `/queue @<agent-id> <message>`; the agent receives `/queue <message>`. People only, like other Workspace commands. |

## Q4 — Hint

The WebUI composer shows, only while the agent is busy: `Enter adds to the current turn · Ctrl+Enter queues it` (`⌘↵` on macOS). Exact copy is settled in the design review (contracts/webui.md).
