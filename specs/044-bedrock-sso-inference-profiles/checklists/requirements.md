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
  **— reversed 2026-11-03 by requester decision: the pairing now fills both limits. See run 8.**
- **Catalogue refresh behaviour**: PASS — entries store an id and re-resolve label and price at display and
  use time; a refresh updates both and orphans nothing; a hand-entered price is never overwritten.

#### Consequences to hand to the requester

- Adding an entry is now: paste the ARN → optionally search the model list → pick one → save. No model id is
  typed, and the dropdown shows the same models the provider's chat picker shows.
- If the model is not in the list, the entry still works for chat, just unpriced — or a price can be typed in
  by hand, or the catalogue refreshed and the model picked again.

### Validation run 7 (2026-11-02, unpriced entries must state a reason)

Requester's decision, verbatim: *"pricing not set diterima jika ada reason itu"* — pricing-not-set is
acceptable **provided the reason is there**. Verified on disk first: FR-034a.2, FR-034a.3, FR-034b and
SC-021 required the label and (for the gone case) the id, but **not** a reason. This pass adds the
reason; it does not reopen Q1-Q4, the ARN grammar, the dropdown, explicit-price precedence, or the
never-blocks-inference rule.

- **Is the reason required, or only the label?** PASS (now required) - FR-034a.2 must show
  "pricing not set" **with** a non-empty reason; SC-021 makes it measurable: 100% of unpriced entries
  carry a cause-specific reason, 0 carry a bare label or a reason that does not name its cause.
- **Does the never-chosen reason say what to do about it?** PASS - FR-034a.2 and FR-034b require the
  cause **and** the way out: pick a model from the list, or enter a price by hand. A reason that only
  says "unpriced" with no next step does not satisfy it.
- **Does the gone-model reason name the missing id inside the reason?** PASS - FR-034a.3 puts the id
  **in the reason itself**, not merely elsewhere in the row, and SC-021 counts that case.
- **Are the two labels distinguishable?** PASS - FR-034a.3 and FR-034b require the cause to be
  identifiable from the reason alone; neither may borrow the other's text. SC-021 counts it.
- **All three absence causes covered?** PASS - FR-034b enumerates them: nothing picked, picked id gone,
  and catalogue not yet available on a cold start before its seed loads.
- **Is the reason stored, and can it go stale?** PASS - it is **derived** from the entry's current state
  at every display and every usage report (FR-034a.2, FR-034b, Key Entities), so no new persisted field
  and no migration are needed, and re-picking or entering a price clears it with no manual edit.
- **Did anything optional become required?** PASS - no. The pairing stays optional (Q3 unchanged); an
  ARN alone still saves and still chats. Only the display of the absence gained an obligation.
- **Did pricing start blocking inference?** PASS - no. FR-034a.5 intact; SC-021 still counts 0 turns
  refused, retried or disabled because of pricing.
- **Did an AWS call or permission enter through this?** PASS - no. FR-034c intact; the reason is built
  from Vizier's own state and catalogue, not from any control-plane call.

#### Consequence for planning (recorded, not executed)

- **Possible DTO/API surface, planning's call**: the field that carries the unpriced marker today can
  carry the reason alongside it, or the reason can be its own field. The spec fixes the content (cause
  plus way out, id inside the reason in the gone case), not the shape. Because the reason is derived, no
  stored column is needed; if it must appear beside historical usage rows, evaluation at report time is
  a planning decision and not a new user-facing requirement.
- The strings quoted in FR-034a.2 are illustrative; the plan may finalise wording, not content.

### Validation run 8 (2026-11-03, the pairing also fills the limits)

Requester's decision, verbatim: *"Iya boleh deh harga dan limit sekaligus terisi selama itu ada di dalam
katalognya"* — price **and** limits may both be filled by the pairing, provided the catalogue carries them.
Verified on disk first: FR-034a.6 said the selection "does not fill in limits", SC-022 measured that 0 limits
were copied, and the Assumptions called the dropdown "not a limit source". This run reverses exactly that
clause and nothing else. Q1, Q2 and Q4 are not reopened.

- **Is the fill source the existing catalogue, or a new one?** PASS - FR-034a.7 names `limit.context` and
  `limit.output` of the **same** `amazon-bedrock` catalogue row the price already comes from. No new data
  source, no AWS call, and FR-034c now says so explicitly for the limits as well.
- **Does the reference case actually exist in the catalogue?** PASS - parsed
  `crates/vizier-core/assets/catalogue/api.json`: `global.xai.grok-4.7` and `us.xai.grok-4.7` both hold
  `"limit": {"context": 500000, "output": 500000}`. SC-022 quotes it, so the criterion is checkable against a
  real row rather than an invented one.
- **Does a hand-entered number win?** PASS - FR-034a.7 rule (1): never overwritten by the fill, by a
  re-pairing, by a refresh, or by a client echoing back what it displayed. SC-022 counts 0 overwritten.
- **What happens when the pair changes?** PASS - rule (2): the new model's limits reach the fields the person
  has not edited, so no entry keeps a previous model's numbers. Stated as behaviour; the mechanism is
  planning's call.
- **What if there is no pair, or the id is not in the catalogue?** PASS - rule (4): unknown (empty), never
  "unsupported", never a number from the ARN, never a guess; a cold start completes the fill when the
  catalogue resolves the id, matching how FR-034a.2's reason clears itself.
- **Can a stale catalogue silently shrink a working window?** PASS - no. Rule (3) makes the fill an act, not
  a live lookup: a renamed or dropped model leaves the limits as they were and only the price falls to
  unpriced. The asymmetry with price is stated and argued in the revision note.
- **Can a limit block a turn?** PASS - no. FR-034a.5 unchanged; rule (6) restates it.
- **Did capabilities get filled by accident?** PASS - no. FR-034a.6 and FR-034a.7 keep `tool_call` and
  `image_input` as the entry's own fields, SC-022 counts 0 capabilities filled, and the revision note
  records the recommendation and the one-clause change that would extend it if the requester asks.
- **Did the ARN validation boundary move?** PASS - no. FR-023 and FR-024 are untouched; FR-034a.6 keeps its
  ARN-shape and region sentences word for word, and adds that a filled limit does not vouch for what the ARN
  points at.
- **Is a new stored field or migration needed?** PASS - no. `context` and `output` already exist on the
  entry; an entry with limits set keeps them, one without gains them the next time its pair is set.

#### Consequence for planning (recorded, not executed)

- **Round-trip hazard, worth a contract line**: if `GET` returns the resolved limits and the form `PUT`s them
  back, they must not be stored as hand-entered values - rule (1) forbids a filled value becoming the
  person's by being saved. Whether that is solved with the existing `edited` flag, per-field provenance, or
  by not accepting unchanged values back is planning's call.
