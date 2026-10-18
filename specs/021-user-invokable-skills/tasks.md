---

description: "Task list for 021-user-invokable-skills"
---

# Tasks: User-Invokable Skills

**Input**: Design documents from `/specs/021-user-invokable-skills/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/invocation-surface.md](contracts/invocation-surface.md), [quickstart.md](quickstart.md)

**Tests**: The spec does not ask for TDD. The plan names unit tests for the pure parts (parser, command-list builder), and the constitution requires the dummyplug e2e in quickstart.md; only those are included.

**Organization**: grouped by user story (US1 invoke, US2 discover, US3 not-found feedback).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1, US2, US3 from spec.md

---

## Phase 1: Setup

No project setup needed: no new crate, table, config or migration (so no #82 entry). Skip to Phase 2.

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the pure parser and the skill resolver every channel and story uses.

- [ ] T001 Create `src/skill/invocation.rs` with `pub enum Parsed { Plain, List, Invoke { name: String, input: String } }` and `pub fn parse(text: &str) -> Parsed`: trim; `/skills` (optionally `@<anything>` suffix) → `List`; `/skill <name> [input]` → `Invoke`; `/<token>[@bot] [input]` where `<token>` matches `^[A-Za-z0-9][A-Za-z0-9_-]*$` → `Invoke` (name lower-cased, `@…` suffix stripped, input trimmed, may be empty); anything else (including `/a/b`, `/ x`, URLs, empty `/`) → `Plain`. Built-in names are not special here (they never reach the `Chat` arm as commands, research R2). Add `pub mod invocation;` in `src/skill/mod.rs`.
- [ ] T002 Add `pub fn same_name(a: &str, b: &str) -> bool` to `src/skill/invocation.rs`: case-insensitive, treating `_` and `-` as equal (research R10, Telegram), and `pub const RESERVED: [&str; 2] = ["skill", "skills"]`.
- [ ] T003 Unit tests in `src/skill/invocation.rs` (`#[cfg(test)]`): `/release-notes v1.2` → Invoke(release-notes, "v1.2"); `/Release-Notes` → name lower-cased, empty input; `/skill standup hi` → Invoke(standup, "hi"); `/release_notes@mybot x` → Invoke(release_notes, "x"); `/skills`, `/skills@bot` → List; `/etc/hosts`, `/ x`, `hello /x`, `https://a/b` → Plain; `same_name("release_notes","Release-Notes")` is true.
- [ ] T004 In `src/agents/skill/mod.rs` add `pub fn resolve(&self, name: &str) -> crate::Result<Option<Skill>>` (agent skill first, then global, matched with `invocation::same_name` against `skill.name`; if more than one skill matches only through `_`/`-` folding, prefer the exact name; skills named in `invocation::RESERVED` never resolve) and `pub fn available(&self) -> crate::Result<Vec<Skill>>` (the existing `all_skills()` merged list minus reserved names, sorted by name). Reuse `all_skills()`; do not cache (research R8).

**Checkpoint**: `cargo test skill::invocation` passes.

---

## Phase 3: User Story 1 — Run a skill by name from chat (P1) 🎯 MVP

**Goal**: `/<skill> [input]` and `/skill <name> [input]` run that skill on the WebUI, Telegram and Discord, with no mention needed in groups.

**Independent test**: quickstart.md steps 1–3 (dummyplug, WebUI) and step 8 (Telegram/Discord bots).

### Agent path (shared by all channels)

- [ ] T005 [US1] In `src/agents/agent/system_prompt/context.rs` change `context_md(memory, skills, reactions)` to `context_md(memory, skills, invoked: Option<&Skill>, reactions)` and, when `invoked` is `Some`, push first (right after the Time section) a `## Invoked Skill` section: `The person invoked this skill by name. Follow it for this message.`, then `**{name}**: {description}`, the skill's `content`, and its `resources` listed the same way `src/agents/tools/skill/use_skill.rs` returns them (research R9). Skip the invoked skill in the "Possibly Related Skills" list. Add a unit test next to the existing ones asserting the section appears and the skill is not repeated in the recommendations.
- [ ] T006 [US1] Update every other `context_md(` call to pass `None` for `invoked`: `src/agents/agent/mod.rs` (~line 1341), `src/agents/process.rs` tests (~1547–1583), `src/agents/agent/model/dummyplug.rs` tests (~591, ~601), `src/agents/agent/system_prompt/context.rs` tests.
- [ ] T007 [US1] In `src/agents/agent/mod.rs` add `invoked: Option<Skill>` to `VizierAgent::chat` and `chat_turn` (after `skills`). In `chat_turn`, keep `req` as saved to history unchanged (the person's original text, research R3); build the model message from a clone whose `Chat` text is the invocation input, or `Run the skill.` when the input is empty; pass `invoked.as_ref()` to `context_md` at ~line 679. Update the three `.chat(` call sites in `src/agents/process.rs` (~1176, ~1224, ~1354): only the `Chat` arm passes a value; the others pass `None`.
- [ ] T008 [US1] In `src/agents/process.rs`'s `VizierRequestContent::Chat` arm (before `recommend_skills`, ~line 1174): `match invocation::parse(&prompt)`; on `Invoke{name,input}` call `agent.skills().resolve(&name)` (add a `pub fn skills(&self) -> &VizierSkills` accessor on `VizierAgent` if none exists); if found, set `request.metadata["invoked_skill"] = name` (merge into the existing object), pass the input on to `chat` (T007) with `Some(skill)`, and run `retrieve_auto_context`/`recommend_skills` on the input rather than the command text. If not found, or `List`, fall through to normal chat for now (US2/US3 replace these). `Plain` is unchanged.

### Discord

- [ ] T009 [US1] Create `src/channels/commands.rs` (add `pub mod commands;` in `src/channels/mod.rs`): `pub struct CommandEntry { name, description, options: Vec<CommandOption> }` (`PartialEq`), `pub struct PlatformRules { name_ok: fn(&str) -> bool, convert: fn(&str) -> String, max_commands: usize, max_description: usize, builtins: Vec<CommandEntry> }`, and pure `pub fn build(skills: &[Skill], rules: &PlatformRules) -> (Vec<CommandEntry>, Vec<(String, &'static str)>)` per research R10 (built-ins, then a `skill` entry with `name` + `input` options, then skills sorted by name; converted name; drop invalid, built-in-clashing, reserved or post-conversion-ambiguous names keeping the exact-name one; truncate descriptions on a char boundary; stop at the cap). Returns skipped skills with a reason. Unit tests: ordering, cap, truncation, clash with a built-in, Telegram `-`→`_` ambiguity.
- [ ] T010 [US1] In `src/channels/commands.rs` add `pub fn watch<F, Fut>(agent_id, deps: VizierDependencies, rules: PlatformRules, send: F) -> AbortOnDrop` (reuse the abort-on-drop guard type `src/channels/woken.rs` returns, or the same pattern): spawned task, ticks every 60 s, rebuilds via `VizierSkills`/`SkillManager` for the agent, logs each newly skipped skill once (`tracing::warn!`), and calls `send(list)` only when the list differs from the last one `send` returned `Ok` for (research R11). Expose `pub fn mark_sent(...)` or take an initial `Option<Vec<CommandEntry>>` so the reader's first call is recorded.
- [ ] T011 [US1] In `src/channels/discord/mod.rs`: define Discord `PlatformRules` (names 1–32 of `a-z0-9_-`, description ≤100, max 100, built-ins = current `slash_commands()` entries); convert `CommandEntry` → twilight `Command` (each skill command gets an optional `StringBuilder` option `input`; `skill` gets a required `name` string option with `.autocomplete(true)` and optional `input`). `Handler::ready()` (~line 248) sends this full list via `set_global_commands` and stores `ready.application.id` in a new `OnceLock<Id<ApplicationMarker>>` on `Handler`. In `run()` (~line 72) hold `let _commands = commands::watch(...)` beside `_woken`, whose `send` calls `set_global_commands` once the application id is set (no-op before).
- [ ] T012 [US1] In `Handler::interaction()` (`src/channels/discord/mod.rs` ~line 283): handle `InteractionData::ApplicationCommandAutocomplete` for `skill` (answer with up to 25 of the agent's `available()` skills whose names start with / contain the typed value, `InteractionResponseType::ApplicationCommandAutocompleteResult`); in the `match data.name` fallback `_ => {}` (~line 449), treat any other command as a skill: respond `DeferredChannelMessageWithSource`, build text `"/{name} {input}"` (or `"/skill {name} {input}"` for `skill`), send it as `VizierRequestContent::Chat` with the interaction user's frontmatter/metadata built the same way `message()` builds them (~lines 700–745), into the channel's active topic session, and stream the response through the existing `TurnRenderer` used by `message()`; finish the deferred interaction with a short followup so Discord stops showing "thinking".
- [ ] T013 [US1] Typed Discord text is unchanged (DM or mention only, research R6). Verify that `@bot /release-notes x` reaches the `Chat` arm as `/release-notes x` after `render_mentions` and trimming in `message()`; trim leading whitespace there if it does not.

### Telegram

- [ ] T014 [US1] In `src/channels/telegram/mod.rs` `handle_message` (~line 594): extend `should_respond` so that it is also true when (a) the text starts with `/<word>@<bot_username>` or (b) `invocation::parse(&text)` is `Invoke{name,..}` and `VizierSkills`/`SkillManager` for this agent `resolve(name)` finds a skill (research R6). Strip a trailing `@<bot_username>` from the first token in `cleaned` so the agent sees `/release-notes x`. Do not call the skill lookup for DMs or already-addressed messages.

**Checkpoint**: quickstart steps 1–3 pass on a dummyplug agent in the WebUI; step 8 invocations run on both bots.

---

## Phase 4: User Story 2 — Discover which skills can be invoked (P2)

**Goal**: `/skills` everywhere, the WebUI `/` suggestions, the Discord picker (from US1) and the Telegram menu.

**Independent test**: quickstart step 4; Telegram menu in step 8.

- [ ] T015 [US2] In `src/agents/process.rs` `Chat` arm, on `Parsed::List`: build the reply from `available()` — one line per skill `` `/{name}` — {description} ``, or `No skills are available to this agent.` — and answer without a model call: save `SessionHistoryContent::Request(request)` and then a `SessionHistoryContent::Response` carrying a `VizierResponseContent::Message { content, stats: None }` via `deps.storage.save_session_history`, then send that response on `response_tx` (pattern of the "Dream cycle started." reply ~line 180). Put the reply-and-save in one helper `answer_directly(...)` in `process.rs` so US3 reuses it. Confirm the saved response appears in the WebUI on reload (research R4).
- [ ] T016 [P] [US2] Add `GET /available` to `agent_skills()` in `src/channels/http/api/v1/agents/skills.rs` (route before `/{slug}`): same agent lookup and `user_can_view_agent` check as `list_agent_skills`; merge `SkillManager::new(workspace)` (global) and `SkillManager::for_agent` with agent winning by name, drop reserved names, sort by name, return `Vec<{ name, description }>` (new `AvailableSkill` struct, no content). Add it to the OpenAPI/utoipa registration if the other skill routes are registered there.
- [ ] T017 [US2] In `webui/app/routes/chat.tsx`: fetch `/api/v1/agents/{agentId}/skills/available` when the agent loads (use the same API helper other calls in this route use), and append entries `{ name: '/' + skill.name, description: skill.description, skill: true }` after `SLASH_COMMANDS` (~line 117) into `filteredCommands` (~line 372). Selecting a skill entry inserts `/<name> ` into the composer (not sent), unlike built-ins which send a `Command`; adjust both the click handler (~line 2300) and the keyboard Enter/Tab handler (~line 2336). Typing `/skills` sends as normal chat.
- [ ] T018 [US2] Telegram menu in `src/channels/telegram/mod.rs`: define Telegram `PlatformRules` (names 1–32 of `a-z0-9_`, `convert` = lower-case and `-`→`_`, description ≤256, max 100, built-ins `abort`, `checkpoint`, `lobotomy`, `session`, `skills`, plus `skill`); in `run()` (~line 55), before the `get_updates` loop, call `self.bot.set_my_commands(...)` with `commands::build(...)` mapped to `teloxide::types::BotCommand` (log failure, do not return an error) and hold `let _commands = commands::watch(...)` beside `_woken` with a `send` that calls `set_my_commands`. Ensure `/skills` typed in Telegram reaches the `Chat` arm (it is not intercepted by the existing `/session`/`/abort` handlers).

**Checkpoint**: `/skills` answers on every channel with no model call; the WebUI composer and both bot menus list the skills.

---

## Phase 5: User Story 3 — Clear feedback for bad invocations (P3)

**Goal**: an unknown command-shaped name gets a not-found reply with close names and no model call; non-commands stay chat.

**Independent test**: quickstart step 5.

- [ ] T019 [US3] In `src/agents/process.rs` `Chat` arm, on `Invoke{name,..}` that does not resolve: reply via `answer_directly` (T015) with `No skill named "{name}".`, then `Did you mean: a, b, c?` from `crate::sandbox::docs::did_you_mean(&name, &available_names)` (first 3) when non-empty, then `Send /skills to list them.` No model call, no usage row.
- [ ] T020 [US3] Unaddressed group messages: confirm `/skills` and `/<unknown>` stay `SilentRead` in Telegram groups (T014 only admits resolving names) and in Discord guilds without a mention, so no not-found reply is posted there (contract, "need the bot addressed").

**Checkpoint**: `/relase-notes` → not found with `release-notes` suggested; `/etc/hosts` → ordinary reply.

---

## Phase 6: Polish & cross-cutting

- [ ] T021 [P] Update help texts: Discord `/help` text in `src/channels/discord/mod.rs` (~line 374) and the Telegram help/`/start` reply if one exists in `src/channels/telegram/mod.rs`: add `/skills` and `/<skill> [input]`, `/skill <name> [input]`.
- [ ] T022 [P] Add an "Invoking a skill" section to `docs/src/configuration/skills.md`: grammar from contracts/invocation-surface.md, WebUI suggestions, Discord per-skill commands and `/skill` autocomplete (name rules, 100-command cap, 60 s re-sync), Telegram menu (`-` shown as `_`), Telegram privacy mode note, built-ins win.
- [ ] T023 [P] Add a short "User-invoked skills" note to `CLAUDE.md` under Agents (resolution in the `Chat` arm of `process.rs`, `## Invoked Skill` context section, `channels/commands.rs` registration and re-sync).
- [ ] T024 Run `cargo clippy` and `cargo test`; fix warnings in touched files.
- [ ] T025 Run `cd webui && npm run typecheck`.
- [ ] T026 Run quickstart.md steps 0–7 end to end against `just run` with a dummyplug agent (constitution e2e gate); run step 8 with real Discord and Telegram bot tokens, marked as such.

---

## Dependencies & execution order

- Phase 2 (T001–T004) blocks everything.
- US1: T005 → T006 → T007 → T008 (agent path); T009 → T010 → T011 → T012 (Discord, needs T008 to have an effect); T013 after T008; T014 after T004/T008.
- US2: T015 after T008; T016 independent of the agent path; T017 after T016; T018 after T009/T010.
- US3: T019 after T015; T020 after T014.
- Polish after the stories it documents.

### Story independence

- US1 alone is the MVP: invocation works on all channels; `/skills` and bad names fall back to ordinary chat.
- US2 needs US1's `Chat` arm dispatch (T008) and, for the Telegram menu, `commands.rs` (T009–T010).
- US3 needs US2's `answer_directly` helper (T015).

## Parallel opportunities

- T003 alongside T004 once T001–T002 exist.
- After T008: Discord (T009–T012) and Telegram (T014) in parallel; T016/T017 (HTTP + WebUI) in parallel with both.
- T021, T022, T023 in parallel.

## Implementation strategy

1. Phase 2, then US1 agent path (T005–T008); validate quickstart 1–3 in the WebUI with dummyplug. This is a shippable MVP for the WebUI and for DMs/mentions on both bots.
2. Discord slash commands (T009–T012) and Telegram unmentioned invocation (T014).
3. US2 (list, endpoint, composer, Telegram menu), then US3.
4. Polish and the full quickstart.
