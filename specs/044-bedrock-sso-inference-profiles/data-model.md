# Data Model: AWS Bedrock over SSO with Application Inference Profiles

**Feature**: `044-bedrock-sso-inference-profiles` | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)

Everything here is derived from the spec's Key Entities and Functional Requirements; each rule names the requirement it satisfies. Nothing is migrated: the existing `amazon-bedrock` row, its `provider_config` JSON and its agents are untouched (FR-004).

---

## Where things live after this change

| Thing | Store | Shape | Requirement |
|---|---|---|---|
| Connection — non-secret half | `provider_config` row, `variant = 'amazon-bedrock-sso'` | `ProviderEntry { id, credentials }` JSON, same as today's Bedrock row | FR-001, FR-008 |
| Connection — session secret | `bedrock_sso_session` row, `profile = <profile name>` | one `SecretCipher` envelope over `SsoSessionSecret` JSON | FR-009, FR-009a, FR-009b |
| ARN entries, incl. the optional pricing pairing (`model`, `price`) | inside the connection's `credentials.profiles` | `Vec<ProfileEntry>`, non-secret | FR-020, FR-021, FR-032, FR-034a |
| Role credentials | process memory only, held by the credential value | `RoleCredentials { access_key_id, secret_access_key, session_token, expires_at }` | FR-009d, FR-013 |
| Usage rows | existing `usage_event` | `provider = 'amazon-bedrock-sso'`, `model` = the ARN unchanged; **no cost column — the price is computed at report time** | FR-034, FR-034a |

No new file on disk. Nothing under `~/.aws` is read or written (FR-005, SC-017b).

**No new facet, table or column for pricing.** An ARN entry's price is derived, never stored: the entry keeps an optional model id and an optional `Cost` inside the JSON it already has, and every report resolves the price from those two fields against the catalogue as it stands at that moment (research D13). `usage_event` keeps the shape it has today, which is why the feature needs no migration at all.

---

## Entity: SSO connection (`amazon-bedrock-sso`)

Stored in `provider_config` as a `ProviderEntry` whose `credentials` gains fields. Written through the existing `upsert_provider`, so it is read back through `decrypt_provider_entry` unchanged.

| Field | Type | Secret | Notes |
|---|---|---|---|
| `start_url` | `Option<String>` | no | Identity Center portal URL chosen by the person; must be `https://` with a host (FR-005) |
| `sso_region` | `Option<String>` | no | Region of the Identity Center instance; validated with `bedrock::valid_region` (FR-010, FR-011) |
| `region` | `Option<String>` | no | **Bedrock** region; the existing field name, unchanged (FR-010) |
| `account_id` | `Option<String>` | no | 12 digits, chosen from the portal (FR-007) |
| `role_name` | `Option<String>` | no | chosen from the portal (FR-007) |
| `sso_profile` | `Option<String>` | no | the **profile name**: the non-secret handle (FR-009e). 1–64 chars, `[A-Za-z0-9._ -]`, unique per workspace |
| `profiles` | `Vec<ProfileEntry>` | no | the inference-profile entries (below), each carrying its optional pricing pairing |

Validation rules (all enforced in `service::providers::check_fields` + a new `check_sso_connection`):

- `start_url`, `sso_region`, `account_id`, `role_name`, `sso_profile` present ⇒ a connection may be saved only when all five are (`400`, one message naming the missing field). A connection may be saved **without** a completed sign-in, so the form can be filled in stages (FR-012).
- `sso_region` defaults from `region` and vice versa when only one is supplied; the two are never silently equalised afterwards (FR-010).
- `sso_profile` is unique: a second connection reusing it is refused naming the existing one (FR-009e).
- `base_url` is refused (existing rule, `check_fields`: a cloud provider's address comes from the catalogue).
- `api_key` is refused: Bedrock API keys are not a credential form here (existing behaviour, kept).

**Why the fields ride on `ProviderCredentials`** rather than a new table: they are non-secret, they belong to exactly one provider row, and `merged_with`/`is_empty` already give the "omitted keeps, empty clears" semantics the form needs (FR-021 "one operation"). The *only* secret is the session, and it lives elsewhere (D3).

---

## Entity: SSO session secret (`bedrock_sso_session.data`, encrypted)

`Serialize` is **deliberately not derived** (D8).

| Field | Type | Notes |
|---|---|---|
| `access_token` | `String` | from the token endpoint; never leaves the process except to the portal |
| `refresh_token` | `Option<String>` | present when the flow returns one |
| `token_expires_at` | `DateTime<Utc>` | access-token expiry |
| `client_id` | `String` | from the one-time client registration |
| `client_secret` | `String` | from the one-time client registration |
| `client_secret_expires_at` | `Option<DateTime<Utc>>` | re-register when past |
| `issued_at` | `DateTime<Utc>` | for diagnostics and for the CAS write |
| `revision` | `u64` | incremented on every write; the compare-and-swap key for two-process renewal |

AAD: `vizier:bedrock-sso:v1␟session␟<profile>`. Binding the profile into the AAD means an envelope copied to another row fails to decrypt (same property as `encryption/fields.rs`).

**Failure modes** (all → `SsoError::SessionUnreadable`, FR-009f):
- `CipherFailure::KeyMissing` / `KeyMismatch` → message names the profile and offers a re-sign-in.
- Row absent while the connection says "signed in" → `SessionExpired` (the profile's half is missing; FR Edge Case "profile name and stored session disagree").
- Envelope present but `key_id` unknown → `SessionUnreadable`.

Nothing is deleted in any of these cases (SC-019).

---

## Entity: role credentials (memory only)

| Field | Type | Notes |
|---|---|---|
| `access_key_id` | `String` | from `GetRoleCredentials` |
| `secret_access_key` | `String` | never written, never logged |
| `session_token` | `String` | always present for role credentials |
| `expires_at` | `DateTime<Utc>` | renewal threshold: 5 minutes before |

Not a stored entity: it is the `Option<RoleCredentials>` inside `SsoRoleCredential`'s mutex (D5). It is dropped with the credential value.

---

## Entity: profile entry (Application Inference Profile)

`ProfileEntry`, serialised inside the connection's `credentials.profiles`:

| Field | Type | Required | Notes |
|---|---|---|---|
| `arn` | `String` | yes | the raw ARN as saved; sent unchanged (FR-027) |
| `name` | `Option<String>` | no | display name; empty ⇒ shown as the profile id from the ARN (FR-026) |
| `context` | `Option<u64>` | no | context window; empty ⇒ unknown (FR-028) |
| `output` | `Option<u64>` | no | output limit; empty ⇒ unknown |
| `tool_call` | `Option<bool>` | no | empty ⇒ unknown (FR-028) |
| `image_input` | `Option<bool>` | no | empty ⇒ unknown |
| `model` | `Option<String>` | no | **the pricing pairing**: one catalogue Bedrock model id, chosen from the searchable list (FR-034a, FR-034a.1). Empty ⇒ no price from the catalogue; the entry chats unpriced and shows "pricing not set" (FR-034a.2). Stored as an id, never as a name or a price, so a catalogue refresh updates every entry (FR-034a.1) |
| `price` | `Option<Cost>` | no | **the escape hatch**: a hand-entered cost, the `Cost` type the catalogue already uses (`crates/vizier-core/src/catalogue/data.rs:142`; per-million rates, `input`/`output` required). Set ⇒ no catalogue lookup happens and a refresh never overwrites it (FR-034a.4) |
| `edited` | `bool` | system | true once the person changed the row, so a future fetch never overwrites it (same convention as `CustomModel.edited`, `crates/vizier-core/src/schema/provider.rs:225`) |

Validation (one place: `service::providers_sso::validate_entries`):
1. `parse_inference_profile_arn(&arn)` must succeed ⇒ otherwise `400` naming the expected shape (FR-023, SC-008).
2. No duplicate `arn` within one connection ⇒ `400`, existing entry untouched (FR-025).
3. `arn.region == credentials.region` (Bedrock region) ⇒ otherwise `400` naming both regions (FR-024, SC-009).
4. `name`, when present: 1–100 chars, trimmed. Empty string means "no name" (FR-026).
4a. `model`, when present: a non-empty id matching `[a-z0-9][a-z0-9.:-]*` on the **shape only** (the charset every catalogue Bedrock id in `assets/catalogue/api.json` uses). Existence in the catalogue is deliberately **not** checked here: an id that is gone must stay saved and be reported as "pricing not resolved" at use time (FR-034a.3), and a save-time rejection would turn a pricing gap into a refusal (FR-034a.5, SC-021).
4b. `price`, when present: finite, non-negative `input` and `output`; `cache_read`/`cache_write` optional and, when present, non-negative. No upper bound — a person may know an enterprise rate the catalogue does not carry (FR-034a.4).
4c. **Neither field takes part in rules 1–3.** The pairing never accepts, rejects, rewrites or region-checks an ARN, and never fills `context`, `output`, `tool_call` or `image_input` (FR-034a.6, SC-022).
5. Removing an entry that an agent still selects ⇒ `409` naming those agents, unless `confirm=true` (FR-029) — same shape as `usage::check_in_use`.

**Resolution order for the entry's price** (FR-034a.1–a.6), one function — `service::catalogue::entry_pricing` (research D13):

| # | Condition | Outcome | Usage cost | Entry flag | Requirement |
|---|---|---|---|---|---|
| 1 | `price` is set | `Explicit(cost)` — used as given, no catalogue lookup | the figure | none | FR-034a.4 |
| 2 | `model` is set **and** resolves through the provider alias (D2) | `FromCatalogue(cost)`, re-resolved now | the chosen model's cost | none | FR-034a.1, FR-034a.3 |
| 3 | `model` is set, id **no longer** in the catalogue | `NotResolved { model }` | `None` (unpriced) | **pricing not resolved**, naming the stored id | FR-034a.3, FR-034b |
| 4 | `model` empty | `NotSet` | `None` (unpriced) | **pricing not set** | FR-034a.2, FR-034b |

Row 3 also covers the third absence FR-034b names: **the catalogue not yet loaded** on a cold start before its seed is read. The resolver cannot distinguish "withdrawn" from "not loaded yet" and does not try — both report the id that failed, and both resolve themselves once the catalogue is there, because nothing was written down.

Rules the table encodes:

- **`None` means unpriced, never zero.** It reuses the representation `Pricer::price` already returns (`crates/vizier-core/src/usage/mod.rs:69`), so no new concept enters the usage schema (FR-034b).
- **Re-resolved at display and at use.** Only the id is stored, and `usage_event` has no cost column, so every report recomputes the price from the catalogue as it stands now (FR-034a.1). A catalogue refresh therefore relabels and reprices existing entries and orphans none.
- **Nothing is derived from the ARN.** No parsing of the ARN feeds the price, in any branch (FR-034a, SC-020).
- **The flag is a rendering of the enum, not a stored field.** `sso_state`-style reporting: the entry's API row carries `pricing` (see `contracts/provider-credentials-api.md` §4a–§5), computed per request. No column, no migration.
- **A price never blocks a turn.** `resolve` and the media path never call the resolver: a turn builds its model from the ARN and the entry's limits, and the price is attached afterwards, when a report prices that ARN by looking the entry up (research D13 §3). There is therefore no code path in which a missing, stale or absurd price could refuse, retry or disable anything — FR-034a.5 holds structurally, not by review.

**Resolution order for limits and capabilities** (FR-028) is the existing one, unchanged:
`agent override` → `entry` → `unknown`. Concretely, `resolve_bedrock_sso` builds `Limits` from the entry when it is the selected model, and `limits()` (`crates/vizier-core/src/llm/resolve.rs:156`) applies the agent override on top; an unset field stays `None`, which the rest of the system already treats as unknown (checkpointing, attachment handling, tool warnings).

---

## Entity: the connection state machine

Drives what the settings row shows and which actions are offered.

```text
unconfigured ──save fields──▶ configured (no session)
configured ──sign-in start──▶ pending  (code + URL shown, expires in ~10 min)
pending ──approved──▶ configured (session stored)      [expiry shown]
pending ──denied / expired──▶ configured               [reason shown, nothing stored]
configured ──first request──▶ signed-in (credentials cached)
signed-in ──credentials near expiry──▶ renewing ──ok──▶ signed-in
signed-in ──session ended on AWS──▶ session-expired    [re-sign-in offered]
signed-in ──key missing/wrong/rotated──▶ session-unreadable [re-sign-in offered, nothing deleted]
any ──sign-out in Vizier──▶ configured (session row deleted)
any ──delete connection──▶ unconfigured (row + session deleted; agents report the missing connection)
```

| State | What the API reports | Allowed actions |
|---|---|---|
| `unconfigured` | `session: null` | save, delete |
| `configured` | `session: { profile, start_url, sso_region, bedrock_region, account_id, role_name, expires_at: null }` | sign-in, save, delete |
| `pending` | `sign_in: { user_code, verification_uri, expires_at }` — **no** `device_code` | poll (implicit), cancel |
| `signed-in` | `session: { …, expires_at: <ts> }` | re-sign-in, sign-out, entries CRUD, test |
| `session-expired` | `session: { …, state: "expired" }` | re-sign-in, sign-out |
| `session-unreadable` | `session: { …, state: "unreadable", reason: "key" }` | re-sign-in, sign-out |

`device_code`, `access_token`, `refresh_token`, `client_secret` and both credential halves are **never** in any of these payloads (FR-009b, FR-035).

---

## Entity: agent model selection (unchanged shape, new value)

An agent stores `provider = "amazon-bedrock-sso"` and `model = <catalogue id>` **or** `model = <arn>`. Resolution (`crates/vizier-core/src/llm/resolve.rs:290`) distinguishes the two by whether `parse_inference_profile_arn` succeeds, not by a stored flag:

| `model` value | Resolves to |
|---|---|
| catalogue model id (e.g. `anthropic.claude-…`) | dialect `Bedrock`, limits and price from the catalogue entry reached through the alias |
| a saved entry's ARN | dialect `Bedrock`, model string = the ARN **unchanged**, limits from the entry (then the agent override). Nothing about the price is decided here: when a report later prices this ARN it finds the entry and applies `price` → `model` → unpriced (research D13) |
| an ARN that parses but is **not** in the connection's list | refused at build with `ProfileNotFound` (the list is the authority, not the grammar) |

This is what makes FR-030 (catalogue models remain selectable) and FR-027 (ARN sent unchanged) hold at the same time.

---

## Entity: media selection (chat-only asymmetry)

| Capability | What the picker offers | What a request refuses |
|---|---|---|
| chat | catalogue models **and** saved entries | — |
| memory embedding | the existing foundation-model families (`EmbeddingFamily`, `crates/vizier-core/src/llm/media/bedrock.rs:35`) | an ARN, **before** the request, naming the capability and the models that work (FR-033a) |
| image generation | the existing foundation-model families (`ImageFamily`) | an ARN, same refusal |

Defaults are untouched: `amazon.titan-embed-text-v2:0` and `amazon.nova-canvas-v1:0` (`crates/vizier-core/src/llm/media/bedrock.rs:16,18`) apply when the field is empty (FR-033b).

---

## Requirement traceability

| Requirement | Where it is satisfied |
|---|---|
| FR-001, FR-001a | new `ProviderId`, `builtin::sso_bedrock()`, `guide::for_provider` per-id dispatch |
| FR-002 | one credential helper used by `resolve.rs` and `media/mod.rs`; signer/dialect untouched |
| FR-003, FR-004 | no migration; the existing row is never read by the new code paths |
| FR-005, FR-006, FR-008 | device flow (D4) + `start_url`/regions/account/role on the connection |
| FR-007 | portal accounts/roles; free-text fallback only when the portal returns none |
| FR-009, FR-009a–f | `bedrock_sso_session` + envelope + AAD + state machine above |
| FR-010, FR-011 | separate `sso_region` / `region`, `valid_region` for both, messages name which |
| FR-012, FR-012a, FR-012b | server-side sign-in endpoints callable before save; invoke-based test (D6) |
| FR-013 – FR-017 | D5 cache/renewal + `invalidate()` retry + session-expired wording |
| FR-018, FR-019 | existing `usage::restart_users` fires for this provider id only |
| FR-020 – FR-032 | `ProfileEntry` + entry CRUD + parser + resolution above |
| FR-033, FR-033a, FR-033b | media table above |
| FR-034 | provider id on the usage row is the new id; a catalogue-model turn is priced exactly as today |
| FR-034a, FR-034a.1 | `ProfileEntry.model` + `entry_pricing` resolving the id through the provider alias on every display and every use; no name or price copied onto the entry |
| FR-034a.2 | `model` empty ⇒ `NotSet` ⇒ `None` cost + "pricing not set"; the entry stays usable |
| FR-034a.3 | `NotResolved { model }` ⇒ `None` cost + "pricing not resolved" naming the id; no re-pointing, no inference |
| FR-034a.4 | `ProfileEntry.price` (`Cost`) wins and short-circuits the lookup; no write path exists that could overwrite it |
| FR-034a.5 | the resolver is reachable only from a report, never from `resolve` or the turn path — a missing price cannot refuse, retry or disable anything |
| FR-034a.6 | validation rules 4a–4c: the pairing is shape-checked only and never feeds rules 1–3 or the limits fields |
| FR-034b | the four-row table above: `None` cost, tokens and ARN still recorded, the flag naming the id |
| FR-034c | no field and no resolver path reaches an AWS endpoint; the dropdown reads `Catalogue` only |
| SC-020 | `Explicit` / `FromCatalogue` are the only two priced outcomes; a price never comes from the ARN; a refresh reprices without touching entries |
| SC-021 | `NotSet` / `NotResolved` still record provider + ARN + tokens with `None` cost, and no code path consults the price before invoking |
| SC-022 | `entry_pricing` reads `price` and `model` only; `parse_inference_profile_arn` and the region check never read either field |
| FR-035 | D8 |
| FR-036 | nothing in the design is channel-specific |
| FR-037 | no AWS call at startup: the flow is only reachable from a request |
| FR-038, FR-039 | guide as data + `docs/src/configuration/providers.md` section |
| FR-040 | delete names the agents (existing `check_in_use`), plus the session row |
| FR-041 | `SsoError` (D7) |
| FR-042 | no new write path in the turn; a failure returns before the assistant message is stored (existing behaviour) |
