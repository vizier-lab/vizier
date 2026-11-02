# Contract: provider credentials API (SSO additions)

**Feature**: `044-bedrock-sso-inference-profiles` | **Plan**: [plan.md](../plan.md) | **Data model**: [data-model.md](../data-model.md)

The HTTP surface. Every addition is **additive**: the existing `amazon-bedrock` endpoints, payloads and errors are unchanged (FR-004). All routes sit under the existing authed provider router (`crates/vizier-http/src/api/v1/providers/mod.rs:28`), so permissions, envelopes and error mapping are whatever that router already does.

---

## §1 The listing (`GET /api/v1/catalogue/providers`)

A second Bedrock row appears, with its own name, guide, fields and model count.

| Field | `amazon-bedrock` | `amazon-bedrock-sso` |
|---|---|---|
| `id` | `amazon-bedrock` | `amazon-bedrock-sso` |
| `name` | `Amazon Bedrock` | `Amazon Bedrock (SSO)` |
| `kind` | `cloud` | `cloud` |
| `dialect` | `bedrock` | `bedrock` |
| `fields` | `[access_key_id, secret_access_key, session_token, region]` | `[start_url, sso_region, account_id, role_name, sso_profile, region]` |
| `model_count` | the catalogue's count | **the same count** (alias, research D2) |
| `guide` | the static-key guide | the SSO guide (§2) |
| `connection.status` | `connected` when keys are saved | `connected` when a session exists |

`guide` for every other provider stays `null` — the assertion at `crates/vizier-http/src/api/v1/catalogue.rs:312` is corrected to exclude **both** Bedrock ids, not weakened.

### New `CredentialField` members

| Variant | Wire | Label (WebUI) | Shown in clear |
|---|---|---|---|
| `StartUrl` | `start_url` | Identity Center start URL | yes |
| `SsoRegion` | `sso_region` | SSO region | yes |
| `RoleName` | `role_name` | Role name | yes |
| `SsoProfile` | `sso_profile` | Profile name | yes |

`account_id` and `region` reuse the existing members (both already exist and are already shown in clear: `webui/app/lib/providers.ts:17`).

---

## §2 The guide (`ProviderGuide`, FR-038)

Six steps, one policy, four links — the same shape the static guide has (`crates/vizier-core/src/catalogue/guide.rs:35`):

| Step | Title | Content |
|---|---|---|
| 1 | Find the Identity Center start URL | the portal URL your organisation gave you; it looks like `https://<subdomain>.awsapps.com/start` |
| 2 | Choose the SSO region | the region your Identity Center instance lives in (often not the Bedrock region) |
| 3 | Sign in and pick account + role | press **Sign in**, confirm the code in a browser on any machine, then choose from the lists |
| 4 | Attach the minimal policy | `bedrock:InvokeModel` only; listing permissions are optional |
| 5 | Add your inference profiles, and price them | paste each ARN, name it, then pick the model it runs from the searchable Bedrock list so its usage is costed — optional, and an entry with nothing picked still chats, shown as "pricing not set" |
| 6 | Test the connection | press **Test** — one small billable call to the model or profile the test names |

Policy (copyable):
```json
{
  "Version": "2012-10-17",
  "Statement": [{ "Effect": "Allow", "Action": ["bedrock:InvokeModel"], "Resource": "*" }]
}
```
Links: Identity Center console, Bedrock model access, inference-profile docs, `https://www.vizier.rs/docs/configuration/providers#amazon-bedrock-sso`.

The same text is the new section of `docs/src/configuration/providers.md` (FR-039), reachable through the anchor above.

---

## §3 Sign-in endpoints

All under the existing provider router; every one requires `settings:providers`.

### `POST /api/v1/providers/{id}/sso/sign-in`

Starts or continues a sign-in. Body — all optional; omitted values fall back to the saved connection:

```json
{ "start_url": "https://acme.awsapps.com/start", "sso_region": "us-east-1", "profile": "acme-bedrock" }
```

200:
```json
{ "user_code": "ABCD-EFGH", "verification_uri": "https://device.sso.us-east-1.amazonaws.com/",
  "verification_uri_complete": "https://device.sso.us-east-1.amazonaws.com/?user_code=ABCD-EFGH",
  "expires_at": "2026-11-02T05:12:00Z", "interval_seconds": 5 }
```
202 while the person has not approved yet:
```json
{ "state": "pending", "expires_at": "2026-11-02T05:12:00Z" }
```
200 when approved (the second call, or the same call after the browser step):
```json
{ "state": "signed-in", "account_id": "123456789012", "role_name": "BedrockOperator",
  "expires_at": "2026-11-02T13:12:00Z" }
```

Errors: `SsoError::SignInDenied` → 400 "the sign-in was denied in the browser"; `CodeExpired` → 400 "the sign-in code expired; start again"; `Unavailable` → 502 with the region named. **No** `device_code`, `access_token` or `client_secret` in any body (FR-009b).

### `GET /api/v1/providers/{id}/sso/accounts`

```json
{ "accounts": [{ "account_id": "123456789012", "account_name": "Acme Prod", "email": "ops@acme.example" }] }
```
Empty list + `"fallback": true` when the portal returned nothing, so the form offers free text (FR-007).

### `GET /api/v1/providers/{id}/sso/accounts/{account_id}/roles`

```json
{ "roles": [{ "role_name": "BedrockOperator", "account_id": "123456789012" }], "fallback": false }
```

### `POST /api/v1/providers/{id}/sso/sign-out`

Deletes the session row for the profile and returns `{ "state": "configured" }`. The connection, its entries and its agents are untouched (FR-009c). Never an error if already signed out.

---

## §4 Provider row (`GET /api/v1/providers/{id}`, `PUT /{id}`, `DELETE /{id}`)

Additive fields on `ProviderResponse`:

| Field | Type | Notes |
|---|---|---|
| `has_sso_session` | `bool` | a session row exists and decrypted |
| `sso_state` | `"configured"｜"signed-in"｜"expired"｜"unreadable"` | the state machine in [data-model.md](../data-model.md) |
| `sso_profile` | `Option<String>` | the profile name (non-secret) |
| `start_url` | `Option<String>` | shown in clear |
| `sso_region` | `Option<String>` | shown in clear |
| `role_name` | `Option<String>` | shown in clear |
| `session_expires_at` | `Option<DateTime<Utc>>` | shown |
| `profiles` | `Vec<ProfileEntryResponse>` | `arn`, `name`, `context`, `output`, `tool_call`, `image_input`, `model`, `price`, `edited`, plus `display_name` (name, or the profile id from the ARN) and the computed `pricing` (§4a) |

Never added: any token, credential half, `client_secret`, `device_code`.

`PUT /{id}` accepts the new credential fields (§1) plus `profiles` (the whole list, one operation — FR-021). Omitted `profiles` keeps the stored list; `[]` clears it (same merge semantics as `ProviderCredentials::merged_with`, `crates/vizier-core/src/schema/provider.rs:180`). Entry validation is [data-model.md](../data-model.md) ("profile entry"), including rules 4a–4c: `model` is checked by **shape only** (an id that has left the catalogue must still be savable, FR-034a.3), `price` must be finite and non-negative, and neither field takes part in the ARN or region checks (FR-034a.6).

### §4a The entry's pricing fields (FR-034a–a.6)

Two **optional** request/response fields on each entry, both non-secret, both meaningful only for pricing:

| Field | Type | Where | Meaning |
|---|---|---|---|
| `model` | `string ∣ null` | request + response | one catalogue Bedrock model **id**, chosen from the list in §5a. Stored as an id; its name and cost are re-resolved on every read, so a catalogue refresh relabels and reprices every entry and orphans none (FR-034a.1) |
| `price` | `Cost ∣ null` | request + response | a hand-entered cost, the catalogue's own `Cost` shape (`input`, `output`, optional `cache_read`/`cache_write`, per-million rates). Set ⇒ no catalogue lookup happens and a later refresh never overwrites it (FR-034a.4) |
| `pricing` | object | **response only, computed** | what the entry's usage will cost right now. Never stored, never accepted on input |

`pricing` is one of **five** shapes, listed in order of precedence — **first match wins**, and the order is load-bearing (see the precedence bullet below):

```json
{ "state": "explicit",       "cost": { "input": 3.0, "output": 15.0 }, "source": "entered price" }
{ "state": "no_catalogue",   "flag": "pricing not set",
  "reason": "the model catalogue has not loaded yet, so no price is available — this resolves itself; or enter a price by hand" }
{ "state": "from_catalogue", "cost": { "input": 3.0, "output": 15.0 }, "model": "anthropic.claude-sonnet-4-5-20250929-v1:0" }
{ "state": "not_resolved",   "model": "anthropic.retired-model-v1", "flag": "pricing not resolved",
  "reason": "no price for anthropic.retired-model-v1: it is no longer in the catalogue — pick another model or enter a price by hand" }
{ "state": "not_set",        "flag": "pricing not set",
  "reason": "no model is paired with this ARN, so there is nothing to price it from — pick one from the list or enter a price by hand" }
```

- `not_set` still chats and still records its tokens; its usage row is **unpriced**, not zero (FR-034a.2, FR-034b).
- `not_resolved` names the stored id **inside the reason** so the person can re-pick it; the entry is not refused, not disabled and not re-pointed at another model (FR-034a.3).
- **The order is precedence, and it decides the two cases the shapes alone leave ambiguous.** An explicit price wins over everything: no catalogue lookup happens, so no absence can apply. **`no_catalogue` is checked before `model`**, so it wins both when the list is absent *and* no model is paired (nothing can be picked from a list that is not there, so `not_set`'s way out would be unactionable) and when the list is absent *and* a model is paired (nothing is "gone" — there is nothing to look in, so `not_resolved` would misname the cause). With the list present the other three are mutually exclusive: a model resolves, is gone, or was never picked. Precedence in full: [data-model.md](../data-model.md) "profile entry".
- **`reason` is required on every absence state and is never empty, never a placeholder, and never shared between any two of the three causes.** It states the cause *and* the way out. This is the shape decision the spec left to planning (spec.md validation run 7: *"pricing not set diterima jika ada reason itu"*): the reason rides beside `flag` rather than replacing it, so an existing client that reads only `flag` keeps working and SC-021 becomes checkable on one field.
- **`no_catalogue` is FR-034b's third absence** — the catalogue carries no Bedrock list, which is what a cold start without its seed leaves behind (the seed is embedded and parsed synchronously at boot, `crates/vizier-core/src/dependencies.rs:163`, so the state is "no provider row", never a race with a load). It renders as "pricing not set" to the person (there is nothing to pick yet) but carries its own reason, because the fix is "wait", not "pick a model". It is distinguishable from `not_set` **and from `not_resolved`** by `reason` alone, which is what SC-021 counts — and it outranks both, per the precedence bullet above.
- None of the three absence states blocks anything: `pricing` is read from a report, never from the turn path (FR-034a.5, SC-021).
- The flag, the reason and this whole object are a **rendering**, computed per request from the entry's current state — not a stored column. A price entered after the fact therefore also reprices turns already recorded, exactly as a catalogue price change already does, and re-picking or pricing an entry clears its reason with no manual edit (research D13 §4, SC-021).

`DELETE /{id}`: `409` naming the agents in use unless `confirm=true` (existing `usage::check_in_use`), then deletes the row **and** the session row (FR-040).

---

## §5 Model list (`GET /api/v1/catalogue/providers/{id}/models`)

For `amazon-bedrock-sso`, the response is the catalogue's Bedrock models **plus** one `ModelSummary` per saved entry:

```json
{ "id": "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123",
  "name": "Acme Prod — Claude", "default": false,
  "limit": { "context": 200000 }, "caps": { "tool_call": true, "image_input": true },
  "cost": { "input": 3.0, "output": 15.0 }, "status": "stable", "origin": "inference_profile",
  "pricing": { "state": "from_catalogue", "model": "anthropic.claude-sonnet-4-5-20250929-v1:0" } }
```

- Entries come **first** (they are the reason the person is here), then the catalogue models, unchanged (FR-022, FR-030).
- `origin: "inference_profile"` is the only new value; `origin` already exists for local models (`service/catalogue.rs`, `bare(...)`).
- **`cost` is the entry's resolved price** — the hand-entered figure if there is one, else the chosen model's catalogue cost, else `null` (FR-034a). `null` is the same "unpriced" a custom provider's model already reports; `pricing` says which absence it is.
- `limit`/`caps` are `null` where the entry left them empty — unknown, never "unsupported" (FR-028). **They are never taken from the paired model** (FR-034a.6, SC-022): an entry with no context window reports unknown limits even when its pair carries 200 000.

### §5a The dropdown's source (FR-034a.1)

`GET /api/v1/catalogue/providers/{id}/models` for **the SSO id** is also the dropdown's source: the entries are the first block, and the catalogue's own Bedrock models follow. That is deliberate — it is the **same list the chat picker already offers**, so the dropdown adds no endpoint, no data source and no AWS call (FR-034c). The form filters it client-side by name and id (the design system's `Picker` already searches and windows), and it must offer **only chat models**: the catalogue's Bedrock row carries no embedding or image model, which is what makes it safe to place beside an ARN (FR-033a).

**Which flag the dropdown loads with.** It calls the same endpoint as the chat picker, with `all=true` — the **unfiltered** Bedrock list, which is the 203-model figure FR-034a quotes. The default picker list (`all=false`) drops deprecated models and models known to lack tool calling; a person may still hold an inference profile over a legacy model and must be able to price it, so the dropdown shows the full list and marks a deprecated row rather than hiding it. It offers **chat models only** either way — there is no media model in that row to hide.

If the SSO connection has no session yet, the entries are still listed and the catalogue block is still returned — the dropdown is populated from Vizier's catalogue, so pricing can be set before signing in.

---

## §6 Error strings (FR-041, SC-015)

| Cause | HTTP | Message |
|---|---|---|
| sign-in denied | 400 | `the sign-in was denied in the browser` |
| code expired | 400 | `the sign-in code expired; start again` |
| session expired | 401 | `the AWS SSO session has ended; sign in again to reconnect "Amazon Bedrock (SSO)"` |
| session unreadable | 409 | `the stored session for profile "…" cannot be read with this workspace key; sign in again (nothing was deleted)` |
| role not assigned | 400 | `role "…" is not assigned in account "…"; pick one of the listed roles` |
| permission missing | 403 | `the role is not allowed to perform bedrock:InvokeModel; attach the policy shown in the guide` |
| model access not enabled | 403 | `model "…" is not enabled for this account in <region>` |
| profile not found | 404 | `inference profile "…" was not found; it may have been deleted` |
| region mismatch | 400 | `the ARN names region "…" but this connection uses "…"` |
| throttled | 429 | `AWS throttled the request; try again shortly` |
| unavailable | 502 | `AWS did not answer; the "…" region may be wrong or unreachable` |

Each is a distinct `SsoError` variant (research D7); a test asserts all eleven strings are pairwise distinct and that none contains `token`, `secret` or `session` followed by a value.
