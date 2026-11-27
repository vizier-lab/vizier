# Implementation Plan: Platform Account Linking

**Branch**: `069-platform-account-linking` | **Date**: 2026-11-27 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/069-platform-account-linking/spec.md`

## Summary

Replace the hand-typed Discord/Telegram ids on a person's profile with a proven link.

- **Connecting**: `/connect` on either bot issues a one-time code, sent privately. It comes with a link `{base_url}/connect#CODE` when `base_url` is set. A signed-in person previews the code, confirms, and the link is stored in a new `platform_link` table.
- **What agents see**: every platform request is stamped by one core resolver, `platform_links::sender::resolve`. A linked account reads as its Vizier username with `verified: true`. Anyone else reads as `Unregistered User` with `verified: false`. Both carry the platform id and names in `metadata`.
- **Owner**: OWNER.md lists only verified links.
- **Old values**: hand-typed values become read-only "not verified" data (#82).
- **Unlinking**: only from the profile's row menu.

See [research.md](research.md) for the decisions (R1–R13).

## Technical Context

**Language/Version**: Rust (workspace edition and toolchain unchanged); TypeScript + React 19 for the WebUI
**Primary Dependencies**: existing only: `ring` (sha256, random), twilight, teloxide, axum, rusqlite/PostgreSQL through `SqlStore`; `@vizier/design` in the WebUI. No new crate.
**Storage**: three new tables, `platform_link`, `platform_connect_code` and `platform_link_event`. SQLite `init_platform_link_schema` and PostgreSQL `0018_platform_links.sql`. `user_profile` is unchanged.
**Testing**: `cargo test --workspace`, with core unit tests for codes, resolve, frontmatter, `owner_md` and the storage contract, plus `vizier-http` route tests. WebUI: vitest for `lib/connect.ts`. End to end: a dummyplug agent with the new §9 `sender` echo ([quickstart.md](quickstart.md)), plus a live Discord and Telegram pass marked live.
**Target Platform**: server (Linux/macOS/Docker) and the desktop app (macOS/Linux/Windows)
**Project Type**: web service plus embedded WebUI, and desktop host
**Performance Goals**: one indexed lookup per platform message; no added latency a person notices
**Constraints**: a code is never logged, stored in plaintext or sent in a URL the server sees; a link or unlink applies on the next message with no cache; works without `base_url`
**Scale/Scope**: a few links per person; codes live 10 minutes

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Status | Notes |
|---|---|---|
| I. Lean by Default | ✅ | No new crate. One new storage trait, because a new concern needs its own home. No cache, no config flag: `base_url` already exists. |
| II. DRY via traits | ✅ | One resolver replaces six hand-built `format!` labels across both readers. Issuing and wording are shared (`platform_links::connect`, `text.rs`); readers only send. Storage is one `SqlStorage` impl over the dialect seam. No `match` on platform outside the readers' own native calls. |
| III. Self-contained default | ✅ | SQLite tables. No external service. The code path works with no public address. |
| IV. Portability | ✅ | No OS-specific code. |
| V. Errors & observability | ✅ | `crate::Result`, `ServiceError` kinds. `tracing` logs name the platform and action, never a code. |
| Feature flags | ✅ | `platform_links` is core and unflagged (storage, routes, resolver). The `/connect` handlers sit inside the already-flagged `channel-discord` / `channel-telegram` readers. No API shape changes by flag. |
| Roles independent | ✅ | Codes live in storage, so the channels role issues and gateway-http redeems with no direct link between them. |
| E2E through dummyplug | ✅ | Planned: dummyplug §9 `sender` (submodule, matching branch) and `tests/dummyplug_e2e.rs` cases. |
| Design before UI code | ⏳ | Required before WebUI work: the Connected accounts card (with its row menu, code field and not-verified rows) and the `/connect` confirmation page. FigCraft is reachable now, so draw on `Vizier shell.figcraft` (settings pages) first. This is the first implementation task. |
| Migration code tracked (#82) | ✅ | Legacy profile fields, the `422 connect_required` refusal and the clear-on-link step get #82 entries. |

Post-design re-check: unchanged, all gates pass. The design gate is satisfied by the first task, before any `webui/` change.

## Project Structure

### Documentation (this feature)

```text
specs/069-platform-account-linking/
├── plan.md              # this file
├── research.md          # R1–R13
├── data-model.md        # tables, types, rules
├── quickstart.md        # dummyplug + live checks
├── contracts/
│   ├── http-connections.md    # /auth/users/me/connections routes
│   ├── bot-connect.md         # /connect replies on Discord and Telegram
│   └── agent-sender.md        # what the agent reads (frontmatter, OWNER.md)
└── checklists/requirements.md
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── platform_links/          # NEW
│   ├── mod.rs               # PlatformPerson, PlatformSender, UNREGISTERED_USER
│   ├── code.rs              # generate, normalise, hash (pure, tested)
│   ├── connect.rs           # issue(deps, person) -> ConnectReply
│   ├── sender.rs            # resolve(deps, person) -> PlatformSender; stamp(request, sender)
│   └── text.rs              # every reply the bots send
├── storage/platform_link.rs # NEW trait PlatformLinkStorage
├── storage/sqlite/platform_link.rs   # NEW impl + init_platform_link_schema
├── storage/sql/postgres/migrations/0018_platform_links.sql  # NEW
├── storage/sqlite/user.rs   # delete_user also deletes links/events
├── service/platform_links.rs # NEW: list, preview, redeem, unlink
├── service/profile.rs       # refuse new hand-typed values (422 connect_required), allow clearing
├── schema/request.rs        # VizierRequest.verified; frontmatter emits it
├── agents/agent/system_prompt/user.rs  # owner_md from links
├── agents/agent/mod.rs      # load owner links with the profile
├── templates/BOOT.md        # one sentence on verified / Unregistered User
├── channels/side_chat.rs    # key limits on platform id
├── channels/discord/{mod.rs,questions_handlers.rs}   # /connect (ephemeral); resolve everywhere
├── channels/telegram/{mod.rs,questions_handlers.rs}  # /connect (DM only); resolve; from.id fix
└── dependencies.rs          # connect_issues limiter; retention purge of codes

crates/vizier-http/src/
├── api/v1/auth.rs (or api/v1/connections.rs)  # routes + OpenAPI
├── state.rs, lib.rs         # connect_attempts_user / _addr limiters
└── tests/platform_links.rs  # NEW route tests (+ conformance vectors)

design/src/…                 # ConnectedAccountRow (with row menu), story
webui/app/
├── routes/connect.tsx       # NEW, outside RequireAuth (fragment → sessionStorage → login → confirm)
├── routes/settings-profile.tsx  # Connected accounts card replaces the four fields
├── screens/settings/ConnectConfirm.tsx  # shared by /connect and the profile code field
├── lib/connect.ts (+ test)  # code normalisation, stash/read
└── api/endpoints/connections.ts

dummyplug/                   # submodule: §9 `sender` echo (matching branch)
docs/src/configuration/channel-identities.md  # NEW docs page (+ SUMMARY.md)
```

**Structure Decision**: a new core module `platform_links` owns the concept. Readers, the HTTP layer and the system prompt each call into it, following the existing `consultations/`, `secrets/` and `notifications/` shape.

## Complexity Tracking

None. No constitution violations to justify.
