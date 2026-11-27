# Tasks: Platform Account Linking

**Input**: Design documents from `/specs/069-platform-account-linking/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R13), data-model.md, contracts/ (http-connections.md, bot-connect.md, agent-sender.md), quickstart.md

**Tests**: included. Every change here is agent-observable or security-relevant, so the constitution's e2e gate requires the dummyplug checks. The storage and HTTP rules (single use, uniqueness, rate limits, no leaks) are cheapest to pin in tests.

**Organization**: by user story. The tables, the storage trait, the code helpers and the sender resolver are Foundational, because US1 writes links and US2 reads them. Each story can still be checked alone: US2 can be tested by writing a link row directly.

**Paths**: engine code in `crates/vizier-core/`, routes in `crates/vizier-http/`, components in `design/`, screens in `webui/`, and the test harness in the `dummyplug/` submodule.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1–US4 from spec.md

---

## Phase 1: Setup

**Purpose**: matching branches and the design pass the constitution requires before UI code.

- [ ] T001 Create the matching `069-platform-account-linking` branch in every submodule with `just checkout 069-platform-account-linking`, and confirm with `just status`
- [ ] T002 [P] Draw the **Connected accounts** card for Settings › Profile in `C:\Users\ganes\OneDrive\Dokumen\Vizier shell.figcraft`, matching the existing settings pages in both themes. It needs:
  - a linked row: platform icon, handle, id, connected date, a ⋯ menu with Unlink
  - an Unlink confirmation dialog
  - a not-linked row with the "Send /connect to any agent's bot on Discord" hint
  - a "Not verified — send /connect to confirm" row with a remove action
  - the code field
  - the recent-changes list

  Copy the file to `C:\Users\ganes\AppData\Local\Temp\opencode` first. If FigCraft is unreachable, use an HTML mockup in `design/explorations/069-connected-accounts.html` instead.
- [ ] T003 [P] Draw the **`/connect` confirmation** in the same file: the platform account, the Vizier username, the "only confirm a code you requested yourself" warning, the "replaces your current Discord account" variant, and the expired/used and already-linked-elsewhere states. Get review before Phase 3's WebUI tasks start.

---

## Phase 2: Foundational (blocks every story)

**Purpose**: tables, storage, codes, the resolver and the request field.

- [ ] T004 Add the `PlatformLinkStorage` trait, `PlatformLink`, `PlatformLinkEvent` and `RedeemOutcome` (data-model.md § Storage trait) in `crates/vizier-core/src/storage/platform_link.rs`. Add it to the `VizierStorageProvider` supertrait in `crates/vizier-core/src/storage/mod.rs`
- [ ] T005 Add `init_platform_link_schema` in `crates/vizier-core/src/storage/sqlite/platform_link.rs`, called from `crates/vizier-core/src/storage/sqlite/mod.rs`. It creates `platform_link` (PK `(platform, platform_user_id)`, `UNIQUE (user_id, platform)`, FK `ON DELETE CASCADE`), `platform_connect_code` (PK `(platform, platform_user_id)`, `UNIQUE code_hash`) and `platform_link_event`
- [ ] T006 [P] Add PostgreSQL migration `crates/vizier-core/src/storage/sql/postgres/migrations/0018_platform_links.sql` with the same three tables, and register it where the other migrations are listed
- [ ] T007 Implement `PlatformLinkStorage` for `SqlStorage` once over `SqlConn`/`Dialect` in `crates/vizier-core/src/storage/sqlite/platform_link.rs`. Writes use `time::stored` for text times. `redeem_connect_code` does one transaction: take the code row by hash and expiry, refuse `LinkedElsewhere`, delete the caller's old link for that platform (`Replaced`), insert the link, clear that platform's legacy pair in `user_profile.data`, and write the event
- [ ] T008 Extend `delete_user` in `crates/vizier-core/src/storage/sqlite/user.rs` to delete the person's `platform_link` and `platform_link_event` rows in its existing transaction (FR-013)
- [ ] T009 [P] Create `crates/vizier-core/src/platform_links/mod.rs` with `PlatformPerson`, `PlatformSender`, `UNREGISTERED_USER`, and `metadata_fields(&PlatformPerson) -> serde_json::Map`. Register `pub mod platform_links;` in `crates/vizier-core/src/lib.rs`
- [ ] T010 [P] Add the pure helpers in `crates/vizier-core/src/platform_links/code.rs`: `generate()` (8 symbols from `23456789ABCDEFGHJKMNPQRSTUVWXYZ` using `ring::rand`), `display(code)` → `XXXX-XXXX`, `normalize(input)` (upper-case, strip `-`/whitespace, reject other characters and wrong lengths) and `hash(normalized)` → hex sha256
- [ ] T011 Add `resolve(deps, PlatformPerson) -> PlatformSender` in `crates/vizier-core/src/platform_links/sender.rs`. It does one `find_link`, then the Vizier username. It calls `touch_link_names` only when the handle or display name changed. On error it falls back to unregistered with a `warn!` that holds no code or name. It also adds `stamp(&mut VizierRequest, &PlatformSender)`, which sets `user`, `verified` and merges `metadata_fields` into `metadata`
- [ ] T012 Add `verified: Option<bool>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`) to `VizierRequest` in `crates/vizier-core/src/schema/request.rs`. In `generate_frontmatter`, insert `verified` right after `sender` in every arm that renders `sender: self.user` (default, mid_turn, question_answer, client_event), and only when `Some`
- [ ] T013 Update `public-api.txt` in `crates/vizier-core/public-api.txt` for the new public items

### Foundational tests

- [ ] T014 [P] Unit tests in `crates/vizier-core/src/platform_links/code.rs`: alphabet and length, `display`/`normalize` round trip, `abcd-efgh ` normalises, `ABCD0FGH` is rejected, and the hash is stable
- [ ] T015 [P] Storage contract tests in `crates/vizier-core/src/storage/sqlite/platform_link_tests.rs`, one per rule:
  - a second issue voids the first
  - an expired code is `Unavailable`
  - a second redeem is `Unavailable`
  - an account linked elsewhere is `LinkedElsewhere`, and the other link is untouched
  - a second account on the same platform is `Replaced`
  - redeem clears legacy profile values for that platform only
  - `delete_user` removes links and events
  - `purge_expired_connect_codes`
- [ ] T016 [P] Frontmatter tests in `crates/vizier-core/src/schema/request.rs` (tests module): `verified: true/false` follows `sender` on a chat request; a request with `verified: None` and every non-platform arm (scheduler, background, subagent, consultation, wake) is byte-identical to before; an old JSON row without `verified` deserialises and renders unchanged

**Checkpoint**: links can be written and read, and requests can carry them.

---

## Phase 3: User Story 1: Connect with `/connect` (P1) 🎯 MVP

**Goal**: `/connect` on either bot gives a private code (plus a link when `base_url` is set). A signed-in person previews it, confirms, and the account is linked.

**Independent Test**: send `/connect`, redeem the code as a signed-in person, and see the link in `GET /auth/users/me/connections` with the platform name and id the bot saw (quickstart §1 steps 2–4, §2 steps 1–3).

### Core

- [ ] T017 [P] [US1] Put every bot reply from contracts/bot-connect.md in `crates/vizier-core/src/platform_links/text.rs`: code with link, code without link, ask in private (with `t.me` link), and too many. The platform name is a parameter
- [ ] T018 [US1] Add `issue(deps, &PlatformPerson, private: bool) -> ConnectReply` in `crates/vizier-core/src/platform_links/connect.rs`:
  - a non-private chat returns `AskInPrivate` and issues nothing
  - it checks `deps.connect_issues` (5 per 10 minutes per `"{platform}:{id}"`)
  - it generates a code, stores its hash with a 10-minute expiry, and builds `{base_url}/connect#{display}` from `deps.config.base_url` when set
  - it logs `info!(platform, "connect code issued")` only
- [ ] T019 [US1] Add the `connect_issues` `RateLimiter` to `VizierDependencies` in `crates/vizier-core/src/dependencies.rs`, and call `purge_expired_connect_codes` from the scheduler's daily retention tick next to the notification purge in `crates/vizier-core/src/scheduler/mod.rs`
- [ ] T020 [US1] Add `service::platform_links` in `crates/vizier-core/src/service/platform_links.rs`: `list(principal)` (links, legacy `unverified`, last 20 events), `preview(principal, code)` and `redeem(principal, code)`, returning `ServiceError` kinds that map to `404 connect_code_unavailable`, `409 platform_linked_elsewhere` and `422` for a malformed code (same message as unavailable). Register it in `crates/vizier-core/src/service/mod.rs`

### Channels

- [ ] T021 [P] [US1] Discord: add `CommandEntry::new("connect", "link this account to your Vizier user")` to `builtin_commands` in `crates/vizier-core/src/channels/discord/mod.rs`. Handle `"connect"` in the interaction match beside `"abort"`/`"btw"`: build `PlatformPerson` from the interaction author, call `platform_links::connect::issue(…, private: true)`, and answer with an `EPHEMERAL` interaction response. Never forward it to the agent
- [ ] T022 [P] [US1] Telegram: add `connect` to `telegram_rules().builtins` in `crates/vizier-core/src/channels/telegram/mod.rs`. Handle `is_command(&text, "connect", &bot_username)` before the chat path with `private = msg.chat.is_private()` and a `PlatformPerson` from `msg.from`. Reply in the chat with the `ConnectReply` text. Never forward it to the agent
- [ ] T023 [US1] Update the built-in name lists in the command tests (`crates/vizier-core/src/channels/commands.rs`, Discord/Telegram command tests) so `connect` is listed and a skill named `connect` is skipped

### HTTP

- [ ] T024 [US1] Add the `connect_attempts_user` and `connect_attempts_addr` limiters (10 per 10 minutes) to `HTTPState` in `crates/vizier-http/src/state.rs` and build them in `crates/vizier-http/src/lib.rs`
- [ ] T025 [US1] Add routes `GET /auth/users/me/connections`, `POST /auth/users/me/connections/preview` and `POST /auth/users/me/connections` in `crates/vizier-http/src/api/v1/connections.rs`, with OpenAPI (`utoipa`) and the request/response types from contracts/http-connections.md. Mount them in `crates/vizier-http/src/api/v1/mod.rs`. Preview and redeem take both limiters before calling the service and return `429` with `Retry-After`. No handler logs the body
- [ ] T026 [US1] Regenerate `webui/app/api/schema.d.ts` with `node scripts/openapi.mjs`

### HTTP tests

- [ ] T027 [P] [US1] Route tests in `crates/vizier-http/tests/platform_links.rs`. Codes are issued through `platform_links::connect::issue` against the test deps. Cover:
  - preview consumes nothing
  - redeem links
  - a second redeem is `404`
  - expired is `404` with the same message as unknown
  - linked elsewhere is `409` and doesn't name the owner
  - a replace reports `replaced`
  - the 11th attempt is `429` per user and per address
- [ ] T028 [P] [US1] Conformance vectors for the three routes in `crates/vizier-http/tests/conformance/groups/connections.rs`. Remove them from `crates/vizier-http/tests/conformance/pending.txt` and record the baseline with `CONFORMANCE_RECORD=1`

### Design and WebUI (after T003 is reviewed)

- [ ] T029 [P] [US1] Build `ConnectConfirm` in `design/src/components/ConnectConfirm.tsx` with a story per state (ready, replaces, unavailable, linked elsewhere, busy), as drawn in T003
- [ ] T030 [P] [US1] Add `normalizeCode`, `stashCode`, `takeCode` (sessionStorage in try/catch) and `codeFromHash` in `webui/app/lib/connect.ts`, with vitest cases in `webui/app/lib/connect.test.ts`
- [ ] T031 [P] [US1] Add `listConnections`, `previewConnection` and `redeemConnection` in `webui/app/api/endpoints/connections.ts`, through the transport seam only
- [ ] T032 [US1] Add `webui/app/routes/connect.tsx`, registered in `webui/app/routes.ts` **outside** `RequireAuth`:
  - read the code from `location.hash`, stash it and clear the hash with `history.replaceState`
  - when signed out, navigate to `/login?returnTo=/connect` (no code in the URL)
  - when signed in, `takeCode` → preview → `ConnectConfirm` → redeem → navigate to `/settings/profile`
- [ ] T033 [US1] Add the code field to `webui/app/routes/settings-profile.tsx`, opening the same `ConnectConfirm` flow in a `Dialog` (with ×, Escape, outside press)

### E2E

- [ ] T034 [US1] Add a `dummyplug_e2e` case `connect_code_links_once` in `crates/vizier-core/tests/dummyplug_e2e.rs`: issue a code for a fake Discord person, redeem it through the service as a test user, check `find_link`, and check that a second redeem is unavailable

**Checkpoint**: people can link accounts. Agents still see the old labels.

---

## Phase 4: User Story 2: Agents see who really sent a message (P1)

**Goal**: every platform request reads as the Vizier username with `verified: true`, or `Unregistered User` with `verified: false`, with the platform identity in metadata. OWNER.md lists only verified links.

**Independent Test**: dummyplug `sender` echo for a linked person, an unlinked person and a web request (quickstart §0 table). This works with a link row written directly, without US1's UI.

### Harness

- [ ] T035 [P] [US2] In the `dummyplug/` submodule, add §9 `sender`: when the newest user message's command text, trimmed, equals `sender` (case-insensitive), reply with that message's frontmatter block verbatim, or `(no frontmatter)`. Document it in `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md` and check §1–§8 still behave the same

### Channels: one resolver everywhere

- [ ] T036 [US2] Discord, in `crates/vizier-core/src/channels/discord/mod.rs`: replace `user_label` and the inline `@{} (DiscordId: {})` sender `format!`s (interaction path ~L892, message path ~L1467, side chat) with `PlatformPerson` from the twilight user (`id`, `name` as username, `global_name` as display name) → `resolve` → `stamp`. Leave `render_mentions` (L1815) as it is
- [ ] T037 [P] [US2] Discord questions, in `crates/vizier-core/src/channels/discord/questions_handlers.rs`: replace the two `@{} (DiscordId: {})` answerer labels (L58, L322) with `resolve(...).label`, and stamp the question-answer request
- [ ] T038 [US2] Telegram, in `crates/vizier-core/src/channels/telegram/mod.rs`: build `PlatformPerson` from `msg.from` (`id`, `username`, `full_name()`), **not** `chat_id`, which fixes R1 Today's bug. Replace the labels at ~L486 and ~L990 with `resolve` → `stamp` on every request built from them (chat, silent read, skill command, abort, btw, queue)
- [ ] T039 [P] [US2] Telegram questions, in `crates/vizier-core/src/channels/telegram/questions_handlers.rs`: replace `question_answerer` with `resolve(...).label` and stamp the request
- [ ] T040 [US2] Key the side-chat limits on `"{platform}:{platform_user_id}"` instead of the label, in `crates/vizier-core/src/channels/side_chat.rs` and the Discord check at `crates/vizier-core/src/channels/discord/mod.rs` (~L764) (R8)

### Prompt

- [ ] T041 [US2] Rewrite `owner_md(profile, links)` in `crates/vizier-core/src/agents/agent/system_prompt/user.rs` to print `vizier_username`, one `<platform>: "<handle>" (id "<id>"), verified link` line per link, and the aliases. Never print the legacy fields. Update `served_owner_md`'s caller in `crates/vizier-core/src/agents/agent/system_prompt/worker.rs` if its embedded text changes
- [ ] T042 [US2] Load the owner's links with the profile at `crates/vizier-core/src/agents/agent/mod.rs` (~L200, `list_links(owner_id)`), store them beside `owner_profile`, and pass them to `owner_md` (~L493). Re-read them on every turn the way the profile is read, so a link applies without a restart (FR-009)
- [ ] T043 [P] [US2] Add the one BOOT.md sentence from contracts/agent-sender.md to the BOOT template under `crates/vizier-core/src/templates/` (the file `constant::BOOT_MD` loads)

### Tests

- [ ] T044 [P] [US2] Unit tests in `crates/vizier-core/src/platform_links/sender.rs` with an in-memory `SqlStorage`: linked gives the Vizier username and `true`; unlinked gives `Unregistered User` and `false`; metadata always carries the platform id, handle and display name; a changed handle is refreshed; two unregistered people keep distinct `platform_user_id`
- [ ] T045 [P] [US2] Unit tests for `owner_md` in `crates/vizier-core/src/agents/agent/system_prompt/user.rs`: a verified link is listed, a hand-typed legacy id is absent, and no links gives the existing "No additional profile data" line
- [ ] T046 [US2] `dummyplug_e2e` cases in `crates/vizier-core/tests/dummyplug_e2e.rs`, each building the request with `resolve` + `stamp` and asking `sender`: `unregistered_sender_reads_unregistered_user`, `linked_sender_reads_vizier_username`, `unlink_applies_on_next_message`, and `web_request_has_no_verified_line`

**Checkpoint**: US1 and US2 together are the MVP. Run quickstart §1–§2 live.

---

## Phase 5: User Story 3: See and unlink connected accounts (P2)

**Goal**: a Connected accounts card on the profile, with an Unlink item in each row's menu. The hand-typed fields are gone.

**Independent Test**: connect, see the row, unlink from ⋯ › Unlink, and dummyplug `sender` reads `Unregistered User` (quickstart §1 step 8).

- [ ] T047 [US3] Add `service::platform_links::unlink(principal, platform)` (`404 not_linked` when absent; writes an `unlinked` event) in `crates/vizier-core/src/service/platform_links.rs`, and `DELETE /auth/users/me/connections/{platform}` in `crates/vizier-http/src/api/v1/connections.rs`, with a conformance vector in `crates/vizier-http/tests/conformance/groups/connections.rs`
- [ ] T048 [P] [US3] Route tests in `crates/vizier-http/tests/platform_links.rs`: unlink returns `204` and the next `resolve` is unregistered; a second unlink is `404`; unlink only touches the caller's own link
- [ ] T049 [P] [US3] Build `ConnectedAccountRow` (linked with ⋯ menu → Unlink, not linked with hint, busy) in `design/src/components/ConnectedAccountRow.tsx`, with stories, as drawn in T002. The menu and the confirmation follow the dismissal convention (`useDismiss`, `Dialog`)
- [ ] T050 [US3] In `webui/app/routes/settings-profile.tsx`, replace the four `TextField`s and the "Who you are on other channels" card with a Connected accounts card: one `ConnectedAccountRow` per platform, the code field from T033, and the recent-changes list from `listConnections().events`. Unlink confirms, calls `DELETE`, then refreshes. Keep the alias field. Add `unlinkConnection` to `webui/app/api/endpoints/connections.ts`

**Checkpoint**: people can see and remove their links.

---

## Phase 6: User Story 4: Existing hand-typed ids after upgrading (P2)

**Goal**: old values identify no one, show as "Not verified", can be removed, and are replaced by a link.

**Independent Test**: quickstart §3.

- [ ] T051 [US4] In `crates/vizier-core/src/service/profile.rs`: a non-null `discord_id`/`discord_username`/`telegram_id`/`telegram_username` in `ProfileChanges` is refused with a `ServiceError` that maps to `422 connect_required`; `null` clears. Map the kind in `crates/vizier-http/src/error.rs` if it is a new one, and update the `PUT /auth/users/me/profile` handler and OpenAPI in `crates/vizier-http/src/api/v1/auth.rs`
- [ ] T052 [US4] Add `DELETE /auth/users/me/connections/{platform}/unverified` (clears that platform's legacy pair) in `crates/vizier-http/src/api/v1/connections.rs` with its service function, and expose `unverified` in the `list` response (T020), with a conformance vector
- [ ] T053 [P] [US4] Tests: `crates/vizier-http/tests/platform_links.rs`, where a profile PUT with a Discord id is `422` and with `null` clears; and `dummyplug_e2e` case `legacy_id_is_unregistered`, where a profile with a hand-typed Discord id gives `Unregistered User` for that id
- [ ] T054 [US4] Draw the not-verified row from T002 in `webui/app/routes/settings-profile.tsx` with a remove action calling the `unverified` DELETE
- [ ] T055 [US4] Add #82 checklist entries naming the legacy `UserProfile` platform fields, the `422 connect_required` refusal, the clear-on-link step in `redeem_connect_code`, and the `unverified` route and listing

---

## Phase 7: Polish & cross-cutting

- [ ] T056 [P] Write `docs/src/configuration/channel-identities.md` covering `/connect`, `base_url`, what agents see, unlinking and the upgrade note. Add it to `docs/src/SUMMARY.md`, and link it from the Discord/Telegram integration docs
- [ ] T057 [P] Update `crates/vizier-core/src/config/base_url.rs`'s module doc ("nothing reads it yet") to name the connect link as its reader, and update `REQUESTED_BY_DESC` in `crates/vizier-core/src/agents/tools/scheduler/mod.rs` so its example is a Vizier username or `Unregistered User`
- [ ] T058 Update CLAUDE.md with a short **Platform links** section (module, tables, resolver as the one sender path, no cache, codes hashed and fragment-only)
- [ ] T059 Run `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, `cargo test -p vizier-core --no-default-features --features channel-discord`, the same with `channel-telegram`, and `just web-check`. Don't run `cargo fmt`
- [ ] T060 Run quickstart.md §0 (automated), §1 and §2 (live, with bot tokens) and §3 (upgrade) against `just run`, and record the outcomes in the PR description

---

## Dependencies & Execution Order

- **Phase 1** first. T002 and T003 block the design and WebUI tasks only (T029–T033, T049, T050, T054), not the Rust work.
- **Phase 2** blocks every story.
- **US1 (Phase 3)** and **US2 (Phase 4)** can go in parallel after Phase 2. US2's tests write link rows through storage, so they don't need US1.
- **US3** needs US1's routes and the T033 code field. **US4** needs US1's `list`/`redeem` (T020, T007).
- **Phase 7** comes last.

Inside a story: core before channels and HTTP, HTTP before WebUI, and tests beside their code.

## Parallel examples

- **Phase 2**: T006 (PostgreSQL), T009 (types), T010 (code helpers), then T014, T015 and T016 together.
- **US1**: T017 (text), T021 (Discord) and T022 (Telegram) after T018; T027 and T028 after T025; T029, T030 and T031 together.
- **US2**: T035 (dummyplug), T037 and T039 (question handlers), T043 (BOOT) and T044/T045 (tests) together.

## Implementation strategy

1. **MVP = US1 + US2** (Phases 1–4): linking works and agents see verified senders. Ship-ready even without the new profile card, since the code can be redeemed through `/connect`.
2. **US3**: the Connected accounts card and Unlink.
3. **US4**: the upgrade path. Before release, at least T051 must land with US2, so nobody can type a new unverified id into a profile.
4. **Polish**: docs, CLAUDE.md, the full check matrix and the live quickstart.
