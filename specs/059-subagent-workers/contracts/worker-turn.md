# Contract: a worker's turn, its delivery and its frame

**Feature**: `059-subagent-workers`

## Prompt layout of a worker turn

System messages, in order (main-turn parts not listed are absent):

1. `worker_md(agent name, description)`: you are a temporary worker for `<name>`; one task; your
   answer goes to `<name>`, not to a person; you act with `<name>`'s tools and they are credited to
   it; if blocked, ask `<name>` with `ask_main_agent` (it ends your step); end with a report:
   what you did, what you could not do and why, how sure you are; no greetings or offers.
2. Sandbox (`sandbox_md`, the turn's categories).
3. `## The person <name> serves` + the owner profile text, when there is one.
4. `## The agent you work for` + the identity block (`<memory bundle="core" path="soul">…`).
5. The project block, when the spawn came from a project chat.
6. The agent's configured documents.

The agent's configured `system_prompt` and the conversational `boot_md` are not included.

## Frontmatter

First request (`SubagentTask`):

```yaml
sender: <agent id>
via: subagent
subagent: s_…
task: 2 of 4
sent_at: …
```

Reply (`SubagentReply`):

```yaml
sender: <agent id>
via: subagent_reply
subagent: s_…
sent_at: …
```

## Delivery text (one section per outcome, inside the existing background delivery message)

```
[Subagent update] s_… — task 2 of 4 "<task, ≤120 chars>"
state: answered
result: <text, cut with a marker>
```

```
[Subagent update] s_… — task 3 of 4 "<task>"
state: asking
question: <question>
reply with reply_subagent(id="s_…", text=…)
```

`failed` and `timed_out` carry `error:` instead of `result:`. Sections from detached scripts and
subagents ready at the same moment share one message, scripts first.

## Session frame

`SessionFrame::Subagent(SubagentSnapshot)` on the **hiring** session, socket frame name
`subagent`, published after every committed change (spawn, ask, reply, settle, stop, delete). The
payload is the tool summary plus `group`, `hiring_turn`, `parent_detached_id`, `updated_at`.
Consumers re-read the row on receipt; the frame is a wake-up, not the source of truth.
