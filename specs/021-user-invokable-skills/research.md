# Research: User-Invokable Skills

**Feature**: `021-user-invokable-skills`

## R1 — Where to resolve an invocation

- **Decision**: In `agents/process.rs`'s `VizierRequestContent::Chat` arm, before `recommend_skills`.
- **Rationale**: Discord, Telegram and the WebUI all produce `Chat(text)` for an addressed person's message (`discord/mod.rs` ~737, `telegram/mod.rs` ~608, WebUI socket). One place gives FR-001/FR-002 on all three channels, satisfies Principle II, and means the dummyplug e2e exercises the real path on any channel.
- **Alternatives**: resolve in each channel and send a new `VizierRequestContent::SkillInvocation` (three copies of parsing, a new enum arm in every match, and the WebUI would need a new socket message); a platform slash-command per skill (Discord caps global commands at 100, needs re-registration on every skill change, Telegram menus are per-bot not per-agent-skill).

## R2 — Command form

- **Decision**: `/<name> [input]` (case-insensitive, `@botname` suffix stripped), `/skill <name> [input]` as the unshadowable form, and `/skills` to list. Built-ins (`abort`, `checkpoint`, `lobotomy`, `new`, `session`, `help`, …) never reach the `Chat` arm as text on the WebUI (sent as `Command`), and are intercepted earlier on Telegram and Discord, so they win automatically (FR-006). `/skill` and `/skills` are reserved and cannot be shadowed by a skill of that name.
- **Rationale**: matches existing `/abort` style; `/skill <name>` is also how a user reaches a skill whose name equals a platform command.

## R3 — How the model sees the invocation

- **Decision**: the full skill content goes into a `## Invoked Skill` section of the per-request context block (user message, not system prompt); the model's user message is the input text, or "Run the skill." when there is none. The saved history `Request` keeps the person's original text with `invoked_skill` in metadata.
- **Rationale**: keeps system prompts and replayed history byte-stable for provider caching (`context.rs` SC-013); the existing "Possibly Related Skills" block only lists names and asks the model to load a skill through `use_skill`, which is exactly the non-determinism FR-002 removes. Replay later shows "/release-notes v1.2", which is truthful and needs no skill text.
- **Alternatives**: rewriting the user message to "Use skill X: …" (loses what the person typed in the UI); loading via a forced `use_skill` tool call (an extra model round and still model-dependent).
- **Dedup**: the invoked skill is removed from the recommended list for that turn.

## R4 — Direct replies without a model turn

- **Decision**: `/skills` and a not-found name are answered from `process.rs` through the request's `response_tx` as a normal `Message` response, and saved to history as `Request` + `Response` so the WebUI shows them on reload.
- **Rationale**: SC-004 (no model call). The existing "Dream cycle started." reply shows the pattern of answering through `response_tx`. To verify during implementation: that a direct `Response` saves cleanly without `VizierResponseStats` (usage accounting records only model calls, so nothing is written).

## R5 — Which leading-slash text is a not-found

- **Decision**: a message whose first token is `/` followed by `[A-Za-z0-9][A-Za-z0-9_-]*` and that matches no skill gets the not-found reply with up to 3 closest names; anything else (paths such as `/etc/hosts`, `/ ` , URLs) stays ordinary chat.
- **Rationale**: FR-007's two halves. Single-token command-shaped text is almost always a mistyped command; a path contains a second `/`.
- **Reuse**: the closest-names helper is the one behind the existing `did_you_mean` data for unknown tool categories; if it is private to the tools module, it is moved to a shared place rather than copied (Principle II).

## R6 — Addressing in groups (FR-008)

- **Discord — per-skill slash commands (revised)**: each skill is registered as a global slash command of that bot, `/<slug>` with an optional string option `input`. The picker shows which bot owns a command, so two agents with the same skill never clash and no mention is needed. The existing `ready()` already registers a fixed list with `set_global_commands`; it now registers `slash_commands()` plus one command per invokable skill (bulk overwrite is idempotent). The `interaction()` handler treats a command that is not a built-in as a skill: it defers the response, sends `Chat("/<slug> <input>")` into the channel's active session, and streams the turn with the same `TurnRenderer` flow the text path uses, so resolution still happens once in `process.rs` (R1).
- **Discord `/skill`**: one more slash command, `/skill name:<autocomplete> input:<text>`. The `name` option answers autocomplete interactions from the agent's skills (up to 25 matches, filtered by what is typed). It is the mention-free route for every skill without its own command, and it handles the `Chat("/skill <name> <input>")` form like the per-skill commands do.
- **Discord limits**: command names must be 1–32 characters of lowercase letters, digits, `-` or `_`; a skill name that does not fit (or collides with a built-in command, or is beyond the cap of 100 global commands minus the built-ins) gets no command of its own, is logged once, and stays reachable through `/skill`. Descriptions are cut to 100 characters.
- **Discord — keeping the set fresh (SC-005)**: skills change through the HTTP API, the agent's own tools and `vizier skill install`, with no shared event. The Discord reader therefore re-reads the agent's skill list every 60 s and calls `set_global_commands` only when the resulting name/description set changed (command registration is rate limited, so no unconditional writes). Typed text keeps working regardless, since it resolves at call time.
- **Discord text path**: unchanged for DMs and mentions (`render_mentions` already strips mentions, so `@bot /release-notes x` parses). Unmentioned guild text is not treated as an invocation, because the slash command is the proper route there.
- **Telegram**: today only a mention, DM or `/<bot_username>` counts as addressed. Add: a text starting `/<word>@<bot_username>` is addressed (Telegram's own group form), and a group message becomes `Chat` when `skill::invocation::names_skill(skills, text)` is true (parses as `Invoke` — either `/<name> …` or `/skill <name> …` — and the name exactly matches one of this agent's skills). `/skills` and not-found replies (including `/skill <wrong name>`) still need an address, so two bots in one group do not both answer unrelated slash text; accepted cost: two agents with a same-named skill both run it. The helper reads the skill folders (cheap, R8) and is not called for DMs or addressed messages. A bot with privacy mode on only receives commands addressed to it; that is a BotFather setting, documented, not worked around.
- **WebUI**: always addressed.

## R7 — Discovery

- **Decision**: `/skills` text reply on Telegram and in the WebUI/typed form; WebUI `/` suggestions fed by `GET /agents/{id}/skills/available` (agent and global, agent winning, name + description only); on Discord the slash-command picker is the list (R6), and `/skills` typed as text still answers.
- **Telegram native menu (`setMyCommands`)**: out of scope; it is set per bot, not per conversation, and `/skills` covers discovery.
- **Why a new endpoint**: the existing `GET /agents/{id}/skills` lists the agent's own skills only (`SkillManager::for_agent`), not the global ones the agent can also use, and returns full content.

## R8 — Freshness (SC-005)

`VizierSkills::all_skills()` reads the directories on each call (no cache), so install/remove is visible on the next message with no restart.

## R9 — Skills that need the skill's `resources`

The existing `use_skill` tool describes resource handling. The invoked-skill section includes the skill's `resources` list the same way `use_skill` returns it, so a skill behaves identically however it is loaded (verify the exact shape when implementing).
