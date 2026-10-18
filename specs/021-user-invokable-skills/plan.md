# Implementation Plan: User-Invokable Skills

**Branch**: `021-user-invokable-skills` | **Date**: 2026-10-18 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/021-user-invokable-skills/spec.md`

## Summary

A person sends `/<skill> [input]` (or `/skill <name> [input]`, or `/skills` to list) as an ordinary chat message on the WebUI, Telegram or Discord. The three channels already deliver such a message to the agent as `VizierRequestContent::Chat(text)`, so **resolution happens once, server-side, in the agent's chat path** (`agents/process.rs`, `Chat` arm) rather than in each channel: a pure parser (`skill::invocation`) turns the text into `List`, `Invoke{name,input}` or `Plain`; `VizierSkills` resolves the name with the agent-over-global precedence it already uses; the turn then runs with the skill's full content in a new `## Invoked Skill` section of the per-request context block, and the model's user message is the input text. A list or a not-found reply is answered directly with no model call. Channels change only to (a) on Discord, register each skill as a slash command of that bot (kept in step with the skills by a 60 s check) whose interaction becomes the same `Chat("/<slug> <input>")`; on Telegram, let a group message that starts with the exact name of one of the agent's skills through as chat without a mention (via one shared helper) and treat `/name@bot …` as addressed, and (b) advertise the feature in help text; the WebUI composer gains skill entries in its existing `/` suggestions, fed by one new read endpoint that returns the skills the agent can actually use (agent and global).

## Technical Context

**Language/Version**: Rust (edition per `Cargo.toml`), TypeScript / React 19 (WebUI)

**Primary Dependencies**: none new — existing `SkillManager`, `VizierSkills`, `axum`, `twilight`, `teloxide`

**Storage**: none new. Skills stay markdown files via `SkillManager`; the invocation is recorded in the existing session history (`Request` entry, with `invoked_skill` in its metadata)

**Testing**: `cargo test` unit tests for the parser and resolver; `npm run typecheck`; end-to-end: dummyplug agent steps in [quickstart.md](quickstart.md) (the `context` command echoes the per-request context block, which is where the invoked skill lands)

**Target Platform**: unchanged (single binary, Linux/macOS/Windows, musl)

**Project Type**: single Rust binary with bundled WebUI

**Performance Goals**: a list or not-found reply makes no model call; resolution is a directory read of the skill folders, as every `all_skills()` call already is

**Constraints**: no change to system prompts or replayed history shape (the invoked skill lives only in the per-request context block in the user message, preserving provider cache prefix, per `context.rs` SC-013)

**Scale/Scope**: tens of skills per agent; one parser, one resolver, one context section, one endpoint, one composer change

## Constitution Check

*GATE: passes before Phase 0 and after Phase 1.*

| Principle | Assessment |
|---|---|
| I. Lean by default | No new crate, table, config flag or trait. One small pure module plus one context section. The "closest names" helper reuses the existing `did_you_mean` logic used for tool categories rather than adding a fuzzy-match crate. |
| II. Trait/shared, no per-kind branching | The decision is made once for all channels at the point every channel already converges (`Chat` in `process.rs`). Channels do not branch on skill names; each only translates its own native event (a Discord interaction, a Telegram addressing rule) into the same `Chat` text. |
| III. Self-contained runtime | No external service; skills are local files. |
| IV. Portability | No OS-specific code. |
| V. Unified errors & tracing | `crate::Result`, `tracing` only. |
| Dev gates | `cargo clippy` + `cargo test`, `npm run typecheck`, dummyplug e2e in quickstart.md. No tool added, so `every_tool_is_in_its_category` is untouched. |

No violations; Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/021-user-invokable-skills/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── invocation-surface.md
└── tasks.md             # /speckit-tasks, not created here
```

### Source Code (repository root)

```text
src/
├── skill/
│   └── invocation.rs            # NEW: pure parser (Plain | List | Invoke) + names_skill, unit-tested
├── agents/
│   ├── skill/mod.rs             # + VizierSkills::resolve(name), ::available(); closest-name helper
│   ├── process.rs               # Chat arm: dispatch parse result; direct replies (list / not found)
│   └── agent/
│       ├── mod.rs               # chat/chat_turn take `invoked: Option<Skill>`; message built from the input
│       └── system_prompt/context.rs   # context_md renders "## Invoked Skill"
├── channels/
│   ├── http/api/v1/agents/skills.rs   # + GET /agents/{id}/skills/available
│   ├── telegram/mod.rs          # unmentioned `/skill-name` allowed; `/name@bot …` addressed; help text
│   └── discord/mod.rs           # per-skill slash commands + periodic re-sync; interaction → Chat; /help text
webui/app/routes/chat.tsx        # `/` suggestions include the agent's skills
docs/src/configuration/skills.md # "Invoking a skill" section for each channel
CLAUDE.md                        # Agents / Skills note
```

**Structure Decision**: single project, server-side resolution at the shared `Chat` entry. The WebUI sends the typed text unchanged (it has no `Command:` message for skills), so the three channels stay equivalent by construction.

## Complexity Tracking

None.
