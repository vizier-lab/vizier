# Implementation Plan: Reaction Awareness

**Branch**: `013-reaction-awareness` | **Date**: 2026-10-10 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/013-reaction-awareness/spec.md`

## Summary

Reactions people make on an agent's messages in the WebUI, Discord and Telegram are stored per message. The agent sees them as a bounded **digest** in its per-request context block, and they never start a turn. The core changes:

- **Storage**: reactions move out of the history row's JSON blob into a `message_reaction` table, where add and remove are distinct atomic statements.
- **Linking**: every reply carries the `history_uid` it was saved as, so the WebUI keys messages by the server's uid and the Discord and Telegram loops can record a `platform_message_link` for each message they post.
- **One ingest path**: `channels/reactions.rs::apply` resolves, validates, stores and broadcasts the change for all three channels. Each channel only translates its native event.
- **Memory**: the checkpoint handover includes the reactions, so feedback reaches the dream cycle and long-term memory.
- **Telegram tool**: the react tool uses `setMessageReaction` instead of posting a message.
- **Harness**: dummyplug gains `context` so all of this can be verified offline.

The design is in [research.md](./research.md) (Decisions 1 to 10), [data-model.md](./data-model.md) and [contracts/](./contracts/).

## Technical Context

**Language/Version**: Rust 2024 edition (stable). The WebUI is TypeScript 5 / React 19 / React Router v7.

**Primary Dependencies**: twilight-gateway/http/model 0.17.1 (Discord), teloxide 0.14 / teloxide-core 0.11.2 (Telegram), axum (WebSocket), rusqlite (bundled), rig-core. **No new dependencies**: every API needed is already in the pinned versions (research Baseline).

**Storage**: embedded SQLite. Two new tables, `message_reaction` and `platform_message_link`, plus a one-time idempotent migration of existing blob reactions. Both tables cascade from `session_history(uid)`.

**Testing**: `cargo test` covers unit tests for `reaction_digest` (selection, caps, ordering, name sanitising), the Telegram old/new diff, emoji normalisation, `ReactionStorage` against an in-memory DB (add/remove idempotence, FR-002, clear, the migration's idempotence), and dummyplug §6. `cd webui && npm run typecheck`. End-to-end: the dummyplug steps in [quickstart.md](./quickstart.md), sections 1 to 5 offline, plus the platform sections 6 and 7 with real bot tokens.

**Target Platform**: the single `vizier` binary (Linux, macOS and Windows; musl cross targets), plus the embedded WebUI.

**Project Type**: a single Rust binary with an embedded web frontend.

**Performance Goals**: a reaction write is O(1) statements. The old path read the whole session. Building the digest adds one indexed `IN (…)` query per history page and a pure in-memory pass over at most the loaded history.

**Constraints**: the per-request context block must stay in the user message so replayed history remains cacheable. The digest is bounded to ≤ 10 messages × bounded names (SC-005). Reactions never enqueue agent work (FR-008, SC-006).

**Scale/Scope**: about 15 Rust files touched, 4 of them new (`channels/reactions.rs`, `storage/reaction.rs`, `storage/sqlite/reaction.rs` and `system_prompt/reactions.rs`), and 2 removed (`channels/reaction_store.rs`, `webui/…/ReactionBar.tsx`). There are about 5 WebUI files.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Assessment |
|---|---|
| **I. Lean by Default** | ✅ No new crates. The new storage trait (`ReactionStorage`) follows the existing per-concern convention of `VizierStorageProvider`, the same way `BackgroundJobStorage` did. `apply` is a plain function, not a trait, because it has one implementation. Net code removed: `reaction_store.rs`, `update_history_reactions` (on 2 backends), `VizierRequestContent::Reaction` and its no-op handler, and `ReactionBar.tsx`. Dummyplug §6 is about 10 lines and is the only offline way to meet the e2e gate for FR-007. |
| **II. DRY via Trait-Based Extensibility** | ✅ Resolve, validate, store and broadcast live once, in `reactions::apply`. Channels translate native events, which is channel knowledge that belongs in each channel module. The digest is one pure function used by `chat`, `dream_chat` and the handover. `Platform` is a storage key (`as_str`), not something dispatch branches on. |
| **III. Self-Contained Runtime** | ✅ SQLite only. No new service. The WebUI stays embedded. |
| **IV. Portability** | ✅ No OS-specific code. No new native dependencies, so `Cross.toml` targets are unaffected. |
| **V. Unified Errors & Observability** | ✅ The storage layer keeps its existing `anyhow::Result` convention (as `storage/sqlite/*` does today). Tools return `VizierError`. Dropped events (unlinked target, bot reactor) log at `debug!`. Link-record and broadcast failures log at `warn!` and never fail a send or a turn. |
| **Gate: end-to-end via dummyplug** | ✅ quickstart.md scripts every agent-observable check as dummyplug steps. §6 is added to the protocol additively and the protocol doc is updated in the same change. |
| **Gate: manual verification for migrations** | ✅ The blob-to-table migration runs at startup, and quickstart §5 covers running it on a pre-feature database, twice. |

**Post-design re-check**: still passing. The one cross-cutting change is the `VizierResponse` `Default` derive plus `history_uid`. It's mechanical (`..Default::default()` at construction sites) and adds no behaviour branching.

## Project Structure

### Documentation (this feature)

```text
specs/013-reaction-awareness/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── agent-context.md       # the ## Reactions section + handover addition
│   ├── websocket.md           # WebUI frames: history_uid, reaction, reactions, reaction_error
│   ├── platforms.md           # Discord/Telegram linking, inbound events, telegram_react_message
│   └── dummyplug-context.md   # dummyplug §6 `context`
├── checklists/requirements.md
└── tasks.md                   # /speckit-tasks
```

### Source Code (repository root)

```text
src/
├── schema/
│   ├── response.rs            # VizierResponse: Default + history_uid
│   ├── request.rs             # ReactionEntry.user_name; remove ReactionEvent, Reaction variant
│   └── history.rs             # (no change to replay)
├── storage/
│   ├── reaction.rs            # NEW: ReactionStorage trait + VizierStorage forwarding
│   ├── history.rs             # save_session_history → Result<String>; drop update_history_reactions
│   ├── mod.rs                 # add ReactionStorage to VizierStorageProvider
│   ├── fs/history.rs          # drop update_history_reactions
│   └── sqlite/
│       ├── mod.rs             # init_reaction_schema (+ blob migration)
│       ├── reaction.rs        # NEW: impl ReactionStorage for SqliteStorage
│       └── history.rs         # fill SessionHistory.reactions on every read; return uid on save
├── transport.rs               # SessionFrame::Reactions
├── channels/
│   ├── reactions.rs           # NEW: ReactionChange + apply()
│   ├── reaction_store.rs      # DELETED
│   ├── mod.rs
│   ├── http/api/v1/agents/channel.rs   # reaction frame → apply; Reactions frame → socket; reaction_error
│   ├── discord/mod.rs         # reaction events; link sent replies
│   └── telegram/mod.rs        # allowed_updates; MessageReaction diff; link sent replies
├── utils/
│   ├── discord.rs             # send_message/send_file return message ids
│   └── telegram.rs            # send_message returns message ids
├── agents/
│   ├── process.rs             # drop Reaction arm; checkpoint passes digest to handover
│   ├── agent/
│   │   ├── mod.rs             # chat sets history_uid; handover takes digest
│   │   ├── model/dummyplug.rs # §6 `context`
│   │   └── system_prompt/
│   │       ├── context.rs     # context_md takes Option<digest>
│   │       └── reactions.rs   # NEW: reaction_digest()
│   └── tools/
│       ├── telegram/mod.rs    # set_message_reaction; link telegram_send_message
│       ├── discord/mod.rs     # link discord_send_message
│       └── webui/mod.rs       # (gets uid back; no link needed)

webui/app/
├── routes/chat.tsx            # key by history_uid; pending/ack/error; stable handler; frames
├── components/MessageItem.tsx # react control on agent msgs only; comparator includes onReact
├── components/ReactionBadges.tsx  # pending state
├── components/ReactionBar.tsx # DELETED (unused)
├── interfaces/types.ts        # history_uid, ReactionsFrame, ReactionErrorFrame
└── routes/agent-settings.tsx  # Telegram tooltip copy

specs/008-dummyplug-provider/contracts/dummyplug-protocol.md   # + §6
```

**Structure Decision**: this is the existing single-binary layout. The new code sits beside the concern it extends: storage traits in `storage/`, cross-channel logic in `channels/`, and prompt sections in `system_prompt/`. Nothing new at the top level.

## Implementation order

The order is chosen so each step is independently testable.

1. **Storage foundation**: `ReactionStorage`, the tables, the migration, `save_session_history` returning the uid, and reads filling `reactions`. Unit tests against an in-memory DB.
2. **`history_uid` on responses**: the `VizierResponse` change, and `chat` setting it.
3. **`reactions::apply` + WebUI server side**: the WebSocket handler uses `apply`, and the broadcast carries `Reactions` frames. Remove the `Reaction` request variant and `reaction_store.rs`.
4. **WebUI client** (Story 2). Quickstart §1, §3 and §4 pass.
5. **Digest + dummyplug §6 + handover** (Story 1). Quickstart §2 and §5 pass.
6. **Discord**: send helpers return ids, linking, inbound events (Story 3). Quickstart §6.
7. **Telegram**: the same, plus `telegram_react_message` and the tooltip (Stories 4 and 5). Quickstart §7.
8. **Docs**: the CLAUDE.md "Channels" note on reactions, and the dummyplug protocol §6.

## Complexity Tracking

No constitution violations to justify.
