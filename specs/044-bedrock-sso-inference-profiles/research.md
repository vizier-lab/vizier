# Research: AWS Bedrock over SSO with Application Inference Profiles

**Feature**: `044-bedrock-sso-inference-profiles` | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)

Every NEEDS CLARIFICATION the plan's Technical Context could have carried is resolved below. Each decision states what was chosen, why, and what was rejected. Paths are absolute in the workspace `/Users/robikaosar/ngoding/vizier`.

---

## D1 — Provider identity: `amazon-bedrock-sso` / "Amazon Bedrock (SSO)"

**Decision**: `ProviderId::AMAZON_BEDROCK_SSO = "amazon-bedrock-sso"`, display name **"Amazon Bedrock (SSO)"**.

**Rationale**:
- The id grammar is enforced at `crates/vizier-core/src/config/provider.rs:59` (`is_valid`): `[a-z0-9._-]`, 1–64 bytes, first byte alphanumeric. `amazon-bedrock-sso` satisfies it and sorts immediately after `amazon-bedrock` in the `BTreeMap` order the pickers already rely on.
- The spec's own Assumptions name this form as the example ("for example `amazon-bedrock-sso`, shown as \"Amazon Bedrock (SSO)\""), so the choice needs no new argument — it needs confirmation that the code can carry it, which it can.
- Nothing in the codebase parses an id into parts, so a compound id costs nothing. `ProviderId::parse` (`:73`) maps legacy names and otherwise validates a plain string; there is no enum to extend.

**Alternatives considered**:
- `bedrock-sso` — shorter, but drops the vendor prefix that the catalogue, the docs (`docs/src/configuration/providers.md:59`) and the existing id all use; two providers for one vendor with two naming styles is how people paste the wrong one.
- `amazon-bedrock-identity-center` — accurate but 30 characters, and it embeds an AWS product name that AWS itself renames (Identity Center was AWS SSO).
- `bedrock` / reusing `amazon-bedrock` with a mode flag — rejected by FR-001: the spec requires a separate, independently connectable provider with its own state, guide and model list.

---

## D2 — A second id reaching the same catalogue entry: one alias, `BuiltinProvider.model_source`

**Decision**: add one field to `BuiltinProvider` (`crates/vizier-core/src/catalogue/builtin.rs:28`):

```rust
/// The catalogue provider whose models this built-in lists. `None` = its own id.
/// Set only where a provider is a second credential form of an existing catalogue entry.
pub model_source: Option<ProviderId>,
```

and one accessor `BuiltinProvider::model_source_id(&self) -> &ProviderId`. Four call sites use it: `Catalogue::providers` (to list models for the entry), `Catalogue::provider`, `Catalogue::models`, `Catalogue::model` (`crates/vizier-core/src/catalogue/mod.rs:141-179`), plus `resolve_with` (`crates/vizier-core/src/llm/resolve.rs:290`) when it looks up the catalogue provider for a Bedrock dialect.

**Rationale**: the alternative — registering a second `CatalogueProvider` named `amazon-bedrock-sso` — would duplicate 100+ model rows, break the catalogue refresh (which writes only catalogue providers, `catalogue/refresh.rs`), and give two places to keep in sync. An alias is one field with one meaning, and it already has a second use case on the horizon (a `bedrock-eu`-style regional provider), which is the constitution's bar for adding it.

**Alternatives considered**:
- Special-casing `*id == ProviderId::AMAZON_BEDROCK_SSO` inside each of the four functions — four scattered branches, exactly what Principle II forbids.
- Extending `ProviderId` with a `family()` method — a bigger surface for one relationship, and it would invite family matching where FR-001a explicitly wants *id* equality for guides.

**Consequence to test**: `builtin::builtins()` is a `LazyLock<Vec<BuiltinProvider>>`; the new entry must be added there, and `builtin(id)` already does a linear scan, so nothing else changes.

---

## D3 — Session storage: one table, one envelope, one AAD

**Decision**: a new SQLite table, created in the same `init_schema` batch as the rest (`crates/vizier-core/src/storage/sqlite/mod.rs:302` precedent):

```sql
CREATE TABLE IF NOT EXISTS bedrock_sso_session (
    profile TEXT PRIMARY KEY,
    data    TEXT NOT NULL
);
```

`data` holds `serde_json` of `SsoSessionSecret` (access token, refresh token, client id/secret, expiry, region, start URL) **wrapped in the encryption envelope** produced by `SecretCipher::encrypt` with AAD `vizier:bedrock-sso:v1␟session␟<profile>` — the same mechanism `protect_provider_entry` uses today (`crates/vizier-core/src/encryption/fields.rs:104`).

Two new functions mirror that precedent:
```rust
pub fn protect_sso_session(cipher: &SecretCipher, profile: &str, json: &str) -> Result<String>;
pub fn decrypt_sso_session(cipher: &SecretCipher, profile: &str, stored: &str) -> Result<String>;
```
returning `CipherFailure` on a wrong/missing key, which the service maps to the "unreadable session" state (FR-009f).

**Rationale**: the spec's FR-009a demands "written only through the existing encrypted path" — so the session must not get its own cipher, key handling or file format. AAD binds an envelope to its row, so a copied envelope cannot be replayed as another profile (same property `fields.rs` relies on). A separate table, rather than a field on `ProviderCredentials`, keeps the secret out of the struct that every provider response, listing and merge path touches: `ProviderCredentials` is serialised into `provider_config` and is read by `check_fields`, `merged_with`, `is_empty` and three services. Putting an encrypted blob there would work, but it would also make it structurally possible for a generic `Serialize` of `ProviderCredentials` to emit it — the exact class of mistake FR-009b forbids.

**Alternatives considered**:
- A field on `ProviderCredentials` (`sso_session: Option<String>`) — fewer moving parts, but the blast radius is every provider surface, and the "never serialise" rule would rely on a `skip_serializing` attribute surviving future edits.
- A file under the workspace (`.runtime/bedrock-sso/<profile>.json`) — a second storage mechanism beside `DocumentStore` and SQLite, needing its own locking and its own backup story. The constitution prefers the store that already exists.
- Reusing `agent_secret` — wrong owner: the session belongs to a *connection*, not an agent, and `agent_secret` is scoped per agent and per requester.

---

## D4 — The device flow: four calls, one registration reuse

**Decision**: implement the standard OIDC device-authorization flow against the SSO OIDC endpoints plus the portal endpoints, in this order:

| Step | Call | Purpose |
|---|---|---|
| 1 | `POST {oidc}/client/register` (once per workspace, cached) | obtain `client_id`/`client_secret` for the device flow |
| 2 | `POST {oidc}/device_authorization` | `verification_uri`, `user_code`, `device_code`, `interval`, `expires_in` |
| 3 | poll `POST {oidc}/token` at `interval` until `access_token` or a terminal error | complete the sign-in |
| 4 | `GET {portal}/assignment/accounts`, then `GET {portal}/assignment/accounts/{id}/roles` | the accounts and roles the person may pick (FR-007) |
| 5 | `GET {portal}/federation/credentials?account_id=…&role_name=…` | derive the role credentials (FR-013) |

`{oidc}` = `https://oidc.{sso_region}.amazonaws.com`, `{portal}` = `https://portal.sso.{sso_region}.amazonaws.com`. Both are derived from the connection's **SSO region**, never the Bedrock region (Edge Case: the two regions are separate settings).

`client_register` output (`client_id`, `client_secret`, `client_secret_expires_at`) is **secret** and is stored in the same envelope as the session, keyed by profile.

**Rationale**: this is the documented public flow, and it is the only one that needs no AWS SDK crate (spec Assumptions). Registering once per profile — not per sign-in — keeps a re-sign-in to steps 2–3 plus 5, so an expired session asks for a code and nothing else (FR-008).

**Alternatives considered**:
- Drive `aws sso login` — forbidden outright by FR-005 (no CLI, nothing under `~/.aws`).
- Ask the person for a client id/secret — workable fallback, but it makes the connect form require a value AWS itself would have generated. Kept as a documented escape hatch (risk 2 in the plan) if an org blocks runtime registration, not as the default.
- Poll the token endpoint with a fixed sleep — the interval comes from step 2 (`interval`, typically 5 s) and must be honoured; the flow uses it and stops on `authorization_pending`, backs off on `slow_down`, and fails on `expired_token`/`access_denied`.

---

## D5 — Credential caching, renewal, serialization

**Decision**: role credentials live in an in-memory cache **per connection**, held by the `SsoRoleCredential` value that `resolve_bedrock_sso` builds and stores inside `ResolvedModel.credential`:

```rust
pub struct SsoRoleCredential {
    profile: String,
    region: String,          // Bedrock region
    deps: Weak<VizierDependencies>,  // store + cipher, for renewal
    cache: Arc<tokio::sync::Mutex<Option<RoleCredentials>>>,
}
```
- A turn with a valid cache makes **zero** extra calls (FR-013).
- Renewal happens when `expires_at - now < 5 min`: one `CreateToken` (if the access token itself is near expiry) then one `GetRoleCredentials`, both under the per-connection mutex, so concurrent turns collapse into one renewal (FR-014).
- On success the fresh session is written back with a **compare-and-swap** on the stored envelope (read → check `updated_at`/revision → write), so a renewal racing across two processes stays idempotent against the store (Edge Case).
- `CredentialSource::invalidate()` returns `true` once and clears the cache, so the existing 401 retry path (`crates/vizier-core/src/llm/model.rs:141-148`) becomes exactly the "renew once and retry once" of FR-015, with no new retry loop.

**Rationale**: the trait already has `invalidate`, and `complete_once` already retries once after a 401 deleting the token — so FR-015 needs no new machinery, only the right `invalidate` semantics. Keeping the cache on the credential value (not in a global map) means a map keyed by profile is not needed and a deleted connection drops its cache with the last `Arc`.

**Alternatives considered**:
- A global `DashMap<profile, Credentials>` — needs eviction, leaks on delete, and duplicates ownership the `Arc` already gives.
- Persisting role credentials — forbidden by FR-009d.
- Renewing on every turn — violates FR-013's zero-extra-request requirement and adds latency to every message.

---

## D6 — The Test action: one real invoke, `max_tokens: 1`

**Decision**: `service::usage::check` gains an arm for the SSO provider that issues **one `Converse` request** through the normal `llm::build` path with `max_tokens: Some(1)` and the user's `"ping"` message — the same shape the Anthropic arm already uses at `crates/vizier-core/src/service/usage.rs:204-235`.

Preconditions the endpoint enforces: the connection must have a session (else the error is the session error, not a test failure) and the request body must name a model — a catalogue model or a saved ARN. No model named = refused before any call, with the message saying the test needs a target (SC-018).

**Rationale**: FR-012a and SC-018 require the invoke, and the code path already exists (`usage::check` builds a `ResolvedModel` and calls `model.complete`). Reusing it means the test proves exactly what a turn will do, including the ARN path. `max_tokens: 1` keeps the billable part to a single token. **The entry's pricing state is irrelevant to the test** (FR-034a.5): an unpaired or stale-priced entry is tested and invoked exactly like a priced one, and the test neither reads nor reports the price.

**Alternatives considered**:
- `ListFoundationModels` (what the static-credentials provider does at `usage.rs:246-270`) — needs `bedrock:ListFoundationModels`, which FR-012b says must not be required, and it does not prove invoke permission.
- A dry-run/signing-only check — proves nothing about permission; the signature is computed locally.

---

## D7 — Error taxonomy: ten causes, one enum

**Decision**: `crates/vizier-core/src/llm/sso/error.rs`:

```rust
pub enum SsoError {
    SignInDenied,        // access_denied from the token endpoint
    CodeExpired,         // expired_token
    SessionExpired,      // refresh failed / no session
    SessionUnreadable,   // cipher failure (FR-009f)
    RoleNotAssigned,     // account/role missing from the portal
    PermissionMissing { action: String },
    ModelAccessNotEnabled { model: String },
    ProfileNotFound { arn: String },
    RegionMismatch { arn_region: String, connection_region: String },
    Throttled,
    Unavailable,
}
```
Each maps to `LlmError` (transport-level) or `ServiceError` (surface) with **one** message, and `SessionExpired`/`SessionUnreadable` map to a message that names the **session** and points at re-sign-in — never at a key, a region or a model (FR-016).

**Rationale**: FR-041 lists exactly these ten, and SC-015 requires each to read distinctly. One enum is the only way to keep them from collapsing into "provider error". The `PermissionMissing`/`ModelAccessNotEnabled` variants carry the action/model because the provider's own message names them and FR-012a requires it verbatim.

**Alternatives considered**: string matching on AWS messages — brittle and untranslatable; the provider's message is preserved in the error body (FR-012a) *in addition to* the variant, not instead of it.

---

## D8 — Keeping secrets out of every surface

**Decision**:
1. `SsoSessionSecret` derives `Deserialize` only — never `Serialize`. The encrypted blob is produced by explicit `serde_json::to_string` inside `protect_sso_session`, so no generic serialisation of a response type can emit it.
2. The **non-secret** projection `SsoSession` (profile, start URL, both regions, account, role, expiry, `client_secret_expires_at` only as a boolean) is what `Serialize`s to the API.
3. `SsoRoleCredential` implements `Debug` by hand, printing `[redacted]` for both halves and the token (the pattern `SigV4Credential` already uses at `crates/vizier-core/src/llm/auth.rs:151-165`).
4. `LlmError` bodies are truncated to 2 000 bytes (`llm/error.rs:56`) and AWS error shapes carry no credentials — audited once in review.
5. Span fields: `telemetry::chat_span` records provider/model only; the SSO spans add `profile`, `sso_region`, `bedrock_region` and nothing else. A test asserts the string `"access_token"` never appears in a rendered error.

**Rationale**: SC-011 is a 100 %-of-checks criterion, so the guarantee has to be structural (no `Serialize` on the secret type) rather than disciplined (remember not to log it).

---

## D9 — Testing the flow without AWS: a stub server

**Decision**: unit tests spin a tiny HTTP server on `127.0.0.1:0` using `tokio::net::TcpListener` + hand-written request parsing (≈80 lines in `tests_support.rs`), returning canned JSON for `/client/register`, `/device_authorization`, `/token`, `/assignment/accounts`, `/assignment/accounts/{id}/roles` and `/federation/credentials`. The flow under test takes a `base_url` override, which it needs anyway for tests and costs one field.

**Rationale**: no new dev-dependency (`wiremock`/`httpmock` would be a crate for one test module — Principle I). The flow's HTTP client is `reqwest` over a plain URL, so a loopback server is sufficient and deterministic.

**Alternatives considered**: `mockito`/`wiremock` (a new dependency), or trait-mocking the client (an abstraction with one implementation — rejected by Principle I).

---

## D10 — The existing provider's "SSO is unsupported" message

**Decision**: the message at `crates/vizier-core/src/llm/resolve.rs:244-249` gains a pointer: "…set an access key ID and secret access key, or connect **Amazon Bedrock (SSO)** to sign in with IAM Identity Center". Wording only; the refusal stays.

**Rationale**: spec Assumptions require it ("MUST point at this new provider once it ships, instead of leaving the person with no next step"). It is a one-line change and it is the only edit to the existing provider's behaviour.

---

## D11 — ARN parsing: one function, one grammar, AWS's two resource types

**Decision**: `crates/vizier-core/src/llm/bedrock.rs` gains

```rust
pub enum ProfileResource { System, Application }  // inference-profile | application-inference-profile
pub struct InferenceProfileArn { pub partition: String, pub region: String, pub account: String,
                                 pub resource: ProfileResource, pub id: String, pub raw: String }
pub fn parse_inference_profile_arn(raw: &str) -> Result<InferenceProfileArn, ArnError>;
```

accepting exactly `arn:{partition}:bedrock:{region}:{account}:{resource}/{id}` where

| Segment | Rule |
|---|---|
| `partition` | one of `aws`, `aws-cn`, `aws-us-gov`, `aws-iso`, `aws-iso-b` (FR-023) |
| `service` | `bedrock` |
| `region` | `valid_region` (`crates/vizier-core/src/llm/bedrock.rs:67`), whose 2-4 letter prefix and lowercase segments already cover `us-iso-east-1` and `us-isob-east-1` |
| `account` | 12 ASCII digits |
| `resource` | **either** `inference-profile` **or** `application-inference-profile`, compared as a whole segment — never as a prefix, so the application form is never read as the system form with an odd id |
| `id` | non-empty, `[A-Za-z0-9.:-]+`, no `/`, no whitespace (AWS's own charset for the segment) |

Whitespace is trimmed **once** before parsing; a value whose trimmed form differs is accepted (a table copy), while an internal space still fails a segment rule. A console URL fails the literal prefix and a foundation-model ARN fails the resource rule. `resource` is kept on the parsed value because the failure message names the expected form and because the tests assert **both** forms in **all five** partitions (SC-008).

**Rationale**: FR-023/SC-008 name exactly these failure modes and both resource types. The feature exists for the **application** form, so a grammar that accepted only `inference-profile` would refuse exactly the ARNs it was built for — the defect that produced this correction. One parser called from two places (save, and the picker rendering an entry name) keeps the rule in one place (Principle II). The wider grammar stays entirely inside the parser: FR-033a's media ban is untouched, because both forms are chat resources (`contracts/media-and-test.md` §2).

**Alternatives considered**:
- A regex — a workspace dependency already exists, so it would not add a crate; rejected for message quality in a six-segment grammar, not for dependency count.
- Accepting only `application-inference-profile` — narrower than AWS, and it would refuse a system-defined profile a person legitimately bills against; FR-023 requires both on equal footing.
- Splitting on `:` and `/` with no per-segment checks — loses the distinct messages FR-041 needs.

**Region comparison** (FR-024): the ARN's `region` must equal the connection's **Bedrock** region. Checked at save and again at first use (an entry can outlive a region change), reported with both values. The resource type takes no part in it.

---

## D12 — What `tasks` must decompose

The workload is listed in the plan's "Deferred to tasks". Research adds three sequencing constraints:

1. **The session store must land before the flow**, because the flow's tests assert against a real row, not a stub.
2. **The catalogue alias must land before the WebUI**, because the picker lists models through `service::catalogue::models_for`, which needs the alias to return the Bedrock models for the SSO id.
3. **The entry schema must land before the pricing resolver**, because `entry_pricing` reads `ProfileEntry.model` / `.price` and the pricer's entry map is keyed by the entry's ARN (D13).

---

## D13 — Pricing an ARN entry: one optional pairing, one resolver, `Cost` reused

**Decision**:

1. **Two optional, non-secret fields on the entry** (`ProfileEntry`, `crates/vizier-core/src/schema/provider.rs`): `model: Option<String>` — one catalogue Bedrock model id **chosen from a searchable list**, not a typed id — and `price: Option<Cost>` — a hand-entered cost reusing `catalogue::data::Cost` (`:142`: `input`/`output` required, `cache_read`/`cache_write` optional). Both ride inside the existing `credentials.profiles` JSON: no table, no column, no new type (Principle I).
2. **One resolver, one place for the order** — `entry_pricing(catalogue, &ProfileEntry) -> EntryPricing`, beside the model list in `service::catalogue`:

   ```rust
   pub enum EntryPricing {
       Explicit(Cost),                  // FR-034a.4 — the person's figure, used as given
       FromCatalogue(Cost),             // FR-034a.1 / a.3 — the chosen model's cost, re-resolved now
       NotResolved { model: String },   // FR-034a.3 — the id is no longer in the catalogue
       NotSet,                          // FR-034a.2 — nothing was picked
   }
   ```

   Order: explicit → chosen model's cost → `NotResolved` → `NotSet`; nothing is ever derived from the ARN (FR-034a, SC-020). It resolves through the provider alias (D2), so `catalogue.model(&ProviderId::AMAZON_BEDROCK_SSO, id)` reaches the `amazon-bedrock` row: the dropdown and the price read **the same list** the chat picker already offers (Annex A).
3. **One lookup in the pricer.** `usage::Pricer::price` (`crates/vizier-core/src/usage/mod.rs:69`) consults a map `(provider, arn) -> EntryLink { price: Option<Cost>, model: Option<String> }` before its catalogue lookup. The map is built **once per report** from the saved rows (`storage.list_providers()`, `storage/mod.rs:326`), so a turn does no pricing work at all: `Pricer::new` keeps its signature and a `with_entries` constructor is added. The four construction sites all hold storage and are already `async` — `usage::build_report` (`:209`), `usage::conversation_usage` (`:483`), `service::dream::price_cycles` (`dream.rs:207`), `service::overview::counts` (`overview.rs:94`).
4. **Nothing is stored on the usage row.** `usage_event` (`crates/vizier-core/src/storage/sqlite/mod.rs:743`) has no cost column and never had: cost is a report-time computation. That makes "re-resolved each time they are displayed or used" (FR-034a.1) true **by construction** rather than by a sync job, and it means a price entered today also prices turns recorded before it — exactly how catalogue prices already behave.
5. **A stale id is reported, never refused.** The id is validated by shape only ([data-model.md](./data-model.md), "profile entry" → rule 4a), so `NotResolved` can actually arise from the two cases FR-034a.3 names — a withdrawn model, a catalogue not yet refreshed — instead of being pre-empted by a save-time existence check that would conflict with FR-034a.5.
6. **The explicit price cannot be overwritten.** Structurally: `catalogue/refresh.rs` writes catalogue rows only and never touches `provider_config`, so no code path exists that could write a price back into an entry (FR-034a.4).

**Rationale**: FR-034a asks for a selection from a list, priced from the catalogue, with an explicit override and a truthful absence. Reusing `Cost` and the existing `Pricer` turns that into three small edits instead of a pricing subsystem, and going through the D2 alias means the dropdown, the chat picker and the usage price share one source of truth. Putting the lookup in the pricer, not in the resolve path, is how FR-034a.5 is guaranteed rather than promised: no pricing code runs during a turn.

**Alternatives considered**:
- **Write the cost onto `usage_event`** — rejected: a migration, it freezes a figure a refresh could improve, and FR-034a.1 wants re-resolution; it would also make "explicit price wins" true only for later turns.
- **A `bedrock_sso_entry` table** — refused by Principle I: a handful of non-secret rows on one connection already live in `provider_config`, whose `merged_with`/`is_empty` already give the form its semantics.
- **A free-text model id** — refused by the requester ("ga pure manual"); a typed id also invites the silent typo that `NotResolved` would then have to explain.
- **`GetInferenceProfile` / `ListFoundationModels`** — forbidden by FR-034c and by the minimal policy (FR-012b); the only AWS call stays the invoke.

---

## Annex A — Terraform of facts verified against this checkout (not assumptions)

| Fact | Where |
|---|---|
| One Bedrock id exists today; ids are validated strings, not an enum | `crates/vizier-core/src/config/provider.rs:29`, `:59` |
| The guide registry compares the id by **exact equality** | `crates/vizier-core/src/catalogue/guide.rs:47` |
| The guide is attached to the listing, so a second Bedrock id gets `None` today | `crates/vizier-core/src/service/catalogue.rs:217-221` |
| The static provider's minimal policy is `InvokeModel` + `ListFoundationModels` | `crates/vizier-core/src/catalogue/guide.rs:44` |
| Credentials are `SigV4Credential` (access key, secret, optional token, region, service) | `crates/vizier-core/src/llm/auth.rs:114-135`, `:167-215` |
| The Bedrock dialect is pure and shared; the signer is `llm/sigv4.rs` | `crates/vizier-core/src/llm/dialect/bedrock.rs:1-20`, `llm/sigv4.rs:124` |
| Model path encoding percent-encodes an ARN into one segment | `crates/vizier-core/src/llm/bedrock.rs:19-36` |
| Embedding/image families are detected from the model string and refuse anything else | `crates/vizier-core/src/llm/media/bedrock.rs:26-90` |
| Media and chat both build a Bedrock credential in two places today | `crates/vizier-core/src/llm/resolve.rs:270-288`, `llm/media/mod.rs:66-92` |
| Secrets are protected per field with a per-field AAD | `crates/vizier-core/src/encryption/fields.rs:104-160` |
| The key handling fails closed (missing explicit key is an error) | `crates/vizier-core/src/encryption/key.rs:59-137` |
| `provider_config` is `(variant TEXT PRIMARY KEY, data TEXT)` | `crates/vizier-core/src/storage/sqlite/mod.rs:302-305` |
| The 401 retry path is one `invalidate()` + one resend | `crates/vizier-core/src/llm/model.rs:141-148`, `llm/auth.rs:56` |
| The Test action already builds a model and calls `complete` for one dialect | `crates/vizier-core/src/service/usage.rs:199-235` |
| The catalogue test that fails asserts "no other provider has a guide" | `crates/vizier-http/src/api/v1/catalogue.rs:279-315` |
| 034 records SSO as out of scope for that iteration | `specs/034-aws-bedrock-provider/spec.md:253` |
| `Cost` is four fields (input/output required, two cache rates optional) and is shared by the catalogue and every report | `crates/vizier-core/src/catalogue/data.rs:142` |
| The pricer is one cached function per `(provider, model)` per report | `crates/vizier-core/src/usage/mod.rs:37-90` |
| `usage_event` has **no** cost column: cost is a report-time computation | `crates/vizier-core/src/storage/sqlite/mod.rs:743-765` |
| The pricer is built at four production sites, each holding `deps.storage` and already `async` | `usage/mod.rs:209`, `usage/mod.rs:483`, `service/dream.rs:207`, `service/overview.rs:94` |
| A model row already carries `cost: Option<Cost>`, so a priced entry needs no new field in the listing | `crates/vizier-core/src/service/catalogue.rs:275-292` |
| A searchable, windowed, keyboard-driven picker already exists in the design system | `design/src/components/Picker/Picker.tsx` |
| Saved provider rows are one storage call away from the pricer | `crates/vizier-core/src/storage/mod.rs:326` (`list_providers`) |
| No SSO code exists in the checkout | `grep -rn 'device_authorization\|portal.sso\|oidc\.' crates webui/app` → no matches |
