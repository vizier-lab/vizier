# Quickstart: AWS Bedrock over SSO with Application Inference Profiles

**Feature**: `044-bedrock-sso-inference-profiles` | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)

Verification is split by access, as with the existing Bedrock provider: **§1–§4 run offline** (no AWS account, deterministic, this is the CI-shaped part — §2b included, because pricing reads only Vizier's own catalogue), and **§5–§6 need a live Identity Center instance** and are a volunteer release checklist, not a CI job. Live steps are marked **[live]**.

## Verifier report

The volunteer fills this in on the special release and attaches it to the release notes.

| Item | Result / notes |
|---|---|
| Build / commit tested | |
| Identity Center start URL region | |
| Bedrock region | |
| `bedrock:InvokeModel` only (no listing permission) | |
| §5.2 sign-in completes, code shown, browser confirm | |
| §5.3 accounts and roles listed, none typed | |
| §5.4 catalogue model chat turn | |
| §5.5 ARN entry paired with its model; turn uses that profile (console metrics) | |
| §5.5b ARN entry with no model → unpriced + "pricing not set", never $0.00 | |
| §5.6 two entries on one model, metered and costed separately | |
| §5.7 Test action on an ARN (billable: one call) | |
| §2b all five pricing outcomes offline, each with its reason | |
| §5.8 memory embedding + `memory_search` on an SSO-only setup | |
| §5.9 one image generated, no second sign-in | |
| §5.10 `~/.aws` unchanged before/after (`diff -r`) | |
| §5.11 re-sign-in after expiry; agents resume without restart | |
| §5.12 session ended on AWS side → session error, not key/model error | |
| Anything that failed, with the error text | |

---

## §1 Offline — storage and encryption (no AWS)

Precondition: `just install` once, then `cargo build`.

```sh
export VIZIER_DATA_DIR=$(mktemp -d)/ws
cargo run -q -- run --config dev.vizier.yaml &   # or `just run-d`
```

1. A fresh workspace gains the table and nothing else:
   ```sh
   sqlite3 "$VIZIER_DATA_DIR/.runtime/vizier.db" ".tables" | tr ' ' '\n' | grep bedrock_sso_session
   ```
   **Expected**: `bedrock_sso_session`.
2. **Restart with a live session keeps it** (FR-009, SC-017a): insert a session through the API in §2, stop the process (`just shutdown`), start it again, and
   ```sh
   curl -s localhost:9999/api/v1/providers/amazon-bedrock-sso -H "Authorization: Bearer $TOKEN" | jq .sso_state
   ```
   **Expected**: `signed-in` and the same `session_expires_at`; no new sign-in.
3. **A wrong key fails closed** (FR-009f, SC-019): restart with `VIZIER_ENCRYPTION_KEY` set to a different 32-byte value (hex), then read the provider.
   **Expected**: `sso_state = "unreadable"`, a message naming the profile, `200`; the connection row, its `profiles` entries and its agents all still present.

## §2 Offline — session storage and the API surface (no AWS)

With the local stub from the unit tests promoted to a scripted mode (`crates/vizier-core/src/tests_support.rs`), or by hand against a saved row:

1. `PUT /api/v1/providers/amazon-bedrock-sso` with the six connection fields (no session) → `200`, `sso_state = "configured"`.
2. `GET /api/v1/providers/amazon-bedrock-sso` → **no** token, credential or `client_secret` anywhere in the body:
   ```sh
   curl -s … | grep -Ei 'token|secret|client_secret|device_code' && echo "LEAK" || echo "clean"
   ```
   **Expected**: `clean`.
3. Mark the row signed-in (stub) → `POST …/sso/sign-out` → `sso_state = "configured"`, session row gone, `profiles` intact (FR-009c).
4. Add two entries with the same `arn` → second is refused `400` naming the duplicate; the first keeps its name (FR-025).
5. Add an entry whose region differs from `region` → refused naming both regions (FR-024, SC-009).
6. Add six malformed ARNs (foundation-model ARN, another service, bare model id, truncated, console URL, internal space) → all refused with the expected-shape message, nothing stored (SC-008).
7. Add one ARN of **each** resource type and one in each partition → all accepted, none refused for being the other (FR-023, SC-008):
   ```sh
   for arn in \
     "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123def456" \
     "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.anthropic.claude-sonnet-4-5-20250929-v1:0" \
     "arn:aws-cn:bedrock:cn-north-1:123456789012:application-inference-profile/x1:0" \
     "arn:aws-us-gov:bedrock:us-gov-west-1:123456789012:inference-profile/y2:0" \
     "arn:aws-iso:bedrock:us-iso-east-1:123456789012:application-inference-profile/z3:0" \
     "arn:aws-iso-b:bedrock:us-isob-east-1:123456789012:inference-profile/w4:0" ; do
     curl -s -X PUT localhost:9999/api/v1/providers/amazon-bedrock-sso \
       -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
       -d "{\"profiles\":[{\"arn\":\"$arn\"}]}" | jq -r '.data.profiles[-1].arn // .error.message'
   done
   ```
   **Expected**: each echoed back unchanged. (The last four need their connection's `region` set to match, or the region check refuses them first — FR-024, which is a different rule and must not be mistaken for a grammar failure.)
8. The same ARN named for **embedding** → refused as a chat-only resource, in **both** forms (FR-033a).

---

## §2b Offline — pricing an ARN entry (no AWS)

The five outcomes of FR-034a, each verified from the API and from the usage report, together with the required reason on every absence (SC-021). No AWS call is made by any of them (FR-034c), so this whole section is CI-able.

Setup: a saved connection with a session (stub, §2) and the catalogue's Bedrock row present (it ships embedded — `crates/vizier-core/assets/catalogue/api.json`).

1. **Unpaired entry → unpriced, stated.** Save one entry with an ARN and nothing else; point an agent at it; send one message.
   ```sh
   curl -s "localhost:9999/api/v1/catalogue/providers/amazon-bedrock-sso/models" -H "Authorization: Bearer $TOKEN" \
     | jq '.data[] | select(.origin=="inference_profile") | {cost, pricing}'
   ```
   **Expected**: `cost: null`, `pricing.state: "not_set"`, `flag: "pricing not set"` **and a non-empty `pricing.reason`** saying no model is paired and that picking one or entering a price fixes it — a bare flag, an empty reason or a placeholder is a **failure** of this step. The turn **succeeded** and its usage row carries provider + ARN + tokens with no cost — not zero (FR-034a.2, FR-034a.5, FR-034b, SC-021).
2. **Paired entry → priced from the chosen model.** Set `model` on that entry to a Bedrock id from the list, e.g. `anthropic.claude-sonnet-4-5-20250929-v1:0`.
   **Expected**: `pricing.state: "from_catalogue"`, `pricing.model` = that id, and `cost` equal to that model's catalogue cost — the same `cost` the chat picker shows for it. The usage report's `by_model` row for the ARN now carries a figure (FR-034a, SC-020).
3. **Explicit price wins.** Set `price` on the same entry to `{ "input": 1.0, "output": 2.0 }`.
   **Expected**: `pricing.state: "explicit"`, `cost` = the entered figures, **not** the catalogue's, and `model` unchanged (so the pairing survives). No catalogue lookup happens (FR-034a.4, SC-020).
4. **Stale id → not resolved, naming the id.** Point the entry's `model` at an id the catalogue does not hold (e.g. `anthropic.retired-model-v1`) and keep no explicit price.
   **Expected**: save is **accepted** (shape only — [data-model.md](./data-model.md), rule 4a), `pricing.state: "not_resolved"`, `flag: "pricing not resolved"`, `model` echoed, and `pricing.reason` **containing the missing id** and the same two ways out, reading plainly as a different cause from step 1's reason. A turn **still succeeds** and is recorded unpriced (FR-034a.3, FR-034a.5, SC-021).
5. **A refresh reprices, never renames.** Note the entry's `name`; refresh the catalogue (`POST /api/v1/catalogue/refresh`); re-read the models.
   **Expected**: `name`, `arn` and `model` byte-identical; `cost` re-resolved from the refreshed row; an explicit price **unchanged** (FR-034a.1, FR-034a.4, SC-020).
5b. **No catalogue → its own reason, and it wins.** Run against a catalogue that carries **no `amazon-bedrock` list** — an empty catalogue, not a race with the seed: the seed is embedded and parsed synchronously at boot (`crates/vizier-core/src/dependencies.rs:163`), so a test builds `Catalogue::new(CatalogueData::default())` (or boots with a corrupt seed) and the provider row is simply absent. Repeat **both** variants of step 1: the entry with **no model**, and the same entry with `model` set to a valid catalogue id.
   **Expected**: in **both** runs `cost: null`, the usage row recorded unpriced, `pricing.state: "no_catalogue"` with `flag: "pricing not set"` and a reason saying there is nothing to price against yet and that it resolves itself — distinct in wording from step 1's reason (which applies with the list present) and from step 4's. **`no_catalogue` outranks `not_set` and `not_resolved`** ([data-model.md](./data-model.md) "profile entry", precedence): an entry that also has no model, or a model that cannot be checked, still reports `no_catalogue`, because neither other reason's way out can be carried out with no list. **Failure** if either variant reports `not_set` or `not_resolved` here, or if any of the three absences shares another's wording (FR-034b, SC-021).
6. **The pairing never validates or fills limits.** On an entry with no `context`/`output`, pair it with a model that has a 200 000 context window.
   **Expected**: `limit.context` stays `null` (unknown), and an ARN whose region disagrees with `region` is still refused by FR-024's message — while the same ARN's **acceptance** is unchanged by which model is picked (FR-034a.6, SC-022).
7. **No AWS call from any of it.** Grep the trace of steps 1–6 for `bedrock-runtime` / `bedrock.{region}.amazonaws.com`:
   **Expected**: no request from a pricing read, a dropdown load or a usage report — only the turn's own invoke (FR-034c).

## §3 Offline — dummyplug e2e (agent-observable behaviour)

The provider changes what a turn can do, so it is exercised through a running binary with a `dummyplug` agent. Every step is deterministic and needs no key.

1. Start `dummyplug` (`just dummyplug`, listens on `127.0.0.1:9998`) and the server.
2. Point an agent at the SSO provider **without** a session, send `ping`.
   **Expected**: the reply names the **session** (`the AWS SSO session has ended; sign in again …`), the agent stays `ready`, and a second message gives the same answer — no retry loop (FR-016, FR-018, SC-005).
3. With a stubbed session (the stub mints fakes), send `ping` to an ARN entry.
   **Expected**: the request path is `/model/{percent-encoded-arn}/converse`, the ARN unchanged (FR-027, SC-007) — for an `application-inference-profile` ARN **and** an `inference-profile` one, with neither form rewritten. Capture it with `RUST_LOG=vizier_core::llm=trace`.
4. Set the agent's **embedding** model to an ARN and run a memory write.
   **Expected**: refused **before** the request, naming the capability and the four working models; no row in `document_index` for that text (FR-033a, SC-016).
5. Set it back to empty (default) and write a memory.
   **Expected**: `amazon.titan-embed-text-v2:0` is used (visible in the same trace), and `memory_search` finds the text (FR-033b).
6. Delete the connection while an agent uses it → the agent reports the missing connection, not an obscure failure (FR-040).

## §4 Offline — secret-leak audit (SC-011)

Run after §2 and §3, on the same workspace and log:

```sh
# 1. No plaintext session in the workspace
sqlite3 "$VIZIER_DATA_DIR/.runtime/vizier.db" "SELECT data FROM bedrock_sso_session;" | grep -q 'enc:v1:' && echo "encrypted" || echo "PLAINTEXT"
# 2. The pattern of a real token must not appear anywhere
grep -rE 'ASIA[0-9A-Z]{16}|Bearer [A-Za-z0-9._-]{40,}' "$VIZIER_DATA_DIR" && echo "LEAK" || echo "clean"
# 3. Every response body captured in §2/§3 was already grepped in §2.2
# 4. Nothing was written under ~/.aws
diff -r "$HOME/.aws" "$AWS_SNAPSHOT" && echo "untouched" || echo "CHANGED"
```

**Expected**: `encrypted`, `clean`, `untouched`. Any other output is a release blocker.

## §5 Live — volunteer checklist [live]

Prerequisites: an Identity Center instance, one assigned account and role, Bedrock model access in the Bedrock region, and at least one Application Inference Profile ARN. Snapshot `~/.aws` first: `cp -a ~/.aws "$AWS_SNAPSHOT"` (or record that it does not exist).

| # | Step | Expected |
|---|---|---|
| 5.1 | Settings › Providers lists **two** Bedrock rows with distinct names, states and guides | both rows, `Amazon Bedrock` unchanged |
| 5.2 | Open the SSO form, read the guide, press **Sign in** | a short code and a URL appear; confirming in a browser on another machine completes it; the form reports account, role and expiry |
| 5.3 | Choose account and role | only the assigned ones are listed; nothing typed |
| 5.4 | Create an agent on the provider with a catalogue model, send a message | a normal reply; history stored |
| 5.5 | Add an ARN entry with a name, pair it with the model it actually runs (the dropdown), point the agent at it, send a message | reply; Bedrock console shows the **profile** used, not the foundation model; Usage shows that entry **costed from the paired model** |
| 5.5b | Add a third entry with the ARN alone and no model | it chats immediately; Usage lists it **unpriced** with "pricing not set" — not `$0.00` (FR-034a.2, SC-021) |
| 5.6 | Add a second entry whose foundation model is the same, point a second agent at it | both reply; console metrics show two profiles, costed separately |
| 5.7 | Press **Test** on an ARN entry | success naming the target; the guide said it was billable. Then try a role without `bedrock:InvokeModel` | `the role is not allowed to perform bedrock:InvokeModel` |
| 5.8 | Set `embedding.provider` to the SSO provider, empty model; write a memory, then `memory_search` | the default Titan model is used and the memory is found (SC-016) |
| 5.9 | Set `image_gen.provider` to the SSO provider, empty model; generate an image | Nova Canvas default; no second sign-in |
| 5.10 | `diff -r ~/.aws "$AWS_SNAPSHOT"` | no differences (SC-017b) |
| 5.11 | Wait for the session to expire (or sign out of all devices and back in), then send a message, then re-sign in | first message names the session and offers re-sign-in; after signing in, agents resume with **no** Vizier restart, and the entries keep their pairing and price across the outage (SC-006, FR-034a.1) |
| 5.12 | Sign out of the session on the AWS side, send a message | session error, never a key/region/model error (FR-016) |
| 5.13 | With a background task (scheduled task or dream) pointed at the provider and no session | it fails within seconds with the same session message, not a hang (FR-017) |

## §6 Regression — the existing provider is untouched [live or offline]

| # | Step | Expected |
|---|---|---|
| 6.1 | `cargo test -p vizier-http` | includes the corrected guide assertion: only the **two** Bedrock ids have a guide |
| 6.2 | Existing static-credentials Bedrock connection, one chat turn | identical behaviour, no migration, no reindex (FR-004, SC-013) |
| 6.3 | `GET /api/v1/catalogue/providers` before and after adding the SSO connection | the `amazon-bedrock` row's `connection` and `model_count` unchanged |
