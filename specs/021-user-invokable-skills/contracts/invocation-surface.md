# Contract: Invocation Surface

**Feature**: `021-user-invokable-skills`

## Message grammar (all channels)

| Message | Result |
|---|---|
| `/<name>` | Runs the skill; the recent conversation is the input |
| `/<name> <input>` | Runs the skill with `<input>` as the message |
| `/skill <name> [input]` | Same, for names that clash with a platform command |
| `/skills` | Replies with the list: one line per skill, `name — description` |
| `/<unknown>` (command-shaped, no match) | Replies `No skill named "<unknown>".` plus `Did you mean: a, b, c?` when there are near matches, and `/skills` hint; no model call |
| `/path/with/slash`, `/ x`, URLs | Ordinary chat |

Empty list reply: `No skills are available to this agent.`

Telegram groups: `/<name>@<bot> …` is addressed to the bot; plain `/<name> …` in a group the bot was not addressed in is not acted on. Discord: needs a mention in guilds; DMs always.

## HTTP

`GET /api/v1/agents/{agent_id}/skills/available` (JWT, same visibility rule as the agent's other skill reads)

```json
{ "status": 200, "data": [ { "name": "release-notes", "description": "…" } ] }
```

Agent skills override global ones of the same name; sorted by name; no content returned. `404` unknown agent, `403` when the person cannot view the agent. The WebSocket and chat message shapes are unchanged: an invocation is a normal chat message.

## What the agent sees

In the user message's context block, before other sections:

```text
## Invoked Skill
The person invoked this skill by name. Follow it for this message.

**release-notes**: <description>

<skill content>
```

and the user message text is the input (or `Run the skill.`).
