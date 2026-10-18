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

- **Decision (revised: no mention needed)**: a group message becomes `Chat` when it is addressed (as today) **or** when `skill::invocation::names_skill(skills, text)` is true, i.e. it parses as `Invoke`/`Unknown` and the name exactly matches one of this agent's skills. Everything else unaddressed stays `SilentRead`. So `/skills`, not-found replies and `/skill <name>` still need an address; only the bare `/<existing-skill>` form is open.
- **Why not fully open**: with two bots in one group, an open `/skills` or not-found reply would be answered by both; an exact-match run only fires for agents that really have the skill. Cost accepted: two agents with a same-named skill both run it.
- **Where**: one shared helper called by both readers (`discord/mod.rs` before the `SilentRead` choice ~line 733, `telegram/mod.rs` `should_respond` ~line 594); it reads the agent's skill folders (cheap, as in R8) and is not called for DMs or addressed messages. Discord needs the message-content intent it already uses to read the text. Telegram groups where the bot has privacy mode on only deliver commands/mentions to the bot; that is a BotFather setting and is documented, not worked around.
- **Telegram**: also treat `/<word>@<bot_username>` as addressed (Telegram's own group command form); the parser strips the `@bot` suffix. **Discord**: `render_mentions` already strips mentions, so `@bot /release-notes x` still parses.
- **WebUI**: always addressed.

## R7 — Discovery

- **Decision**: `/skills` text reply on all channels; WebUI `/` suggestions fed by `GET /agents/{id}/skills/available` (agent and global, agent winning, name + description only). Platform-native menus (Discord slash options, Telegram `setMyCommands`) are out of scope: skills are per agent and dynamic, which does not fit global command registration (assumption in spec).
- **Why a new endpoint**: the existing `GET /agents/{id}/skills` lists the agent's own skills only (`SkillManager::for_agent`), not the global ones the agent can also use, and returns full content.

## R8 — Freshness (SC-005)

`VizierSkills::all_skills()` reads the directories on each call (no cache), so install/remove is visible on the next message with no restart.

## R9 — Skills that need the skill's `resources`

The existing `use_skill` tool describes resource handling. The invoked-skill section includes the skill's `resources` list the same way `use_skill` returns it, so a skill behaves identically however it is loaded (verify the exact shape when implementing).
