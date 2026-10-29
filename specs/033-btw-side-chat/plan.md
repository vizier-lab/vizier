# Implementation Plan: /btw Ephemeral Side-Chat

**Branch**: `033-btw-side-chat` | **Date**: 2026-10-28 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/033-btw-side-chat/spec.md`; agreed design in [vizier-lab/vizier#108](https://github.com/vizier-lab/vizier/issues/108)

## Summary

`/btw <question>` lets a person ask the agent they are talking to an off-the-record question from the WebUI, Discord or Telegram. A side chat is **one aside with follow-ups** (up to 32,000 characters), discarded in full when it ends (research R0). Each answer is one tool-less completion, made by the agent as itself with the conversation so far and the aside's earlier turns as context. Nothing about the aside is kept except its token usage.

Technically: a new `VizierRequestContent::Btw(SideChat)`, always `transient`, is sent on the host conversation's session through the existing transport by one shared helper, `channels::side_chat::ask`. That helper validates, rate-limits (5 / 60 s per person) and awaits the reply under `prompt_timeout`. `agent_process` routes the request to `run_btw` beside `checkpoint`/`lobotomy`, and skips the session-detail refresh and the thinking flag for it. `run_btw` runs `VizierAgent::side_chat`, which assembles the same prompt prefix as a host turn and makes one `complete_until` call with `tool_choice: none`. It never touches `chat()`, history, the turn journal or session events.

Each channel presents the exchange differently. The WebUI shows a side panel whose turns live in component state. On Discord the side chat is public and stays in the channel. On Telegram it's visible until it deletes itself, and a content-free deletion ledger survives restarts. On both, a follow-up is a reply to one of the side chat's answers, and the two readers share one in-memory exchange store and one reply classifier, so replies to an aside never reach the agent as ordinary messages.

## Technical Context

**Language/Version**: Rust (edition 2024, the workspace toolchain); TypeScript / React 19 in the `webui/` submodule

**Primary Dependencies**: existing only: axum + utoipa (HTTP), twilight (Discord: slash-command interactions and messages), teloxide (Telegram), flume (transport), tokio, `nanoid`. WebUI: `@vizier/design` (`SlideOver`, `Composer`, `AgentReply`), Zustand untouched. **No new crates or npm packages.**

**Storage**: none new. No table or column. One content-free key per Telegram chat in the existing `state` table (deletion ledger, research R13). One `usage_event` row per answered side chat, through the existing path.

**Testing**: `cargo test` (unit: `side_chat` validation/limits/sweep/ledger, `is_side_chat`/`is_durable`, dispatch never reaching `handle_request`); `cargo clippy`; `cd webui && npm run typecheck` + vitest for the panel's pure exchange reducer. End-to-end: dummyplug agent steps in [quickstart.md](quickstart.md) §1–9, per the constitution's e2e gate. Discord/Telegram steps are **LIVE**.

**Target Platform**: the single vizier binary (Linux/macOS/Windows, musl cross builds) and the embedded WebUI

**Project Type**: single Rust binary with an embedded web frontend (submodule)

**Performance Goals**: a side chat in an idle conversation answers within one completion's latency (SC-001's 10 s for a typical question). The request prefix is byte-identical to the host conversation's, so prompt caching applies (research R5).

**Constraints**: zero durable trace of content (FR-006); no tool execution path reachable (FR-004); bounded by `prompt_timeout`; at most 32,000 characters per side chat, 4,000 per question; 5 asks / 60 s per person; Telegram removal within 2 min 15 s of last activity; Discord asides forgotten after 5 min idle (messages stay).

**Scale/Scope**: about 1 new Rust module (`channels/side_chat.rs`) and 1 agent file (`agents/agent/side_chat.rs`), edits to `schema/request.rs`, `agents/process.rs`, `agents/agent/mod.rs` (helper extraction), `channels/http/api/v1/agents/mod.rs`, `channels/discord/mod.rs`, `channels/telegram/mod.rs`, `dependencies.rs`. In the WebUI, 1 new panel component, `Composer.tsx` and the API types. Plus docs.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Assessment | Status |
|------------------|------------|--------|
| **I. Lean by Default** | No new dependency, table, config key or trait. Limits are constants (R8). One shared helper serves three channels. Reuses `RateLimiter`, `completion_request`, `prepare_system_prompts`, `history_entries_to_messages`, `with_context`/`context_md`, `UsageScope::turn` and design components. The only new persisted value is the Telegram ledger, needed to keep SC-004 across restarts (R13). | ✅ |
| **II. DRY via trait-based extensibility** | The new `Btw` arm in `start_session_job` follows the existing content-kind dispatch (`checkpoint`/`lobotomy`); `VizierRequestContent` is not a trait family, so this is not type-tag branching over trait variants. Logic shared by the channels (validate, rate-limit, send, await, exchange store, lifetime sweep) is in one module, not copied per channel. `agent_chat`'s send-and-await loop moves into `side_chat::ask` rather than being duplicated (R10). The handover system message is extracted into one helper used by `chat_turn` and `side_chat` (R6). One `is_side_chat()` predicate, not scattered `matches!`. | ✅ |
| **III. Self-contained runtime** | No external service. State is in memory or in the embedded SQLite `state` table. The WebUI panel is embedded with the rest of the build. | ✅ |
| **IV. Portability** | No OS-specific code. Timers use tokio. No new system libraries, so `Cross.toml` targets are unaffected. | ✅ |
| **V. Unified errors & observability** | `crate::Result` throughout; `SideChatError` is a channel-side outcome enum mapped to responses, not a module error type leaking into library code. `tracing` only, never logging question/answer text. Spans follow the existing content-capture opt-in (R15). | ✅ |
| **Distribution: dependency weight** | None added. | ✅ |
| **Workflow: clippy/test/typecheck** | Planned (quickstart §12). | ✅ |
| **Workflow: e2e through dummyplug** | Agent-observable change (agent loop, HTTP, channels). Quickstart §1–9 are dummyplug steps with expected replies and stored state. §10–11 are marked LIVE because they need platform bots. No change to the dummyplug protocol. | ✅ |
| **Workflow: submodule order** | WebUI change lands in `vizier-lab/webui` first. No `design/` change is expected; if one becomes needed, it lands in `design/` before `webui/`, then the pins are bumped in vizier. | ✅ |
| **Conventions: #82 tracking** | No migration, legacy shim or alias is added, so there is no #82 entry. | ✅ |

**Deviations from the agreed issue** (not constitution violations; recorded so review sees them):

1. The rate limiter is on `VizierDependencies`, not `HTTPState`, because the Discord and Telegram readers can't reach `HTTPState` (R9).
2. The rate-limit key is per platform identity, because platform links in profiles are unverified. This narrows the spec's cross-platform assumption (R9).
3. On Discord the side chat is public, not an ephemeral reply, and is continued by replying, as on Telegram. Ephemeral replies can't be replied to, and the private alternative (a button, a form and token-bound deletion) was judged too heavy in review on #109 (R12).
4. Telegram keeps a content-free deletion ledger in `state`. Without it, a restart would leave exchanges in the chat indefinitely (R13).
5. The request carries the earlier turns (`SideChat { question, turns }`) instead of `Btw(String)` (R11).
6. Two more per-request writes (session-detail titling, the `is_thinking` flag) must be skipped. The issue did not list them (R2).

## Project Structure

### Documentation (this feature)

```text
specs/033-btw-side-chat/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R0–R16
├── data-model.md        # Phase 1: SideChat, SideChats, ledger, unchanged entities
├── quickstart.md        # Phase 1: dummyplug e2e script (+ LIVE Discord/Telegram)
├── contracts/
│   ├── http-btw.md          # POST /api/v1/agents/{agent_id}/btw
│   ├── agent-side-chat.md   # Btw in the agent loop; prompt assembly; invariants
│   └── channel-commands.md  # What a person sees on WebUI, Discord, Telegram
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
src/
├── schema/request.rs                 # + VizierRequestContent::Btw(SideChat), SideChat, SideChatTurn,
│                                     #   kind() "btw", Display, is_side_chat()
├── dependencies.rs                   # + side_chat_limits: Arc<RateLimiter> (5 / 60 s)
├── agents/
│   ├── process.rs                    # gate refresh_session_detail + thinking state on is_side_chat();
│   │                                 # Btw(_) => run_btw in start_session_job; defensive arm in handle_request
│   └── agent/
│       ├── mod.rs                    # extract checkpoint_context(handover) from chat_turn; mod side_chat
│       ├── side_chat.rs              # NEW: impl VizierAgent { side_chat(..) } — one tool-less completion
│       └── system_prompt/side_chat.rs # NEW: SIDE_CHAT_MD, SIDE_CHAT_NO_TOOLS
├── channels/
│   ├── side_chat.rs                  # NEW: ask(), SideChatError, PersonKey, limits, SideChats store
│   │                                 #      (LiveExchange, Lifetime, sweep), unit tests
│   ├── http/api/v1/agents/mod.rs     # + POST /{agent_id}/btw (BtwRequest/BtwResponse, utoipa);
│   │                                 #   agent_chat's await loop reused from side_chat::ask
│   ├── discord/mod.rs                # /btw builtin + message option; public deferred reply with the
│   │                                 # marker line; replies to aside messages recognised before
│   │                                 # abort/Chat/SilentRead (classify_reply); /help text
│   └── telegram/mod.rs               # /btw + reply-to-answer recognised first; notice; sweep task with
│                                     # deleteMessage; state-storage ledger load/merge/clear; menu entry
docs/src/
├── configuration/channels.md         # /btw on Discord and Telegram (privacy model, timings, admin right)
├── api-integration/rest-api.md       # POST /agents/{id}/btw
└── configuration/observability.md    # side chats and content capture (R15)

webui/ (submodule vizier-lab/webui)
├── app/screens/chat/Composer.tsx     # BUILTIN += btw; intercept /btw in send(); onSideChat prop
├── app/screens/chat/SideChatPanel.tsx  # NEW: SlideOver panel, local exchange state, ask/retry/limits
├── app/lib/sideChat.ts (+ .test.ts)  # NEW: pure exchange reducer + error mapping
├── app/routes/… (chat screen)        # mount the panel, pass agent/channel/topic
└── app/api/schema.d.ts               # regenerated (BtwRequest/BtwResponse)
```

**Structure Decision**: single binary, existing layout. The shared side-chat logic lives in `src/channels/side_chat.rs` beside `reactions.rs`/`woken.rs`, which already hold logic shared by several channels. The model call lives on `VizierAgent` (it needs the model, tools snapshot and prompt helpers), in its own file so `agent/mod.rs` doesn't grow. WebUI work follows the submodule order (webui PR first, then the pin bump here).

## Phase 0 / Phase 1 outputs

- [research.md](research.md): every open question from the issue is settled (R7 usage, R6 context, R10 route, R12 Discord entry, R13 Telegram timeout, R16 tools), and the deviations are recorded. No `NEEDS CLARIFICATION` remains.
- [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md).
- Agent context: the `CLAUDE.md` SPECKIT block points to this plan.

## Constitution Check (post-design)

Re-checked against the Phase 1 artifacts. Nothing new: the contracts add no trait, dependency or config surface. The ledger is the only persisted addition, it carries no content, and it is justified in R13. Every shared behaviour sits in `channels/side_chat.rs` or on `VizierAgent`. **PASS.**

## Risks to watch in implementation

- **Missing one trace path.** The invariants in `contracts/agent-side-chat.md` (history, session detail, `session_turn` unchanged; no `history_uid`) should be pinned by a test that runs `run_btw` against SQLite storage. Code review alone won't catch a future write added to `agent_process`'s per-request path.
- **Queue wait.** A side chat in a busy conversation waits for the running turn (R4). If people report it, move side chats out of the session queue into their own `JoinSet`.
- **Provider quirks with `tool_choice: none`.** Some providers still emit tool calls. They are dropped (R5), so the worst case is the fixed no-tools notice rather than an action.
- **Replies slipping through.** On Discord and Telegram, a reply to an aside that isn't recognised becomes an ordinary message and is saved. Recognition must run before every other branch of each reader's message handler, including the text `/abort` shortcut. It must cover the live aside, the marker (Discord) or ledger (Telegram) after it ends, and other people's replies.
