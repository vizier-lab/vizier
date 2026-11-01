# Implementation Plan: Asking the Person — Structured Questions and Sensitive Input

**Branch**: `042-agent-asking-question` | **Date**: 2026-11-01 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/042-agent-asking-question/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Two built-in tools — one family, one module — that let an agent ask a person for something:

- **`ask_question`** asks 1–10 typed questions (text, choose one, choose several, confirm) and
  **ends the turn**. The answer returns as the person's next turn, a `question_answer` request
  carrying structured answers, so it takes the ordinary history and queue path. Presented as a
  custom WebUI card, and with Discord buttons/selects/modals and Telegram inline keyboards
  where those channels exist.
- **`ask_sensitive_input`** asks for one sensitive value and **does not end the turn**: it
  blocks inside the running script, the WebUI shows a masked dialog, and the value is armed
  into the turn's existing `SecretRedactor` on arrival, so the execution report, history entry,
  model copy, log line and telemetry are scrubbed exactly as for `get_secret`. WebUI only,
  because a channel stores what it carries.

The approach is deliberately all-existing-machinery: one new tool category, one new per-turn
availability predicate on the tool trait, two new tables merged into history at read time
exactly as `client_tool_call` is, one new request kind, three new `SessionFrame` variants, and
one new per-conversation surface registry so a channel post's failure can fail the call. No new
dependency, no new transport, no new store (research R1–R16).

## Technical Context

**Language/Version**: Rust 2021, workspace toolchain as pinned.

**Primary Dependencies**: none added. Reused: `twilight-model` + `twilight-http` +
`twilight-util::builder` (Discord controls; the gateway already subscribes
`INTERACTION_CREATE`), `teloxide` 0.17 (Telegram inline keyboards, `answer_callback_query`),
`schemars` (tool input schemas), `rusqlite` (two new tables), `tokio::sync::oneshot` (the
blocking answer path, as `ClientTools` already uses).

**Storage**: embedded SQLite; the schema is one `CREATE TABLE IF NOT EXISTS` batch. Two new
tables (`question_set`, `sensitive_ask`) and **no** migration or back-compat code, so nothing is
added to #82 (research R13).

**Testing**: `cargo test --workspace` (validation, the atomic claim, short-id resolution,
read-time merge, redaction through `execute_python`, the availability filter) and
`cargo clippy --workspace --all-targets -- --deny warnings`; `cd webui && npm run typecheck`,
`cd design && npm test`. End-to-end: **dummyplug**, scripted in [quickstart.md](quickstart.md) —
every check is a `{"tool": "execute_python", "arguments": {"code": …}}` message against a
running binary, per the constitution's e2e gate. The channel controls, the WebUI card and the
sensitive dialog are additionally listed there as manual steps with stated expected outcomes,
because they need a live bot and a person; they are never claimed as covered by the offline run.

**Target Platform**: the shipped single binary on Linux/macOS/Windows and the Docker image. No
platform-specific code: the channel work stays behind the existing `discord` and `telegram`
cargo features, and nothing new is `cfg`-gated in core.

**Project Type**: single Rust workspace (`vizier-core`, `vizier-http`, `vizier-derive`,
`vizier`) plus the `webui` and `design` submodules.

**Performance Goals**: an answer starts the next turn within one second on the same machine
(SC-009); a press is acknowledged inside the platform's deadline, so the acknowledgement is sent
before any storage read; the read-time `question_set` fill is one chunked `SELECT … IN (…)` per
history page — the same shape and cost as `fill_client_calls`.

**Constraints**: no new dependency; no extra bot permission or privileged intent (FR-031); the
sensitive value must never be at rest, logged, or returned to the model (FR-041, FR-044); a
channel that cannot show a secure input simply does not offer the tool; the blocking wait must
be strictly shorter than the script's own tool timeout, or the script sees a timeout error
instead of a branchable result (research R8).

**Scale/Scope**: 1 new core module (`questions/`), 1 new storage module, 2 tables, 1 new tool
category, 2 tools, 3 `SessionFrame` variants, 2 `SessionHistoryContent` variants, 1
`VizierRequestContent` variant, edits to 2 channel readers, 1 WebUI card + 1 dialog, and docs.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Assessment |
|---|---|
| **I. Lean by default** | No new crate, no new config flag, no speculative abstraction. The one new trait method (`VizierTool::available`) and the one new trait object (`QuestionSurface`) each have a real second caller on day one (two tools; two channels). ✅ |
| **II. DRY via trait-based extensibility** | No new `match` over a kind in shared code. Channels implement `QuestionSurface` and register it in their reader constructor; core never names Discord or Telegram. The shared rules (validation, the atomic claim, limits, the text form) live in one place and every surface calls them (research R7, R12). ✅ |
| **III. Self-contained runtime** | Embedded SQLite only; no external service. The Discord/Telegram paths are existing optional channel integrations: with none configured, the tool is offered only in the WebUI. ✅ |
| **IV. Portability** | No new OS-specific code and no new dependency, so every `Cross.toml` target keeps building. ✅ |
| **V. Unified errors & observability** | `crate::Result<T>`/`VizierError` throughout, `throw_vizier_error` for foreign errors, `tracing` only. A tool error reaches the script as a catchable `RuntimeError`, never a failed turn. ✅ |
| **Migrations tracked in #82** | None added: two new tables need no migration, and no existing field changes shape (research R13). ✅ |
| **Matching submodule branches** | `webui` and `design` are uninitialised in this worktree; implementation starts with `just checkout 042-agent-asking-question` so both carry the same branch (research R15). A task, not an assumption. ✅ |
| **Manual verification for runtime-affecting changes** | New tables means the binary is actually run against a pre-existing database in quickstart.md §1 (a restart with a set still pending), not just `cargo build`. ✅ |
| **E2E through a dummyplug agent** | [quickstart.md](quickstart.md) scripts the tool listing, a full question-set round trip, the redaction grep and the availability checks as dummyplug steps. ✅ |
| **Conventional commits** | `feat(questions): …` in the workspace and both submodules; hook-generated messages held to the same rule. ✅ |

**No violations.** Complexity Tracking stays empty.

## Project Structure

### Documentation (this feature)

```text
specs/042-agent-asking-question/
├── plan.md                 # This file
├── research.md             # Phase 0: R1–R16, each with its tree anchor
├── data-model.md           # Phase 1: entities, state machines, SQL, storage trait
├── quickstart.md           # Phase 1: dummyplug steps + manual channel/dialog checks
├── contracts/
│   ├── agent-tools.md      # Phase 1: both tools' input/output/description/limits
│   ├── questions-wire.md   # Phase 1: WebSocket frames, request kind, history entry
│   └── channel-controls.md # Phase 1: Discord/Telegram controls and their limits
├── checklists/
│   └── requirements.md
├── spec.md
└── tasks.md                # Phase 2 output (/speckit-tasks) — not created here
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── questions/                     # NEW — the whole feature's core
│   ├── mod.rs                     # registry: surfaces, per-turn state, submit, abandon
│   ├── model.rs                   # QuestionSet, Question, Option, Answer, records
│   ├── validate.rs                # every FR-005 / FR-012 rule: one input, many errors
│   ├── limits.rs                  # research R12 verbatim — one table
│   ├── tool_question.rs           # AskQuestion
│   ├── tool_sensitive.rs          # AskSensitiveInput
│   ├── surface.rs                 # QuestionSurface trait + the platform registry
│   ├── text_form.rs               # FR-027 parser/renderer for sets native controls cannot hold
│   └── sensitive.rs               # open-ask registry, wait, arm, outcome
├── agents/tools/mod.rs            # + ToolCategory::Questions, + available(), catalogue filter
├── agents/agent/mod.rs            # + ToolContext.interactive, per-turn question bookkeeping
├── agents/process.rs              # + abandon asks on abort/panic
├── agents/steps.rs                # reuse record_step / seal_open_calls
├── dependencies.rs                # + the questions registry, + a startup sweep
├── schema/history.rs              # + SessionHistoryContent::{QuestionSet, SensitiveAsk}
├── schema/request.rs              # + VizierRequestContent::QuestionAnswer
├── transport.rs                   # + SessionFrame::{QuestionSet, SensitiveAsk, SensitiveAskClosed}
├── channels/discord/mod.rs        # + surface impl, components, modal, interaction branch
├── channels/telegram/mod.rs       # + surface impl, inline keyboards, CallbackQuery branch
├── storage/question.rs            # NEW trait surface (data-model.md §10)
├── storage/sqlite/question.rs     # NEW impl + fill_question_sets
├── storage/sqlite/mod.rs          # + two schema functions
├── storage/sqlite/history.rs      # + content-type arm, + the fill call
└── storage/sqlite/session.rs      # + explicit deletes in delete_session

crates/vizier-http/src/
├── topic.rs                       # + 1 inbound frame, + 3 outbound mappings, ask replay on open
└── api/v1/agents/channel.rs       # + frame validators next to check_client_event

webui/  (submodule, same branch)
├── app/lib/frames.ts              # + question_set / sensitive_ask frames in the reducer
├── app/components/questions/      # NEW — QuestionCard, per-type fields, SensitiveAskDialog
└── app/stores/turns.ts            # + pending set/ask state

design/ (submodule, same branch)
├── src/components/…               # card and dialog primitives the WebUI composes
└── src/stories/…                  # the stories that make them reviewable

docs/src/
├── api-integration/client-tools.md # + the new frame kinds
├── configuration/channels.md       # + how questions look on each channel
└── configuration/questions.md      # NEW — the two tools in agent terms
```

**Structure Decision**: a core-side module plus one thin surface per channel, because the
platform-specific part is only *presentation* of one shared model. `questions/` owns the model,
validation, limits, the claim and the sensitive-ask lifecycle. The WebUI needs no server-side
surface at all — its presentation is the history entry plus the frame `record_step` already
publishes. Discord and Telegram implement `QuestionSurface` inside their existing reader modules
and register it in their constructor, the pattern `IntegrationKind` and the readers already use.

## Phase 0 — Outline & Research

Complete: [research.md](research.md) resolves every unknown with a tree anchor. The five that
carried the checklist's "flagged for planning" weight are R2 (what makes a turn interactive),
R3 (how a pending set survives a reload and a restart), R4 (how an ask reaches a channel and
reports failure), R8 (how a blocked script gets a value) and R9 (the ask lifecycle). No open
questions remain; each carries the alternative that was rejected and why.

## Phase 1 — Design & Contracts

Complete: [data-model.md](data-model.md) — entities, state machines, SQL, storage trait — and
three contracts.

- [contracts/agent-tools.md](contracts/agent-tools.md): the two tools as the agent sees them,
  every limit and every refusal name.
- [contracts/questions-wire.md](contracts/questions-wire.md): the WebSocket frames, the
  `question_answer` request, and the history entry that draws the card.
- [contracts/channel-controls.md](contracts/channel-controls.md): the Discord and Telegram
  control mapping, their limits, and the text-form fallback.

**Agent context**: the `CLAUDE.md` SPECKIT block deliberately stays generic; its own text says
not to point it at one feature, so it is not edited.

**Re-check of the Constitution Check after design**: unchanged — no dependency, no abstraction
without a second caller, no migration code. Two enum-growth facts worth stating plainly:
`SessionFrame` goes from 9 to 12 variants and `ToolCategory` from 17 to 18. Both are additions
to existing traits, not new branching.

## Complexity Tracking

> No Constitution Check violations, so nothing to justify.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| — | — | — |
