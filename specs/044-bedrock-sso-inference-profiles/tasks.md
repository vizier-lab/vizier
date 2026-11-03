# Tasks: AWS Bedrock over SSO with Application Inference Profiles

**Feature**: `044-bedrock-sso-inference-profiles` | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)
**Input**: [spec.md](./spec.md), [plan.md](./plan.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: included — the plan's *Testing* section commits to `cargo test -p vizier-core` for the device-flow state machine, ARN parsing, `entry_pricing`, session storage/encryption and credential caching, plus the dummyplug e2e gate the constitution requires for agent-observable change. Test tasks sit in the phase they belong to, next to the code they prove.

**Organization**: one phase per user story, in spec priority order, so each story is independently implementable and testable.

**Spec revision folded in**: validation run 7 of 2026-11-02 ("pricing not set diterima jika ada reason itu") tightened FR-034a.2, FR-034a.3, FR-034b and SC-021: an unpriced entry must carry a **cause-specific, non-empty reason plus the way out**, not a bare label. The tasks below already assume it; T043 and T046/T047 are where it is proven.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: parallelizable — different files, no dependency on an incomplete task
- **[Story]**: US1-US5, from spec.md
- Every task names the exact file path. Paths are the **Cargo workspace** (`crates/*`), not the pre-split `src/`; verified against disk on 2026-11-02.

## Path Conventions

- Behaviour: `crates/vizier-core/src/`
- HTTP surface: `crates/vizier-http/src/api/v1/`
- UI: `webui/app/` (git submodule — its own commit, **before** the root pin bump)
- Docs: `docs/src/` (the single docs source; `landing/` is generated from it)
- Inline unit tests live in `#[cfg(test)] mod tests` in the file under test; cross-crate e2e lives in `crates/vizier-core/tests/`

## Commit rule for this feature (owner, 2026-11-02)

One phase = one commit, conventional format, explicit paths (`git add crates/... specs/...`, never `git add .`), never `package-lock.json`. Each phase's messages are given at its checkpoint. Cross-repo order is **design -> webui -> vizier**; the root pin bump is its own commit, after the webui commit is pushed. An unconventional message is rejected, not pushed.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: establish a known-good baseline before touching a large surface.

- [X] T001 Confirm the branch and a green baseline: `just status` (root + 4 submodules on `044-bedrock-sso-inference-profiles`), then `just check` — record the result so a later failure is attributable to this feature.
- [X] T002 [P] Read `crates/vizier-core/src/llm/resolve.rs`, `llm/auth.rs`, `llm/bedrock.rs` and `llm/media/mod.rs` end to end and note the exact extension points named in plan.md Summary, so the new credential source is registered where credentials are built (`resolve.rs:270`, `media/mod.rs:63`) rather than in the turn path.
- [X] T003 [P] Read `crates/vizier-core/src/catalogue/{builtin,guide,mod}.rs` and record how the existing `amazon-bedrock` entry is declared, where `guide::for_provider` dispatches, and the four call sites that need the alias (plan.md, risk "New risk found in design").

**Checkpoint**: baseline green, extension points located. No commit (nothing changed).

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the provider identity, the data shapes, the stored-session plumbing and the ARN grammar. Every user story stands on these.

**CRITICAL**: no user story work begins until this phase is complete.

- [X] T004 Add `ProviderId::AMAZON_BEDROCK_SSO` (constant `amazon-bedrock-sso`, display name "Amazon Bedrock (SSO)") in `crates/vizier-core/src/config/provider.rs`, and make every provider-id consumer accept it — compile the workspace and fix each non-exhaustive `match` the compiler names (FR-001).
- [X] T005 [P] Add the non-secret SSO fields to the Bedrock credentials struct in `crates/vizier-core/src/schema/provider.rs`: `start_url`, `sso_region`, `account_id`, `role_name`, `sso_profile`, `profiles` — all `Option`/`Vec` with `#[serde(default)]` so an existing row still deserialises (data-model "SSO connection", FR-008, FR-020).
- [X] T006 [P] Add `ProfileEntry` to `crates/vizier-core/src/schema/provider.rs` with `arn`, `name`, `context`, `output`, `tool_call`, `image_input`, `model`, `price: Option<Cost>`, `edited`, matching data-model "profile entry" and the `CustomModel.edited` convention at `schema/provider.rs:225` (FR-020, FR-034a).
- [X] T007 [P] Add `crates/vizier-core/src/schema/sso.rs`: the non-secret `SsoSession` view (profile, start URL, both regions, account, role, expiry, state) and `SsoSessionSecret` (access/refresh token, client id/secret, expiries, `issued_at`, `revision`) — do **not** derive `Serialize` on the secret (research D8, FR-009b).
- [X] T008 Create `crates/vizier-core/src/storage/sso_session.rs` with the `SsoSessionStorage` trait: read by profile, compare-and-swap write on `revision`, delete by profile (research D3, FR-009, FR-014).
- [X] T009 Create `crates/vizier-core/src/storage/sqlite/sso_session.rs` implementing that trait over `rusqlite`, one row per profile, and register `CREATE TABLE bedrock_sso_session (profile TEXT PRIMARY KEY, data TEXT NOT NULL)` beside the existing `provider_config` precedent at `crates/vizier-core/src/storage/sqlite/mod.rs:302`; export the module from `crates/vizier-core/src/storage/mod.rs` (FR-009a, SC-017a).
- [X] T010 Add `protect_sso_session` / `decrypt_sso_session` to `crates/vizier-core/src/encryption/fields.rs`, with the AAD binding `vizier:bedrock-sso:v1\x1fsession\x1f<profile>` (the unit separator, per data-model.md:66) and fail-closed handling for `KeyMissing` / `KeyMismatch` (data-model "SSO session secret", FR-009a, FR-009f).
- [X] T011 [P] Add `parse_inference_profile_arn` and `ArnError` to `crates/vizier-core/src/llm/bedrock.rs`, accepting **both** `inference-profile/` and `application-inference-profile/`, all five partitions, with region/account/profile-id segments required, trimming surrounding whitespace before the match (FR-023).
- [X] T012 [P] Write the parser's unit tests in `crates/vizier-core/src/llm/bedrock.rs` under `#[cfg(test)]`: both resource types across all five partitions accepted; foundation-model ARN, another service's ARN, bare model id, truncated ARN, console URL and padded values refused; the returned `region` equals the ARN's region segment (FR-023, SC-008).
- [X] T013 Add the session round-trip test — envelope written, read back, and a **different** profile's envelope failing to decrypt — in `crates/vizier-core/src/storage/sqlite/sso_session.rs` under `#[cfg(test)]` (FR-009a, FR-009b).

**Checkpoint**: `just check` green. Commit — `feat(bedrock-sso): add the sso credential schema, arn parser and session storage`

---

## Phase 3: User Story 1 - Sign in with AWS SSO and talk to Bedrock (Priority: P1) (MVP)

**Goal**: a person with no access keys connects the SSO provider from the form alone, signs in through IAM Identity Center, and holds a conversation through Bedrock using the resolved role.

**Independent Test**: with an Identity Center instance, one assigned account and role and Bedrock model access, connect the provider from Settings > Providers (no AWS CLI, no shell), then hold a multi-turn conversation on that provider; the connection reports account, role and session expiry, and the conversation continues across turns.

- [X] T014 [US1] Create `crates/vizier-core/src/llm/sso/mod.rs` with the module's entry points and re-exports, and register it from `crates/vizier-core/src/llm/mod.rs` (plan.md structure decision: the flow lives under `llm/` because it produces a credential, not a top-level module).
- [X] T015 [US1] Create `crates/vizier-core/src/llm/sso/error.rs` with the ten distinct failures of FR-041, mapping each to `LlmError`/`ServiceError` at the boundary per constitution V, and assert in a unit test that no variant's `Display` can carry a token (FR-035, contracts/bedrock-sso-wire.md §7).
- [X] T016 [US1] Create `crates/vizier-core/src/llm/sso/client.rs`: one-time client registration, device authorization and token creation against the endpoints of contracts/bedrock-sso-wire.md §1-§3, derived from the **SSO region**, using `VizierDependencies.http` (FR-006).
- [X] T017 [US1] Create `crates/vizier-core/src/llm/sso/portal.rs`: list accounts, list roles, get role credentials (contracts §4), keeping the free-text fallback only for the case where the portal lists nothing (FR-007).
- [X] T018 [US1] Create `crates/vizier-core/src/llm/sso/session.rs`: persist and load the session through `SsoSessionStorage` + the encryption helpers, expose expiry, and implement the CAS write of `revision` (FR-009, groundwork for FR-014).
- [X] T019 [US1] Write the device-flow state machine test against a local stub server bound on `127.0.0.1:0` (research D9) in `crates/vizier-core/src/llm/sso/client.rs` under `#[cfg(test)]`: success, denial, timeout and a code confirmed twice — each producing its own error and **no** stored session on failure (FR-006, US1 acceptance 8).
- [X] T020 [US1] Create `start_sign_in` / `complete_sign_in` in the new `crates/vizier-core/src/service/providers_sso.rs`, callable **before** a connection is saved (FR-012), returning the user code and verification URI and never the device code (contracts §3, data-model state machine).
- [X] T021 [US1] Implement `discover_accounts` / `discover_roles` / `sign_out` in `crates/vizier-core/src/service/providers_sso.rs`, with `sign_out` deleting the stored session row (FR-009c).
- [X] T022 [US1] Add `SsoRoleCredential` (a `CredentialSource` impl) to `crates/vizier-core/src/llm/auth.rs`, holding `Option<RoleCredentials>` behind a mutex in memory only, plus `bedrock_role_credentials` as the one shared helper both chat and media call (FR-002, FR-009d, research D5).
- [X] T023 [US1] Add `resolve_bedrock_sso` to `crates/vizier-core/src/llm/resolve.rs` beside `resolve_bedrock`, reusing the same dialect, signer, endpoint and error translation and differing only in the credential source (FR-002, FR-003).
- [X] T024 [US1] Add the SSO branch to `crates/vizier-core/src/catalogue/builtin.rs` (`sso_bedrock()` + the `model_source` field) and make the alias resolve in `crates/vizier-core/src/catalogue/mod.rs` at **all four** call sites — `models`, `provider`, `model`, `resolve_with` (FR-001, FR-030, plan.md risk "second ProviderId").
- [X] T025 [US1] Add `bedrock_sso()` to `crates/vizier-core/src/catalogue/guide.rs` and dispatch `guide::for_provider` per id so each Bedrock provider answers with its **own** guide and neither reports the other's or none (FR-001a, FR-038).
- [X] T026 [P] [US1] Add the SSO credential fields to the provider upsert and response in `crates/vizier-core/src/service/providers.rs`, with `check_sso_connection` enforcing the five-present rule, the `sso_region`/`region` defaulting (never silently equalised afterwards), `sso_profile` uniqueness, and `base_url`/`api_key` refused (data-model validation, FR-010, FR-011).
- [X] T027 [P] [US1] Add the SSO provider's guide content — steps, the `bedrock:InvokeModel`-only policy, the AWS and Vizier docs links — as data in `crates/vizier-core/src/catalogue/guide.rs` (FR-012b, FR-038).
- [X] T028 [US1] Add the sign-in endpoints to `crates/vizier-http/src/api/v1/providers/mod.rs`: `POST /{id}/sso/sign-in`, `GET /{id}/sso/accounts`, `GET /{id}/sso/accounts/{account_id}/roles`, `POST /{id}/sso/sign-out`, all `utoipa`-annotated, with no device code, token or secret half in any response body (contracts §3, FR-009b).
- [X] T029 [US1] Add the SSO types and sign-in calls to `webui/app/api/endpoints/providers.ts` and the field labels/`PLAIN_FIELDS` to `webui/app/lib/providers.ts` (webui submodule).
- [X] T030 [US1] Extend `webui/app/screens/providers/ConnectForm.tsx` with the sign-in action, the code-and-URL panel, account/role selection and success/failure reporting — kept **generic** on provider id, with no `if (id === 'amazon-bedrock-sso')` branch in the component (constitution II).
- [X] T031 [US1] Add a vitest case to `webui/app/screens/providers/ConnectForm.test.tsx` covering the sign-in panel's pending, success and denied states, and proving no token, code or secret is rendered (FR-035).
- [X] T032 [US1] Run `cd webui && npm run typecheck && npm test` and, when the API shape changed, regenerate `webui/app/api/schema.d.ts` with `just openapi` — never by hand (plan.md, rule W3).

**Checkpoint**: the MVP — one SSO connection, one multi-turn conversation, nothing else. Commit per repo, in order:
1. webui: `feat(providers): sign in to bedrock sso through the identity center`
2. root: `feat(bedrock-sso): sign in through iam identity center and chat with role credentials`
3. root, after the webui commit is pushed: `chore(webui): bump for the bedrock sso sign-in form`

---

## Phase 4: User Story 2 - Choose the team's Application Inference Profile by name (Priority: P1)

**Goal**: the connection carries a managed list of inference-profile entries, each selected by name, sent to the endpoint unchanged, and priced from a catalogue model the person pairs it with.

**Independent Test**: with an SSO connection, save two profiles over the same foundation model, point two agents at them, send each a message and confirm both reply and both are metered separately; then check that an ARN-only entry chats unpriced and is flagged "pricing not set".

- [X] T033 [US2] Implement the entry CRUD (`validate_entries`, add/edit/remove, duplicate-ARN refusal, region-mismatch refusal, in-use `409`) in `crates/vizier-core/src/service/providers_sso.rs`, enforcing data-model "profile entry" validation rules 1-5 in that one place (FR-021, FR-024, FR-025, FR-029).
- [X] T034 [US2] Extend `resolve_bedrock_sso` in `crates/vizier-core/src/llm/resolve.rs` to send a saved entry's ARN as the model string **unchanged**, one encoded path segment, and to refuse a well-formed ARN that is not in the connection's list with `ProfileNotFound` (FR-027, FR-030, data-model "agent model selection").
- [X] T035 [US2] Resolve an entry's `context`/`output`/`tool_call`/`image_input` in `crates/vizier-core/src/llm/resolve.rs` so an unset field stays `unknown` and the agent override still wins on top via `limits()` at `resolve.rs:156` — never inferred from the ARN, never copied from the paired model (FR-028, FR-034a.6).
- [X] T036 [US2] Add `entry_pricing` to `crates/vizier-core/src/service/catalogue.rs` returning the five outcomes of the data-model table (`Explicit`, `FromCatalogue`, `NotResolved{model}`, `NotSet`, `NoCatalogue`) — `price` first and short-circuiting, then the catalogue-readiness check (**no Bedrock list ⇒ `NoCatalogue`, decided before `model` is looked at**, so it wins over both `NotSet` and `NotResolved`), then `model` resolved through the provider alias, then unpriced, exactly the precedence data-model fixes — and have each outcome carry its own reason string, so no caller can render an absence without one (FR-034a.1-a.5, FR-034b, SC-021, research D13).
- [X] T037 [US2] Add `Pricer::with_entries` and the per-report entry map to `crates/vizier-core/src/usage/mod.rs`, looking an ARN up before the catalogue and reusing the existing `None`-means-unpriced representation at `usage/mod.rs:69` (FR-034, FR-034b).
- [X] T038 [US2] Pass the entries into the pricer at both report builders — `crates/vizier-core/src/service/dream.rs` (`price_cycles`) and `crates/vizier-core/src/service/overview.rs` (`counts`) — and at every other `Pricer` construction site, so no report prices an ARN from the catalogue alone (FR-034a.1).
- [X] T039 [US2] Publish the entry's computed `pricing` state on the provider row in `crates/vizier-core/src/service/providers.rs` and through it on the model list in `crates/vizier-core/src/service/catalogue.rs`: not just the label but a non-empty, cause-specific **reason with the way out** — never-chosen says no model is paired and that picking one or entering a price fixes it; gone says so and **puts the missing model id inside the reason**; the no-catalogue case says there is nothing to price against yet and that it resolves itself (the Bedrock list is absent — the observable form of a cold start). The reason is derived per request from the entry's current state, so re-picking or entering a price clears it with no manual edit. Rendering of the enum, no stored column, no migration (contracts §4a-§5, FR-034a.2, FR-034a.3, FR-034b, SC-021).
- [X] T040 [US2] Add the entry endpoints and the SSO id's models list to `crates/vizier-http/src/api/v1/providers/mod.rs`, keeping the ARN visible wherever the person can verify it (contracts §4, §5, FR-022, FR-026).
- [X] T041 [US2] Add the searchable catalogue-model source for the pricing dropdown to `crates/vizier-core/src/service/catalogue.rs` — the `amazon-bedrock` list as currently loaded, name plus id, searchable — with **no** AWS call and no new list (FR-034a.1, FR-034c).
- [X] T042 [P] [US2] Unit-test the ARN rejection messages and `validate_entries` rule order (shape before duplicate before region) in `crates/vizier-core/src/service/providers_sso.rs` under `#[cfg(test)]` (FR-023, FR-024, FR-025).
- [X] T043 [P] [US2] Unit-test **all five** `entry_pricing` outcomes — `Explicit`, `FromCatalogue`, `NotResolved{model}`, `NotSet` **and `NoCatalogue`** — in `crates/vizier-core/src/service/catalogue.rs` under `#[cfg(test)]`, including the precedence the data-model table fixes: an explicit price wins over every absence, and **`NoCatalogue` wins over both `NotSet` and `NotResolved`** — assert that for an entry with **no model** *and* for one with a **valid model paired**, by building the pricer against an empty catalogue (`Catalogue::new(CatalogueData::default())`), so `NoCatalogue` has an automated test of its own rather than only quickstart 5b. Plus the refresh case: a changed catalogue relabels and reprices an existing entry without rewriting it (FR-034a.1, FR-034a.3, FR-034a.4, FR-034b, SC-020, SC-021). **Also assert SC-021's reason rules**: all **three** unpriced outcomes produce a non-empty reason that names its own cause, the gone case has the missing id **inside** the reason, no two of the three reasons share wording, and re-picking a model or setting a price clears the reason with nothing else touched.
- [X] T044 [P] [US2] Unit-test that a missing or stale price never blocks a turn: the resolve path builds the model without consulting `entry_pricing`, and the usage row keeps provider, ARN and tokens with `None` cost (FR-034a.5, FR-034b, SC-021).
- [X] T045 [US2] Create `webui/app/screens/providers/ProfileEntryForm.tsx`: the ARN, the optional name and limits, the searchable model `Picker` from `@vizier/design` and the explicit price — the existing picker, not a new component (rule W1, FR-034a).
- [X] T046 [US2] Extend `webui/app/screens/providers/ModelsDrawer.tsx` with the entry list, add/edit/remove and the pricing state per row: the label **and** its reason, visible without a further click — never a bare "pricing not set" with nothing behind it, no placeholder, and no shared wording between the three causes (FR-022, FR-026, FR-034a.2, FR-034a.3, SC-021).
- [X] T047 [US2] Extend `webui/app/api/endpoints/providers.ts` with the entry types and calls, and `webui/app/lib/providers.ts` with the entry helpers — including the pricing-reason rendering — keeping the shape logic in `lib/` with a vitest test that all three unpriced causes render a distinct, non-empty reason and that a priced entry renders none (rule W5, SC-021).
- [X] T048 [US2] Run `cd webui && npm run typecheck && npm test` and regenerate `webui/app/api/schema.d.ts` via `just openapi` if the entry payload changed the schema.

**Checkpoint**: profiles are selectable by name and priced, or visibly unpriced. Commits:
1. webui: `feat(providers): manage inference profile entries and pick one by name`
2. root: `feat(bedrock-sso): manage inference profile entries and price them from the catalogue`
3. root, after the webui commit is pushed: `chore(webui): bump for the inference profile entry editor`

---

## Phase 5: User Story 3 - The session refreshes itself, and says so when it cannot (Priority: P2)

**Goal**: role credentials renew silently before expiry, once per connection under concurrency; an ended session says so plainly, in the foreground and in background work, and re-sign-in needs only the confirmation code.

**Independent Test**: run turns past the role-credential expiry and confirm they succeed with no re-sign-in; then end the session and confirm the next interactive turn names the session (not the key, region or model), a scheduled task fails promptly instead of hanging, and the agent's health reports it until a new sign-in.

- [X] T049 [US3] Implement `renew()` in `crates/vizier-core/src/llm/sso/session.rs`: one `CreateToken` plus one `GetRoleCredentials`, zero extra requests while the cache is valid, renewal threshold 5 minutes before `expires_at` (FR-013, FR-009d).
- [X] T050 [US3] Serialize renewal **per connection** via the mutex in `SsoRoleCredential` in `crates/vizier-core/src/llm/auth.rs`, so N concurrent requests at an expiry produce exactly one renewal and all use the fresh credentials (FR-014, US3 acceptance 2).
- [X] T051 [US3] Implement `invalidate()` plus retry-once on a credential rejection in `crates/vizier-core/src/llm/auth.rs`, ending the turn with the provider's own explanation on a second failure — no loop (FR-015).
- [X] T052 [US3] Map an ended session to its own `SsoError` variant and wording in `crates/vizier-core/src/llm/sso/error.rs`, so a session failure is never reported as a wrong key, region or model and always carries the re-sign-in action (FR-016, contracts §6).
- [X] T053 [US3] Make the background paths fail promptly rather than wait: verify and, where needed, correct the SSO failure handling reached by `crates/vizier-core/src/scheduler/`, the dream path in `crates/vizier-core/src/service/dream.rs` and background/sub-agent turns — a session failure must not retry in a loop or block on a sign-in (FR-017).
- [X] T054 [US3] Report the failure on agent health in `crates/vizier-core/src/service/providers.rs` and the agent status surface, clearing it on the next successful sign-in without a Vizier restart (FR-018).
- [X] T055 [US3] Make re-sign-in reuse the saved start URL, regions, account and role so only the confirmation code is asked for, and restart exactly the agents on this connection through the existing provider-change path (this id only) in `crates/vizier-core/src/service/providers.rs` (FR-008, FR-019, US3 acceptance 5).
- [X] T056 [US3] Implement the compare-and-swap write in `crates/vizier-core/src/storage/sqlite/sso_session.rs` so two processes on one workspace renewing at once stay idempotent against the store, and unit-test the losing writer's retry (FR-014, spec edge case "two Vizier processes").
- [X] T057 [P] [US3] Unit-test credential caching in `crates/vizier-core/src/llm/auth.rs` under `#[cfg(test)]`: valid cache means no portal call; near-expiry means one renewal; concurrent callers mean one renewal; a rejection means exactly one retry then the provider's error (FR-013, FR-014, FR-015).

**Checkpoint**: expiry is unremarkable, and an ended session is self-explaining. Commit — `feat(bedrock-sso): renew role credentials and report an ended session`

---

## Phase 6: User Story 4 - Memory and images use the same SSO connection (Priority: P2)

**Goal**: embeddings and image generation run through the one connection, with the documented defaults when the field is empty, and an inference-profile ARN is refused as a media model before the request.

**Independent Test**: with only an SSO connection, embed and search a distinctive phrase and generate one image; end the session and confirm all three capabilities report the same cause and recover together after one sign-in; then confirm the media pickers offer foundation models, not ARNs.

- [X] T058 [US4] Switch `endpoint()` at `crates/vizier-core/src/llm/media/mod.rs:63` to the shared `bedrock_role_credentials` helper so media uses the same connection and credential source as chat, with no second sign-in (FR-002, FR-033).
- [X] T059 [US4] Refuse an ARN **before** the family check in `crates/vizier-core/src/llm/media/bedrock.rs`, with a message naming the capability and the models that do work — `parse_inference_profile_arn` succeeding is the test, in both resource types (FR-033a).
- [ ] T060 [US4] Keep the documented defaults unchanged when the field is empty — `amazon.titan-embed-text-v2:0` and `amazon.nova-canvas-v1:0` at `crates/vizier-core/src/llm/media/bedrock.rs:16,18` — and let the provider's unsupported-family refusal pass through (FR-033b).
- [ ] T061 [US4] Restrict what each capability offers in `crates/vizier-core/src/service/catalogue.rs` and the model list: chat offers catalogue models **and** saved entries; embedding and image offer the foundation-model families only; an ARN entry stays listed for chat and is never substituted (FR-033a, contracts/media-and-test.md §2-§3).
- [ ] T062 [P] [US4] Unit-test the media refusal and the defaults in `crates/vizier-core/src/llm/media/bedrock.rs` under `#[cfg(test)]`: an ARN refused before any request for both capabilities and both resource forms, and an empty field resolving to its default (FR-033a, FR-033b).
- [ ] T063 [P] [US4] Unit-test that a session failure on a media call writes no wrong-sized vector and no broken file in `crates/vizier-core/src/llm/media/bedrock.rs` under `#[cfg(test)]` (US4 acceptance 4).

**Checkpoint**: an SSO-only workspace reaches memory recall and image generation. Commit — `feat(bedrock-sso): embed and generate images over the same sso connection`

---

## Phase 7: User Story 5 - Operate it: who uses it, what it costs, how to remove it (Priority: P3)

**Goal**: usage is attributed to this provider and priced per entry, a connection change restarts exactly its own agents, deletion names the affected agents first, and no screen or log ever shows a token.

**Independent Test**: run a chat turn and confirm the usage view attributes it to this provider and not the other Bedrock one; change the region and confirm only this connection's agents restart; delete a connection used by two agents and confirm the affected list is shown first and both agents then report the missing connection.

- [ ] T064 [US5] Attribute usage to `amazon-bedrock-sso` on every answered completion, priced through the entry map, with neither Bedrock provider's rows merged, in `crates/vizier-core/src/service/usage.rs` and `crates/vizier-core/src/usage/mod.rs` (FR-034, US5 acceptance 1).
- [ ] T065 [US5] Replace the SSO connection test in `crates/vizier-core/src/service/usage.rs` with the invoke-based check — a minimal invoke against the selected model or ARN, runnable before save, reporting the model it invoked and surfacing the provider's refusal verbatim (FR-012a, contracts/media-and-test.md §1).
- [ ] T066 [US5] Restart exactly the agents on this connection on a region, account or role change, reusing the existing provider-change path and adding the session row deletion, in `crates/vizier-core/src/service/providers.rs` (FR-019, FR-040).
- [ ] T067 [US5] Name the affected agents before a connection or entry delete, reusing the existing in-use `409` plus `confirm=true` shape in `crates/vizier-core/src/service/usage.rs` and `crates/vizier-core/src/service/providers_sso.rs` (FR-029, FR-040, US5 acceptance 3).
- [ ] T068 [P] [US5] Audit every new payload, log line and error for secrets: the provider row in `crates/vizier-core/src/service/providers.rs` shows start URL, both regions, account, role, profile name and expiry, and no token, code, client secret or credential half (FR-009b, FR-035, SC-011).
- [ ] T069 [P] [US5] Audit the new `tracing` spans and fields across `crates/vizier-core/src/llm/sso/` and `crates/vizier-core/src/llm/auth.rs` for redaction, and confirm the log level matches the crate directives (constitution V, FR-035).
- [ ] T070 [US5] Confirm the display surfaces show state without secrets and that every unpriced entry shows a reason rather than a bare label: the settings row's state machine rendering in `webui/app/screens/providers/ConnectForm.tsx` and the entry rows in `webui/app/screens/providers/ModelsDrawer.tsx` (FR-009b, FR-034b, SC-021).

**Checkpoint**: the feature is operable, not just functional. Commits:
1. webui (only if T070 changed anything): `feat(providers): show the bedrock sso session state without secrets`
2. root: `feat(bedrock-sso): attribute usage, restart affected agents and gate deletion`
3. root, only if (1) happened and is pushed: `chore(webui): bump for the sso session state display`

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: documentation, the corrected test and the evidence the constitution's gates demand.

- [ ] T071 Correct the now-false assertion at `crates/vizier-http/src/api/v1/catalogue.rs:312` so it excludes the SSO id instead of being weakened — it must still prove every non-Bedrock provider has no guide (plan.md "Test that must change", FR-001a).
- [ ] T072 [P] Add the "Amazon Bedrock (SSO)" section to `docs/src/configuration/providers.md`: the five fields, the `bedrock:InvokeModel`-only policy, the two regions, the session's lifecycle, the ARN entry plus its pricing pairing, and what happens when the session ends (FR-039).
- [ ] T073 [P] Confirm the existing provider is untouched per `specs/044-bedrock-sso-inference-profiles/quickstart.md` §6: no migration, no reindex, its agents' behaviour identical, and a static-credentials connection still connecting after the feature ships (FR-003, FR-004).
- [ ] T074 Run `just check` over `crates/` and `webui/` (clippy, `cargo test --workspace`, webui typecheck) and paste the output — no "should pass" (constitution quality gate).
- [ ] T075 Walk `specs/044-bedrock-sso-inference-profiles/quickstart.md` §1-§2b offline (storage and encryption, the API surface, entry pricing) and record the verifier report it asks for.
- [ ] T076 Run the dummyplug e2e steps of `specs/044-bedrock-sso-inference-profiles/quickstart.md` §3 against a running binary, scripts in `scripts/` and the harness in `dummyplug/` (deterministic order tools, tool name, JSON request, echoed result), including the scripted session-ended and ARN-named-for-embedding failures with their expected error text (constitution e2e gate).
- [ ] T077 Run the secret-leak audit of `specs/044-bedrock-sso-inference-profiles/quickstart.md` §4: grep the workspace DB, the log and the API bodies for the session, tokens and both credential halves (SC-011, FR-035).
- [ ] T078 Run the two-start check of `specs/044-bedrock-sso-inference-profiles/quickstart.md` §1: the session survives a restart of the real binary against the same workspace, and startup makes no AWS call (FR-009, FR-037, SC-017a).
- [ ] T079 Record `specs/044-bedrock-sso-inference-profiles/quickstart.md` §5 (live volunteer checklist) as a release item rather than claiming it verified — the live Identity Center path is not CI (plan.md risk 2).

**Checkpoint**: gates evidenced. Commits:
1. root: `docs(configuration): document amazon bedrock sso`
2. root: `test(catalogue): exclude the sso provider from the no-other-guide assertion`
3. root: `test(bedrock-sso): cover the sso flow, inference profile entries and pricing` (only if test code was still uncommitted by then)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: after Setup — **BLOCKS all user stories**. T004 gates the workspace compiling; T005-T010 gate the session; T011-T013 gate every entry task.
- **User Stories (Phases 3-7)**: all after Phase 2.
  - **US1 before US2, US3, US4 in practice**: every other story needs a saved, signed-in connection to test against. US2's ARN work does not need US3 or US4; US3 does not need US2's pricing; US4 needs US1 only.
  - **US2 and US3 can run in parallel** once US1 lands (disjoint files beyond `ProfileEntry`, which Phase 2 fixed).
  - **US5 last**: it reuses the surfaces US1-US4 create.
- **Polish (Phase 8)**: after the stories it verifies; T071 can be done any time after T025.

### Within Each User Story

Schema -> services -> HTTP routes -> WebUI -> tests that need the whole path. A unit test that needs only a pure function (T012, T043, T044, T062) may be written beside it, in the same commit.

### Parallel Opportunities

- Phase 2: T005, T007, T011 and T012 touch different concerns and may be written together (T005 and T006 share one file: sequence them). T008-T010 plus T013 form one chain (trait, sqlite impl, encryption, round-trip test) and should not be split.
- US1: T026 and T027 run parallel to T014-T019 once T004-T013 are in.
- US2: T042, T043, T044 are [P] against each other; T036-T038 are one chain (`entry_pricing`, then pricer, then call sites).
- Across stories: US2's WebUI (T045-T048) and US3's renewal (T049-T057) touch disjoint files and can be done in the same window by two people.
- Polish: T072 and T073 are [P]; T068 and T069 are [P].

### Parallel Example: Phase 2

```bash
# Different files, no cross-dependency:
Task: "Add crates/vizier-core/src/schema/sso.rs"
Task: "Add parse_inference_profile_arn to crates/vizier-core/src/llm/bedrock.rs with its unit tests"
# Same file as each other, so in order:
Task: "Add the non-secret SSO fields to the credentials struct in crates/vizier-core/src/schema/provider.rs"
Task: "Add ProfileEntry to crates/vizier-core/src/schema/provider.rs"
```

---

## Implementation Strategy

### MVP first (User Story 1 only)

1. Phase 1: baseline green.
2. Phase 2: provider id, schema, storage, encryption, ARN grammar — without this nothing compiles or persists.
3. Phase 3: the device flow, the credential source, the form.
4. **STOP and VALIDATE**: a person with no access keys connects and holds a conversation (US1's independent test, against a real Identity Center instance when one is available, otherwise the offline halves plus the dummyplug step).
5. Report up; only then continue.

### Incremental delivery

Each phase is a shippable slice with its own commit set: US1 (connect and chat) -> US2 (profiles and pricing) -> US3 (renewal and expiry wording) -> US4 (media parity) -> US5 (operations) -> Polish. Never one commit for the whole feature, and never a commit whose message is not `type(scope): subject`.

### Practical notes

- **Do not bump the root's `webui` pin until the webui commit is pushed.** `just push` already orders submodules first for this reason.
- `webui/app/api/schema.d.ts` is generated by `just openapi`; a hand edit is a review blocker.
- `design/` is expected to stay untouched — the searchable picker exists. If a new component turns out to be needed, stop and report: that adds a third repo to the cascade.
- If `tasks.md` and `plan.md` disagree while implementing, follow `tasks.md` (it is later) and report the discrepancy rather than silently choosing.

---

## Notes

- [P] means different files, no dependency on an incomplete task.
- [Story] maps a task to its user story for traceability; Setup, Foundational and Polish carry no story label.
- Tests are part of the deliverable: the plan's Testing section commits to them.
- The live Identity Center path (quickstart §5) is a release checklist item, never a CI claim.
- Total: **79 tasks** — 3 setup, 10 foundational, 19 US1, 16 US2, 9 US3, 6 US4, 7 US5, 9 polish.

### Commit map (phase -> message)

| Phase | Repo | Message |
|---|---|---|
| 2 | root | `feat(bedrock-sso): add the sso credential schema, arn parser and session storage` |
| 3 | webui | `feat(providers): sign in to bedrock sso through the identity center` |
| 3 | root | `feat(bedrock-sso): sign in through iam identity center and chat with role credentials` |
| 3 | root | `chore(webui): bump for the bedrock sso sign-in form` (after the webui push) |
| 4 | webui | `feat(providers): manage inference profile entries and pick one by name` |
| 4 | root | `feat(bedrock-sso): manage inference profile entries and price them from the catalogue` |
| 4 | root | `chore(webui): bump for the inference profile entry editor` (after the webui push) |
| 5 | root | `feat(bedrock-sso): renew role credentials and report an ended session` |
| 6 | root | `feat(bedrock-sso): embed and generate images over the same sso connection` |
| 7 | webui | `feat(providers): show the bedrock sso session state without secrets` (only if it changed) |
| 7 | root | `feat(bedrock-sso): attribute usage, restart affected agents and gate deletion` |
| 8 | root | `docs(configuration): document amazon bedrock sso` |
| 8 | root | `test(catalogue): exclude the sso provider from the no-other-guide assertion` |
| 8 | root | `test(bedrock-sso): cover the sso flow, inference profile entries and pricing` (only if needed) |
