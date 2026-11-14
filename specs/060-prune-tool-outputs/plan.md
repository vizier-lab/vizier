# Implementation Plan: Prune old tool outputs, recall them on demand

**Branch**: `060-prune-tool-outputs` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/060-prune-tool-outputs/spec.md`

## Summary

The model's view of a conversation is narrowed by one more pure step: after `messages_for_model`
narrows script reports, `prune_for_model` replaces old, large tool results with
`[output pruned (<size>) - use recall_history("<id>") to recall]`. The id is the stored history
uid of that result. To know it, `Message::ToolResult` gains a `record: Option<String>` field that
is never sent to a provider: replay fills it from `SessionHistory.uid`, and the live loop fills it
from the uid `StepRecorder` gets back when it saves the result. The rule walks the messages
once, in order, with a high mark of about 60k estimated tokens and a low mark of about 30k. A
decision at any point depends only on what came before it, so a result once pruned stays pruned,
and the live turn and the replay agree. A new `recall_history` tool, in the new `history`
category, reads a result back by uid from the calling conversation, uncapped.

## Technical Context

**Language/Version**: Rust 2024, the workspace toolchain

**Primary Dependencies**: no new crates. `serde_json` and the existing `tracing` fields only

**Storage**: none new. Recall reads `session_history` through the existing
`HistoryStorage::get_session_history_entry(session, uid)`, already scoped to agent and channel

**Testing**: unit tests in `schema/prune.rs` (the rule) and `agents/tools/history.rs` (the tool);
end to end: the dummyplug steps in [quickstart.md](quickstart.md), using a new dummyplug command
`pruned` (contract below)

**Target Platform**: every server and desktop target. Pure Rust, nothing platform-specific

**Project Type**: the engine crate `vizier-core` only. No HTTP, WebUI or binary change

**Performance Goals**: the prune pass is one linear walk per request over messages already in
memory, so it adds microseconds to a completion that takes seconds

**Constraints**: the model's view must be byte-identical below the high mark (SC-002). Pruning must
keep the prompt-cache prefix stable between turns (SC-004). Storage and the WebUI are untouched
(FR-001)

**Scale/Scope**: about 8 files changed, 2 new, 1 dummyplug protocol section

## Constitution Check

| Gate | Status |
|---|---|
| I. Lean by default | ✅ No crate, no config surface, no trait. One field on `Message::ToolResult`, rejected alternatives in [research.md](research.md) R1. Fixed constants, not settings (spec Assumptions) |
| II. Trait-based extensibility | ✅ The tool implements `VizierTool` and is registered on `default_toolset`. The category is a `ToolCategory` variant like the other 20. No branching on provider or tool kind: the rule reads only message shape and the report's `ok` / ending call |
| III. Self-contained default | ✅ Nothing external |
| IV. Portability | ✅ Nothing platform-specific |
| V. Errors & observability | ✅ The tool returns `status` results, never `Err` for a miss. The counts go on the `invoke_agent` span as `vizier.prune.*`. No `unwrap` outside tests |
| Design first (UI) | ✅ Not UI: no FigCraft pass. The WebUI shows stored history, which is unchanged |
| Dummyplug e2e | ✅ [quickstart.md](quickstart.md). This needs one additive dummyplug command (`pruned`) because the existing protocol cannot show what the model was sent. Existing quickstarts are unaffected (the protocol contract's rule) |
| Feature flags / roles | ✅ Not optional: core behaviour, no flag |
| #82 compat tracking | ✅ Nothing new of that kind: `record` is `#[serde(default)]` on a type that is not a stored format |

Post-design re-check: still passing. The design added nothing beyond the above.

## Project Structure

### Documentation (this feature)

```text
specs/060-prune-tool-outputs/
├── plan.md              # this file
├── research.md          # R1–R8 decisions
├── data-model.md        # record id, eligibility, marks, placeholder
├── quickstart.md        # dummyplug e2e steps
├── contracts/
│   ├── prune-rule.md        # the pruning rule, exactly
│   ├── recall-history.md    # the tool's input/output
│   └── dummyplug-pruned.md  # §7 of the dummyplug protocol
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── llm/types.rs                 # Message::ToolResult { call_id, content, record }; tool_result() sets None
├── llm/dialect/*.rs, llm/prepare.rs, llm/telemetry.rs
│                                # patterns gain `..`; record is never encoded
├── schema/
│   ├── history.rs               # history_entries_to_messages fills record = entry.uid
│   ├── prune.rs                 # NEW: prune_for_model + PruneStats + constants (pure, unit-tested)
│   └── mod.rs                   # re-export
├── agents/steps.rs              # StepRecorder::record returns the saved uid(s)
├── agents/agent/mod.rs          # tool results carry their record; completion_request prunes
│                                # (dream_prompt opts out); handover summarises the pruned view;
│                                # span fields vizier.prune.outputs / vizier.prune.tokens_removed
├── agents/agent/model/dummyplug.rs  # `pruned` command (§7)
└── agents/tools/
    ├── mod.rs                   # ToolCategory::History (name, description, ALL), table entry
    └── history.rs               # NEW: RecallHistory (VizierTool)

specs/008-dummyplug-provider/contracts/dummyplug-protocol.md   # §7 `pruned`
docs/src/…                       # one paragraph where tools/categories are documented
CLAUDE.md                        # one paragraph under Tools
```

**Structure Decision**: everything lives in `vizier-core`. The rule sits beside
`messages_for_model` in `schema/` because it is a model-view transform of the same kind, and
the tool sits with the other built-in tools.

## Complexity Tracking

None.
