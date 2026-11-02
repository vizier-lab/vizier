# Specification Quality Checklist: AWS Bedrock over SSO with Application Inference Profiles

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-11-02
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- Items marked incomplete require spec updates before `speckit_clarify` or `speckit_plan`.

### Validation run 1 (2026-11-02, first draft)

- **No implementation details**: PASS with a deliberate, named exception: three AWS product names are the
  requester's own vocabulary (*IAM Identity Center / AWS SSO*, *Application Inference Profile*,
  `bedrock:InvokeModel`). No crate, module, endpoint path or storage schema is named.
- **[NEEDS CLARIFICATION]**: FAIL by design — 3 markers (Q1–Q3), the skill's maximum.
- **Scope bounded**: PASS, with excluded credential forms and Bedrock features named in Assumptions.

### Validation run 2 (2026-11-02, verification pass after an interrupted run)

- **All mandatory sections**: PASS, template order preserved.
- **No source edits left behind**: PASS — no SSO Bedrock id, module or config key under `crates/` or `webui/app/src`.
- **034 cross-reference**: PASS — SSO excluded verbatim at `specs/034-aws-bedrock-provider/spec.md:251`, error path at 149.
- **Signer and client reuse**: PASS — `llm/sigv4.rs` and `llm/dialect/bedrock.rs` exist; no `aws-*` crate anywhere.
- **Corrections**: the failing assertion is at `crates/vizier-http/src/api/v1/catalogue.rs:312` (test from :279);
  the `src/channels/...:932` path from run 1 does not exist. `guide::for_provider` compares ids by exact
  equality, so a new Bedrock id returns `None`; giving the **existing** id a second guide is what breaks the
  assertion. Recorded as planning input, test untouched.

### Validation run 3 (2026-11-02, Q1=A applied after a wrong B draft)

- **Q1 correct**: full parity (chat + embeddings + images), FR-033/033a/033b, US4 with seven scenarios.
- **Embedding/image selection verified in the repo**: `crates/vizier-core/src/llm/media/bedrock.rs` selects by
  foundation-model family, refuses unsupported families before the request, defaults to
  `amazon.titan-embed-text-v2:0` / `amazon.nova-canvas-v1:0`. Basis for FR-033a/033b, not an assumption.

### Validation run 4 (2026-11-02, Q2 reversed from B to A)

Q2 was answered B (platform AWS SSO cache shared with the AWS CLI) and then changed to **A** (Vizier's own
encrypted storage) after both options were explained to the requester. This pass rewrites every section that
depended on the B answer rather than patching it.

- **No `~/.aws` dependency anywhere in the normative text**: PASS — 0 mentions of an AWS SSO cache, a shared
  cache or a cached session outside the change-history table that records the reversal.
- **At-rest encryption is now explicit and Vizier-owned**: PASS — FR-009a requires the session be written only
  through the existing encrypted path and be unreadable without the workspace key; FR-035 states at-rest
  protection is Vizier's own obligation, not a borrowed one; SC-011 checks the workspace for plaintext.
- **Machinery reused, not invented (verified in the repo)**: PASS — `crates/vizier-core/src/encryption/`
  provides `SecretCipher`, a versioned envelope with per-field AAD, and key resolution that fails closed
  (`key.rs`); `protect_provider_entry` (`fields.rs:104`) already encrypts `secret_access_key` and
  `session_token` on the existing Bedrock provider. FR-009a therefore names an existing path, and FR-009f
  can name the key-loss failure mode precisely.
- **Profile name still required and still non-secret**: PASS — FR-009e, restated for an own-store context: a
  readable handle, survives a restart, one handle per connection, never a credential. Its purpose survives
  the reversal; only what it addresses changed.
- **Requirements reworked one by one**: FR-005, FR-009 rewritten; FR-009a–e reworked; **FR-009f added**
  (key lost/rotated); FR-035 rewritten; SC-011 and SC-017 rewritten; **SC-019 added**; Assumptions updated;
  US1 acceptance 7, Key Entities, three Edge Cases and the Context paragraph reworded.
- **Withdrawn assumptions**: PASS — "Vizier implements the AWS CLI cache format", "atomic writes against a
  concurrently written CLI file", "a CLI sign-in is honoured by Vizier and the reverse", "one sign-in serves
  both tools". All are recorded as withdrawn in the change-history table.
- **Kept**: no AWS SDK crate; device flow is Vizier's own; profile name non-secret; role credentials in memory
  only; restart with a live session needs no re-sign-in.

#### Consequences recorded, not hidden

- **Q2=A**: two independent sessions when the person also uses the CLI (browser approval twice); the session is
  not shareable with any other tooling; Vizier owns the whole lifecycle (refresh, expiry, sign-out, key loss);
  at-rest security equals the security of the workspace key and directory, and a workspace backup including
  the key is effectively a backup of the live session. Not lost: a restart with a live session still needs no
  re-sign-in (SC-017 a). Genuinely lost: cross-tool session sharing only.
- **Q1=A**: chat can be metered per team profile while embeddings and images stay on foundation-model access;
  a profile-only model access will have embedding/image calls refused by AWS, passed through naming the model.
- **Q4=B**: the connection test is one small billable inference call consuming tokens and model quota; the
  guide and the button must say so first (SC-018).

#### Remaining item, delegated to planning (not a clarification)

The provider id and display name strings are chosen at planning time, as the Assumptions record. No other
question is open.

### Validation run 5 (2026-11-02, pricing of ARN entries and ARN grammar)

Two later requester decisions applied on top of the closed clarifications. Q1–Q4 are not reopened; Q3's
"optional, unknown when empty" is unchanged and the pricing field follows the same shape.

- **Pricing from models.dev, mechanism (a)**: PASS — FR-020 gains the optional pricing field; FR-034 is
  clarified for catalogue-resolvable entries; **FR-034a/b/c added** for the resolution order (explicit price →
  foundation-model id in the catalogue → unpriced), the honest-absence rule, and the ban on control-plane calls
  and extra permissions. **SC-020 and SC-021 added**; SC-012 extended. Assumption, Key Entities, Edge Cases and
  US5 scenario 1 updated.
- **Verified against the repo, not assumed**: `catalogue/seed.rs:36` names `https://models.dev/api.json`; the
  embedded `assets/catalogue/api.json` carries provider `amazon-bedrock` with 203 model ids, each with `cost`,
  `limit`, `modalities`, `attachment`, `tool_call`; `usage/mod.rs:69` resolves cost from
  `catalogue.model(provider, model).cost` and returns `None` when absent — the existing "unpriced"
  representation that FR-034a reuses instead of inventing a new one.
- **Media pricing correctly excluded, and the reason is stated**: media comes from OpenRouter
  (`catalogue/media.rs:66`) and `llm/media/bedrock.rs` only accepts foundation-model families. Since an
  inference profile is a chat resource, no ARN entry can reach the media catalogue, so "ambil dari models.dev"
  covers every ARN case. FR-033a is untouched by the grammar fix.
- **ARN grammar corrected**: PASS — FR-023 now accepts both `inference-profile` and
  `application-inference-profile`, with partitions `aws`, `aws-cn`, `aws-us-gov`, `aws-iso`, `aws-iso-b`;
  SC-008 extended so both forms must be stored and neither refused for being the other; FR-033a reinforced so
  a wider grammar does not widen what embedding or image calls may use; Assumptions and Edge Cases record why
  the application form is the one the feature exists for.
- **No permission creep**: PASS — FR-034c and FR-012b together keep the minimal policy at
  `bedrock:InvokeModel` alone. 0 control-plane calls are introduced.

#### Consequences to hand to the requester

- The person now has **one optional field** per ARN entry: the underlying foundation-model id (or an explicit
  price). Filling it is what makes usage costed; leaving it empty is allowed and simply means "unpriced",
  shown as such, never as zero.
- A wrong or unknown foundation-model id does not break the turn — it flags the entry as pricing-not-resolved
  and names the id, so it can be corrected without affecting past rows.

### Validation run 6 (2026-11-02, pricing model chosen from a dropdown)

The requester refined mechanism (a): instead of typing the underlying model id, the person **picks it from a
list of Bedrock-supported models** and pairs it with the ARN. Verified on disk first — the earlier pricing and
grammar revision was already written, so this pass refines it rather than repeating it.

- **The list has a stated, existing source**: PASS — FR-034a names the catalogue's own `amazon-bedrock` entry,
  the same list that already feeds this provider's chat model picker. No new data source, no AWS call.
- **The list is safe to offer beside an ARN, verified**: PASS — the embedded catalogue holds 203 Bedrock
  models, **0 without a `cost`**, and **none of them an embedding or image model** (families are all chat
  families). A dropdown built from it cannot smuggle a media model into a chat entry, and every option can
  actually price a turn.
- **Optional or required, stated unambiguously**: PASS — optional (Q3 unchanged). An ARN alone still saves and
  still chats; usage records provider + ARN + tokens with **no cost**, flagged pricing-not-set. 0 costed at 0.
- **Model absent from the list**: PASS — the entry is still savable; either save unpriced, or enter a price by
  hand, or refresh the catalogue and re-pick. No ARN entry is ever refused for want of a model.
- **Pricing never blocks inference**: PASS — FR-034a.5 and SC-021.
- **Selected model gone later**: PASS — stored id kept, flagged pricing-not-resolved naming it, no silent
  re-pointing, no silent zero (FR-034a.3, FR-034b).
- **Does the dropdown touch ARN validation?** PASS — no. FR-034a.6 and SC-022: ARN acceptance stays exactly
  FR-023's, region checking stays exactly FR-024's read from the ARN string, and changing the pairing never
  changes whether an ARN is accepted.
- **Does the dropdown fill in Q3's limits?** PASS — no. FR-034a.6 and SC-022: an entry with no limits entered
  still reports **unknown**, even when the paired model carries `limit.context` and `limit.output` in the
  catalogue. 0 limits copied, so Q3's "unknown when empty" is preserved exactly.
- **Catalogue refresh behaviour**: PASS — entries store an id and re-resolve label and price at display and
  use time; a refresh updates both and orphans nothing; a hand-entered price is never overwritten.

#### Consequences to hand to the requester

- Adding an entry is now: paste the ARN → optionally search the model list → pick one → save. No model id is
  typed, and the dropdown shows the same models the provider's chat picker shows.
- If the model is not in the list, the entry still works for chat, just unpriced — or a price can be typed in
  by hand, or the catalogue refreshed and the model picked again.
