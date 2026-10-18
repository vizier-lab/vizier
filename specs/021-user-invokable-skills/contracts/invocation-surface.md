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

**Discord**: each skill is a slash command of that bot, `/<slug> [input]`, picked in Discord's command picker (no mention needed; the picker shows which bot). Names Discord rejects (not 1–32 of `a-z0-9_-`), clashes with a built-in command, or skills past the command cap are not registered and are reached by typed text (`@bot /skill <name>`, or DM). Typed text with a mention or in a DM follows the grammar above. The command set is re-synced within 60 s of a skill being added, changed or removed.

**Telegram**: no mention needed for `/<name> …` in a group when `<name>` exactly matches one of the agent's skills. `/skills`, `/skill …` and not-found replies need the bot addressed (`/<cmd>@<bot>` or a mention). DMs always. The bot must receive group messages (privacy mode off, or the command addressed with `@bot`).

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
