# Implementation Plan: AWS Bedrock over SSO with Application Inference Profiles

**Branch**: `044-bedrock-sso-inference-profiles` | **Date**: 2026-11-02 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/044-bedrock-sso-inference-profiles/spec.md`

## Summary

A **second** Bedrock provider (`amazon-bedrock-sso`, "Amazon Bedrock (SSO)") coexists with the static-credentials one. It authenticates through IAM Identity Center using Vizier's **own** OIDC device-authorization flow (no AWS CLI, nothing under `~/.aws`), keeps the resulting session in **Vizier's own encrypted storage** under a non-secret **profile name**, derives short-lived role credentials in memory only, and lets model selection come from a **managed list of Application Inference Profile ARNs** stored on the connection, each entry optionally **paired with a model picked from the catalogue's Bedrock list** so its usage is priced from models.dev.

Technically this is one credential source behind the existing extension points, not a second Bedrock stack:

- the wire behaviour (Converse dialect, SigV4 signer, model-path encoding, regional endpoint, error translation) is **reused unchanged** (`crates/vizier-core/src/llm/bedrock.rs`, `llm/dialect/bedrock.rs`, `llm/sigv4.rs`);
- a new `CredentialSource` impl (`SsoRoleCredential`) supplies the same `(access_key_id, secret_access_key, session_token)` triple that `SigV4Credential::bedrock` receives today;
- the SSO provider id resolves to the **same catalogue entry** as `amazon-bedrock` (one alias function), so the catalogue's models remain selectable on it (FR-030) and no model list is duplicated;
- an inference-profile ARN is carried as the model string unchanged and encoded into `/model/{arn}/converse` by the existing `model_path` (FR-027);
- an ARN entry's **price** comes from nothing new: the entry stores the id of a model the person chose from the catalogue's own Bedrock list, and the existing `Pricer` resolves that id through the same catalogue alias, so **models.dev is the only price source** and an explicit hand-entered `Cost` wins over it (FR-034a–a.6). **The same pairing fills the entry's context window and output limit** from that row, where the person has not typed them: an act at pairing time, never a live lookup, so a catalogue that later drops the model leaves the limits standing and only the price falls to unpriced (FR-034a.7).

## Technical Context

**Language/Version**: Rust 2024 edition (workspace `0.14.1`, `resolver = "3"`); TypeScript 5 / React 19 for the `webui/` submodule. No new crate: the device flow, the portal calls and the renewal are `reqwest` + `serde_json` + the existing signer.

**Primary Dependencies**: `reqwest` (device flow, SSO portal, Bedrock runtime — the same client `VizierDependencies.http` holds), `serde`/`serde_json` (session row, ARN parse, request bodies), `chrono` (expiry), `rusqlite` (session table), and the existing `encryption::SecretCipher`. Nothing from `aws-sdk-*` or `aws-config` (spec Assumptions).

**Storage**: one new SQLite table `bedrock_sso_session` (`profile TEXT PRIMARY KEY, data TEXT NOT NULL`), where `data` is the encryption envelope of the session secret; the connection's non-secret half (start URL, both regions, account, role, profile name, the ARN entry list) rides in the existing `provider_config` JSON row for `amazon-bedrock-sso`, exactly like the static-credentials provider's fields do. Role credentials are **never** persisted (FR-009d).

**Testing**: `cargo test -p vizier-core --lib` for the device-flow state machine (against a local stub server bound on `127.0.0.1:0`, research D9), ARN parsing (both resource types, all five partitions), `entry_pricing` (all five outcomes — explicit, from-catalogue, not-resolved, not-set and the cold-start no-catalogue case — plus the explicit-price override and the required reason on every absence), session storage/encryption, credential caching and the two-credentials-per-connection resolver; `cargo test -p vizier-http` for the API surface and the corrected catalogue test. WebUI: `cd webui && npm run typecheck && npm test`. End-to-end: dummyplug steps in [quickstart.md](./quickstart.md) — the provider surface is agent-observable (a turn succeeds or fails on the credential source), so the constitution's e2e gate applies; the **live** SSO/Bedrock path is a volunteer release checklist, not CI.

**Target Platform**: the same matrix as today (Linux/macOS/Windows servers, Tauri desktop, embedded WebUI). The device flow needs an outbound HTTPS client only; no keychain, no file under `$HOME/.aws`, no OS-specific path.

**Project Type**: Rust library + binary workspace (`vizier-core` → `vizier-http` → `vizier`) plus the React `webui/` and `design/` submodules.

**Performance Goals**: sign-in is interactive (browser confirmation dominates); an account/role listing is two portal GETs; a renewal is one `CreateToken` + one `GetRoleCredentials`, serialized per connection, and must be **zero** extra requests per turn while the cached credentials are valid (FR-013). No change to turn latency otherwise: the credential is already an `Arc<dyn CredentialSource>` resolved at build time.

**Constraints**: the stored session is written only through the existing encrypted path with a per-field AAD (FR-009a); no session, token, device code or role credential in any response, log, error or span (FR-009b, FR-035); renewal must stay idempotent against the store even with two processes on one workspace (Edge Case; FR-014 is per process); the existing provider must not migrate, restart or be revalidated (FR-003, FR-004).

**Scale/Scope**: one row per SSO connection; single-digit connections per workspace; a handful of ARN entries each. Roughly **31–37 files** touched across `crates/vizier-core`, `crates/vizier-http`, `webui/` and `docs/` (no `design/` change expected: the searchable picker the dropdown needs already exists).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Assessment | Evidence |
|---|---|---|
| **I. Lean by Default** | **Pass, with one new field justified.** No crate, no generic, no config key: one SQLite table, one `CredentialSource` impl, one ARN parser, one `BuiltinProvider.model_source` field (this feature is its second real use case — before it, the field would have had exactly one value for every entry), **two optional fields on an existing JSON struct** and **one `Cost` it already owns** reused for the explicit price. No new pricing subsystem and no `aws-sdk-*`: the price is looked up in the pricer that already exists. | `crates/vizier-core/src/catalogue/builtin.rs:28` (`BuiltinProvider`), research D2/D3 |
| **II. DRY via Trait-Based Extensibility (NON-NEGOTIABLE)** | **Pass by construction.** The new provider is *not* a `match` over a provider id in the turn path: it is a new `CredentialSource` impl registered where credentials are built (`llm/resolve.rs`, `llm/media/mod.rs`). Signer, dialect and model-path code is untouched. The one id-keyed lookup that must exist (catalogue alias, guide, media capability list) is a single function per surface, not a scattered branch. | `llm/auth.rs:27` (`CredentialSource`), `llm/resolve.rs:270` (`resolve_bedrock`), `llm/media/mod.rs:63` (`endpoint`) |
| **III. Self-Contained, Zero-Dependency Runtime** | **Pass.** Nothing new runs as a service: the session is a row in the bundled SQLite, the flow is outbound HTTPS on demand, and startup makes no AWS call (FR-037). A workspace with no SSO connection behaves exactly as today. | `dependencies.rs:130` (cipher from the workspace key), `storage/sqlite/mod.rs:302` (`provider_config` precedent) |
| **IV. Portability by Default** | **Pass.** No path handling beyond existing workspace paths; the session lives in SQLite, so `DocumentStore::resolve` is untouched and Windows needs no special case. Nothing shells out, so no `cfg` branch. | `storage/document/mod.rs` unchanged |
| **V. Unified Errors & Observability** | **Pass, needs care.** Every new failure becomes `VizierError` at the boundary and a distinct `LlmError`/`ServiceError` at the surface, so the ten act-on-able cases of FR-041 stay distinguishable; all logging goes through `tracing`, and every new span field is audited for secrets (FR-035, SC-011). | research D7 (error taxonomy), D8 (redaction) |
| **Gate: conventional commits** | **Pass.** `type(scope): subject` — `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`, scoped to the feature where it helps (`feat(bedrock-sso):`, `test(catalogue):`); no `[Spec Kit] …` prefix (the constitution forbids it explicitly). The owner (2026-11-02) made this a hard gate: a commit that is not conventional is **rejected and must not be pushed**. Applies to every spec-kit auto-commit and to each implementation phase. | `.specify/memory/constitution.md:151`; owner instruction 2026-11-02 |
| **Gate: matching submodule branches** | **Pass.** Branch `044-bedrock-sso-inference-profiles` exists in the root and in all four submodules — root `2fa1eee`, design `4e769d3`, landing `429e229`, webui `1311f06`, dummyplug `9a49709` — so `webui/` (which this feature edits) already carries it. Each HEAD currently equals that submodule's `origin/master`: the branches exist locally with no commit ahead yet. The owner's git permission of 2026-11-02 covers spec-kit operations (feature branch, per-phase commits); and since 2026-11-02 **commit and push are both allowed** within that scope — so a submodule commit can now reach the remote and the root may bump its pin, in the order `just push` already enforces: submodules first, then the workspace. | `git rev-parse --abbrev-ref HEAD` in the root and in `design/`, `landing/`, `webui/`, `dummyplug/`; `git rev-parse --short HEAD` vs `origin/master` in each |
| **Gate: manual verification for runtime-affecting change** | **Pass by design.** A new storage table is a startup path, so quickstart §1 runs the real binary against a fresh workspace and then against a second start (the session survives, FR-009/SC-017a). | [quickstart.md](./quickstart.md) §1–§2 |
| **Gate: dummyplug e2e** | **Pass by design.** The provider's failure modes (session ended, ARN named for embedding) are scripted as dummyplug steps with an expected error text, not assumed from unit tests. | [quickstart.md](./quickstart.md) §3 |
| **Gate: no secret in output** | **Pass by design; must be proven.** SC-011 is a checklist item in quickstart (grep the workspace DB, the log, the API bodies) rather than a code comment. | [quickstart.md](./quickstart.md) §4 |

**Gate result: no blocking items.** Everything else passes without a Complexity Tracking entry.

### Re-evaluated after Phase 1 design

| Concern | Verdict |
|---|---|
| **I. Lean** | Holds. Phase 1 added no abstraction beyond the three already justified (session storage trait, credential source, ARN parser). The entry list reuses the `provider_config` JSON shape instead of opening a second table for a handful of non-secret rows. |
| **II. DRY** | Strengthened. The media path (`llm/media/mod.rs`) and the chat path (`llm/resolve.rs`) both call **one** new helper `bedrock_role_credentials(deps, id, profile)`, so the static pair and the SSO session differ in exactly one place and nothing is copy-pasted (FR-002). |
| **III. Self-contained** | Holds. `contracts/bedrock-sso-wire.md` adds no external dependency; the session row lives inside the existing database file. |
| **IV. Portability** | Holds. No new filesystem path, no process invocation. |
| **V. Errors** | Holds and is pinned: `contracts/provider-credentials-api.md` fixes response shapes and error strings, so FR-041 is a contract rather than prose. |
| **New risk found in design** | A **second** `ProviderId` now maps to one catalogue entry, so `Catalogue::models`/`provider`/`model` and `resolve_with` each need the alias. Design keeps it to one function (`builtin::model_source`) called from those four places; a missed call site shows up as "provider not listed", which quickstart §3 catches. Recorded, not hidden. |
| **Post-plan sync: pricing (FR-034a–a.6)** | Holds, and cheaply. Two optional fields on a JSON struct that already exists, one resolver function beside the model list, and one lookup added to the `Pricer` that already prices every report. No table, no migration, no new dependency, and no code in the turn path — which is also what makes FR-034a.5 true by construction rather than by discipline. The dropdown reuses the design system's existing searchable `Picker`, so `design/` is untouched. |
| **Post-plan sync: the limit fill (FR-034a.7)** | Holds, and stays out of the turn path. One write-time function beside `validate_entries`, reading the catalogue row `entry_pricing` already reads (`Catalogue::model` → `limit.context`/`limit.output`): no new data source, no AWS call (FR-034c), no migration (`context`/`output` already exist), and no code on the read or turn path — so FR-034a.5 holds by construction, exactly as it does for the price. What it does cost is one small persisted field, `limits_edited` (two bits, serde-defaulted, server-owned), because rule (1) — a typed number is never overwritten, not even by a client echoing back what it displayed — cannot be made exact by comparing values alone. That is the option the spec's checklist named ("the existing `edited` flag, per-field provenance, or not accepting unchanged values back") and it is the only one that keeps rule (2) true as well. |
| **Post-plan sync: ARN grammar (FR-023)** | Holds. The correction widens one match arm inside the single parser (D11); it adds a `ProfileResource` marker to the parsed value and four partitions to a list. Nothing outside `llm/bedrock.rs` learns about the resource type, and FR-033a's media ban is stated in terms of "an ARN", so it covers both forms without a second check. |
| **Test that must change** | `crates/vizier-http/src/api/v1/catalogue.rs:312` asserts *no other provider has a guide*. With a second Bedrock provider that assertion is false by design; it is corrected to exclude the SSO id, not weakened (it still proves every non-Bedrock provider has none). |

## Project Structure

### Documentation (this feature)

```text
specs/044-bedrock-sso-inference-profiles/
├── plan.md              # this file
├── research.md          # Phase 0: decisions D1–D13
├── data-model.md        # Phase 1: entities, tables, state machine
├── quickstart.md        # Phase 1: offline dummyplug checks + live volunteer checklist
├── contracts/
│   ├── bedrock-sso-wire.md          # device flow, portal calls, renewal, ARN grammar
│   ├── provider-credentials-api.md  # HTTP surface: fields, responses, errors
│   └── media-and-test.md            # Test action (invoke) and the ARN refusal for embedding/image
├── spec.md
└── checklists/requirements.md
```

`tasks.md` is **not** created by this command; it is the next phase (`speckit_tasks`).

### Source Code (repository root)

New files are marked `+`; everything else exists today and is edited.

```text
crates/vizier-core/src/
├── config/provider.rs                       # + ProviderId::AMAZON_BEDROCK_SSO
├── catalogue/
│   ├── builtin.rs                           # + sso_bedrock() entry, + model_source field
│   ├── guide.rs                             # + bedrock_sso() guide, per-id dispatch (FR-001a, FR-038)
│   └── mod.rs                               # alias through model_source in provider/models/model
├── llm/
│   ├── auth.rs                              # + SsoRoleCredential (CredentialSource impl)
│   ├── bedrock.rs                           # + parse_inference_profile_arn + ArnError
│   ├── resolve.rs                           # + resolve_bedrock_sso, + bedrock_role_credentials
│   ├── sso/mod.rs                     (+)   # module root, re-exports, the flow's entry points
│   ├── sso/client.rs                  (+)   # register / device-authorization / create-token
│   ├── sso/portal.rs                  (+)   # list accounts, list roles, get-role-credentials
│   ├── sso/session.rs                 (+)   # stored session, expiry, compare-and-swap renewal
│   └── sso/error.rs                   (+)   # the ten distinct failures (FR-041)
├── llm/media/
│   ├── mod.rs                               # endpoint() uses the shared credential helper
│   └── bedrock.rs                           # + ARN refusal before the family check (FR-033a)
├── encryption/fields.rs                     # + protect_sso_session / decrypt_sso_session
├── schema/
│   ├── provider.rs                          # + SSO credential fields, + ProfileEntry (arn, name, limits, + model, + price)
│   └── sso.rs                         (+)   # SsoSession (non-secret) + SsoSessionSecret
├── storage/
│   ├── sso_session.rs                 (+)   # SsoSessionStorage trait
│   ├── sqlite/sso_session.rs          (+)   # rusqlite impl, one row per profile
│   └── sqlite/mod.rs                        # + CREATE TABLE bedrock_sso_session
└── service/
    ├── providers.rs                         # + SSO fields on upsert/response, session state
    ├── providers_sso.rs               (+)   # sign-in start/complete, accounts, roles, sign-out, entries
    ├── usage.rs                             # check() gains the invoke-based test for the SSO id
    └── catalogue.rs                         # listing synthesised from the bedrock entry, + entry_pricing (all five outcomes, each absence carrying its reason)

crates/vizier-core/src/usage/mod.rs          # Pricer::with_entries + the per-report entry map (price + pairing) looked up before the catalogue
crates/vizier-core/src/service/dream.rs      # price_cycles builds the pricer with the entries
crates/vizier-core/src/service/overview.rs   # counts builds the pricer with the entries

crates/vizier-http/src/api/v1/
├── providers/mod.rs                         # + /{id}/sso/{sign-in,accounts,roles,sign-out}, + /{id}/models entries
└── catalogue.rs                             # corrected guide assertion (:312)

webui/app/
├── api/endpoints/providers.ts                # + CredentialField members, SSO types, sign-in calls
├── lib/providers.ts                          # + FIELD_LABELS / PLAIN_FIELDS for the new fields
└── screens/providers/
    ├── ConnectForm.tsx                       # sign-in button + code/URL panel (generic, no id branch)
    ├── ModelsDrawer.tsx                      # + add/edit/remove an ARN entry, its pricing state and its dropdown
    └── ProfileEntryForm.tsx            (+)   # one entry: ARN, name, limits, the model Picker (searchable), an explicit price

webui/app/api/schema.d.ts                   # regenerated by `npm run openapi` (`scripts/openapi.mjs`, openapi-typescript) — not hand-edited

docs/src/configuration/providers.md           # new "Amazon Bedrock (SSO)" section (FR-039)
```

**Structure decision**: follow the existing crate split — behaviour in `vizier-core`, HTTP surface in `vizier-http`, UI in the `webui` submodule. The SSO flow lives under `llm/sso/` because everything it produces is a credential for `llm`; it is *not* a new top-level module, since nothing outside `llm` needs the device flow.

## Phase 0 — Outline & Research

Every item the Technical Context would have marked NEEDS CLARIFICATION is resolved in [research.md](./research.md):

| # | Question | Decision |
|---|---|---|
| D1 | Provider id and display name (the spec leaves these to planning) | `amazon-bedrock-sso`, **"Amazon Bedrock (SSO)"** |
| D2 | How a second id reaches the same catalogue models | `BuiltinProvider.model_source` alias, one function |
| D3 | Where the session is stored and how it stays unreadable | new `bedrock_sso_session` table + `SecretCipher` envelope with a per-field AAD |
| D4 | Which HTTP calls the device flow makes, and in what order | register → device authorization → poll token → portal accounts/roles → get-role-credentials |
| D5 | How role credentials are cached, renewed and serialized | in-memory per-connection cache + `tokio::sync::Mutex`, renew 5 min before expiry, CAS write-back |
| D6 | How the Test action proves the right permission | one `Converse` invoke of the selected model/ARN, `max_tokens: 1` |
| D7 | The error taxonomy for FR-041 | one enum, one message per cause, mapped to `LlmError`/`ServiceError` |
| D8 | How secrets are kept out of every surface | redaction in `Debug`, no session field on any `Serialize` struct, span fields audited |
| D9 | How the device flow is tested without AWS | a local stub server on `127.0.0.1:0`, driven by the flow's own HTTP client |
| D10 | What happens to the existing provider's "SSO is unsupported" message | it names the new provider (spec Assumptions) |
| D11 | How an ARN is validated, and which forms are accepted | one parser in `llm/bedrock.rs`: **both** resource types (`inference-profile`, `application-inference-profile`), five partitions, one message per segment |
| D13 | How an ARN entry is priced, and in what order | `ProfileEntry.price` (`Cost`) first — it wins outright, no lookup; then a catalogue carrying **no Bedrock list** ⇒ **`NoCatalogue`**, decided *before* `model` is read so it beats both absences below; then `ProfileEntry.model` (chosen from the catalogue list) → its catalogue `Cost`, re-resolved per report ⇒ `FromCatalogue`; a chosen id no longer in the list ⇒ `NotResolved { model }`; no model ⇒ `NotSet`. **Five outcomes, each a distinct state carrying its own non-empty `reason` that names the cause and the way out** — FR-034b's three absences stay identifiable from the reason alone, never a bare label — and **no control-plane call and no extra permission on any of the five paths** (FR-034c): the list and the rates come from Vizier's own catalogue, never from AWS |
| D12 | What `tasks` must decompose | the dependency-ordered split in "Deferred to tasks" |

## Phase 1 — Design & Contracts (outputs)

- [data-model.md](./data-model.md) — the session row, the connection fields, `ProfileEntry` **including its pricing pairing, the five-outcome price table and the precedence between its rows**, the credential cache, the state machine (unconfigured → signed-in → expired → unreadable) and every validation rule with its requirement.
- [contracts/bedrock-sso-wire.md](./contracts/bedrock-sso-wire.md) — the device flow step by step, the portal calls, renewal, and the ARN grammar.
- [contracts/provider-credentials-api.md](./contracts/provider-credentials-api.md) — the HTTP surface: new credential fields, **the entry's `model`/`price` fields and its computed `pricing` flag**, new endpoints, which response fields may and may not carry secrets, and the error strings.
- [contracts/media-and-test.md](./contracts/media-and-test.md) — the Test action and the ARN refusal for embedding/image (FR-012a, FR-033a/b).
- [quickstart.md](./quickstart.md) — offline dummyplug verification, the restart check, the five pricing scenarios (paired / unpaired / stale id / explicit price / catalogue not loaded), the secret-leak grep, and the live volunteer script.

## Provider identity (the one item the spec delegated to planning)

- **Provider id**: `amazon-bedrock-sso` (`ProviderId::AMAZON_BEDROCK_SSO`).
  - Matches the id grammar the code enforces (`crates/vizier-core/src/config/provider.rs:67`: 1–64 chars of `[a-z0-9._-]`, first character alphanumeric) and sorts next to `amazon-bedrock`, so the two appear together in every picker.
  - Built from the existing id plus the identity method, which is the form the spec itself names as the example.
  - Not `bedrock-sso`: `amazon-bedrock` is the vendor-qualified name the catalogue and the docs already use, and a second provider that drops the vendor prefix would read like a different product.
  - Not `amazon-bedrock-identity-center`: accurate but long, and it would put an AWS product name in an id people type into config.
- **Display name**: **"Amazon Bedrock (SSO)"** — the existing row is "Amazon Bedrock"; the parenthesised qualifier is the shortest form that distinguishes them in a list sorted by name and in a `<select>`, and it names the thing the person chose (SSO) rather than the mechanism (Identity Center).
- Both are compile-time constants, not config: a settings-driven id would break every stored row, the guide registry and the media capability list.

## The limit fill (FR-034a.7) — what planning had to decide

The spec left three things open and forbade a fourth: whether the fill writes the entry's own `context`/`output` or a snapshot beside them, how "the person edited this" survives a round trip, and that no **new** persisted field should be needed — while rule (3) already rules out re-reading the catalogue on every display. Decided as follows.

- **Into the entry's own fields.** No snapshot, no shadow copy: `context` and `output` are written once, when the pair is set, and are the entry's numbers from then on (rule 3 — an act, not a live lookup).
- **On the write paths only.** One function in `service/providers_sso.rs` beside `validate_entries`, run by every place a list is written: `add_entry`, `update_entry`, the surviving list in `remove_entry`, and the whole-list write `PUT /providers/{id}` (`service/providers.rs:655`, which already calls `validate_entries`). Never on a read, never in the turn path.
- **Provenance is the server's, and it is two bits.** `ProfileEntry.limits_edited: Option<LimitsEdited { context, output }>` — serde-defaulted so every stored row still deserialises, **ignored on input**, and recomputed on each write from the stored entry plus the submitted value: a limit is the person's when the submitted number differs from the stored one, or when it arrived on an add. A client echoing back exactly what it displayed therefore changes nothing, which is the rule (1) hazard the checklist flagged as a "round-trip hazard"; a value the person edits is marked and stays theirs through any later re-pair. An entry saved before this rule (`None`) reads as "any number it holds is the person's" — no migration, and no old number is adopted as a fill by accident.
- **What the fill does, in one sentence.** When the pair is set or changed, every limit the person has not edited becomes the new model's `limit.context` / `limit.output` — *including empty* when the row carries no figure or the id does not resolve (rules 2 and 4). With the pair unchanged, it fills only a still-empty, unedited field, which is rule (4)'s cold start (the seed had not resolved the id when the pair was made). Removing the pair leaves the limits as they are (rule 3: a window that was resolved does not stop being true), and a field the person cleared stays cleared (rule 5).
- **Capabilities stay unfilled.** `tool_call` and `image_input` remain per-entry fields under FR-028; extending the fill to them is a spec change (spec.md's revision note carries the one-clause version), not something this plan does quietly.

**What this does not do.** It does not touch ARN validation (FR-023) or the region check (FR-024) — the function never sees the parsed ARN, only the `model` id and the entry's numbers — and it cannot refuse, delay or disable a turn (FR-034a.5). The **reference case** is in the embedded catalogue and is the one SC-022 measures: `global.xai.grok-4.7` and `us.xai.grok-4.7` both carry `{"context": 500000, "output": 500000}` (verified in `crates/vizier-core/assets/catalogue/api.json`, 203 Bedrock models), so a single selection fills both fields at 500 000 with no number typed, while the two rows' different `cost` (2/6 vs 2.2/6.6 per million) still decides the price.

## Risks and open questions for the owner

1. **Submodule branch — RESOLVED 2026-11-02 (owner's permission); not an open risk.** The owner's git permission of 2026-11-02 covers spec-kit operations, and `044-bedrock-sso-inference-profiles` exists in the root and in all four submodules. Extended the same day: **commit and push are permitted inside that scope**, commit messages MUST be conventional commits, and the repo's own `just push` recipe orders it safely (submodules first, then the workspace, so a pin never points at an unpushed commit). What survives is a discipline, not a blocker: never bump a submodule pin to a commit that has not been pushed, and never use a free-form message — an unconventional commit is rejected, not pushed.
2. **Portal endpoints versus a real Identity Center instance.** The flow is implemented against the documented requests; only a volunteer with an Identity Center instance can confirm them end to end. If the org forbids runtime client registration, the design needs a pre-registered client id/secret — a policy question, not a code one.
3. **Two processes on one workspace.** Renewal is idempotent against the store by compare-and-swap, but the *cached* credentials differ per process, so a sign-out in one process reaches the other on its next request. This is the intended reading of FR-009c; flagged because it is observable.
4. **Usage/cost for an ARN — settled by the dropdown; two consequences worth stating.** The price is the chosen model's catalogue cost, or the hand-entered figure. Where there is neither — no model paired, a paired id gone from the catalogue, or a catalogue with no Bedrock list at all — the entry is **not** left with a bare `unpriced` label: `entry_pricing` returns one of **five** outcomes (`Explicit`, `FromCatalogue`, `NotResolved { model }`, `NotSet`, `NoCatalogue`) and every absence carries **its own non-empty reason naming the cause and the way out**, because an absence the person cannot read is not a stated fact (FR-034a.2, FR-034a.3, **FR-034b**, SC-021) — including `NoCatalogue`, which outranks the other two and says nothing can be picked yet. Nothing on this path reaches AWS: the list and the rates both come from Vizier's own catalogue, so **FR-034c** holds by construction — no control-plane call and no extra IAM action. Consequences to accept rather than discover: (a) an SSO profile billed at a negotiated rate reads as the catalogue's list rate unless someone enters the price by hand — the escape hatch exists, nothing prompts for it; (b) because no cost is stored on the usage row, a historical report is priced with **today's** catalogue and today's pairing, so a price entered now also reprices past turns — exactly how catalogue prices already behave. Both follow from pricing at report time; named here so the release notes can say so.
5. **What a hand-entered price is a price *of* affects nothing in the plan, but the form must say so.** The explicit field reuses `catalogue::data::Cost` (per-million input/output rates), not a dollar figure per turn, so a person who types a per-turn number would be off by orders of magnitude. The form labels the unit and the API rejects a negative value; nothing else guards it. Named because it is the one place in this feature where a wrong number is *accepted* rather than refused.
6. **Deleting a connection that still has a live session.** Deletion removes the session row, so a person who deletes and re-adds must sign in again. Stated so it is a decision, not a surprise.
7. **A filled limit is the paired model's number, not a fact about the profile.** The fill is exact for what the catalogue holds, and the person can correct either field by hand — from then on the correction wins and no fill, re-pair or refresh touches it (FR-034a.7 rule 1, SC-022). Consequences to accept: where a profile's real window is smaller than its foundation model's, an entry that was never corrected reports the larger figure, so an over-long turn is refused by the provider rather than by Vizier's checkpoint (the provider's own error is surfaced and named, FR-031); and an entry whose limits were filled keeps them even after the pair is removed, because a resolved window does not stop being true (rule 3).
## Complexity Tracking

No constitution violation needs justification. The only additions with a surface beyond one call site are the session storage trait and the catalogue alias field, both justified above (D2, D3, D4).

## Deferred to `tasks`

The plan stops here. `speckit_tasks` decomposes, in dependency order:

1. Foundation: `ProviderId`, `ProfileEntry`/credential schema, ARN parser, session table + storage trait + encryption fields.
2. Device flow: `llm/sso/` (client, portal, session, error) with its stub-server tests.
3. Credential source: `SsoRoleCredential`, the shared `bedrock_role_credentials` helper, chat + media wiring.
4. Surface: catalogue alias + guide, `service/providers_sso.rs`, `service/providers.rs`, `service/usage.rs` test action, HTTP routes.
5. Model list: entry CRUD, picker, ARN refusal on media, limits resolution.
5a. Entry pricing: `ProfileEntry.model`/`.price` shape rules, `entry_pricing` (five outcomes), the pricer's per-report entry map and its four call sites, the entry's computed `pricing` flag **and its required reason** in the API row (SC-021: an absence must explain itself, never a bare label); and the **limit fill of FR-034a.7** — one write-time function, two provenance bits and the form's hint, with SC-022's reference case as its test.
6. WebUI: fields, sign-in panel, entries editor.
7. Docs + agent context: the new `providers.md` section, the failing catalogue test correction.
8. Verification: quickstart scripts as dummyplug steps, the five pricing scenarios, secret-leak grep, restart check.

Estimated size: **large** — roughly **31–37 files**, one new table, one new UI panel and a multi-step interactive flow. The pricing half of the sync did not open a new phase: it adds one step inside "Model list" (5a), two optional fields, one resolver and one pricer lookup, and one more WebUI component. What it did remove is a risk rather than a phase — the old "ARN is always unpriced" note is gone. Expect the implementation phase to be the longest of the four features in flight, and to need a live Identity Center instance only at the very end.
