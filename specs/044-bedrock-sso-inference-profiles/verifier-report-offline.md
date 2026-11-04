# Verifier report - offline walk of quickstart §1-§2b (T075)

**Feature**: `044-bedrock-sso-inference-profiles` | **Walked by**: implementer (agent), no AWS account
**Date**: 2026-11-03 | **Branch**: `044-bedrock-sso-inference-profiles`

## How this was walked, and what it is not

The quickstart's §1-§2b are written as `curl`/`sqlite3` steps against a running binary. On this
round a `just check` run held the Cargo build lock, so no server was started and no request was
issued. The walk was done the way T075 allows: every expected behaviour below is tied to a real
assertion in the committed tests or to the exact source that produces it, by `file:line`. Where a
claim needs a live workspace, a built binary or an AWS account it is marked **not run**, not
"should pass".

A citation is `path:line`. Test names are the committed `#[test]`/`#[tokio::test]` functions, so
`cargo test --workspace` either runs them or fails.

## §1 Storage and encryption

| Step | Expected | Evidence | Verdict |
|---|---|---|---|
| 1.1 fresh workspace gains the table | `bedrock_sso_session` exists | `crates/vizier-core/src/storage/sqlite/mod.rs:310` (`CREATE TABLE IF NOT EXISTS bedrock_sso_session`), applied on open; the HTTP test `a_saved_connection_reports_its_fields_and_a_configured_session` proves a freshly opened workspace answers `SELECT COUNT(*) FROM bedrock_sso_session` (`crates/vizier-http/src/api/v1/providers/mod.rs:655-660`). No separate migration: the schema is created at open, so there is no version gate to miss. | pass (code + test). The literal `.tables` shell step not run - no binary. |
| 1.1b the row is an envelope, not plaintext | `enc:v1:` in `data` | `storage/sqlite/sso_session.rs:199` `a_written_session_reads_back_and_is_stored_as_an_envelope`; `schema/sso.rs:1-12` (`SsoSessionSecret` derives no `Serialize`; one writer, `encryption::fields::protect_sso_session`). | pass (test) |
| 1.2 restart with a live session keeps it | `signed-in`, same expiry | Not walked: needs a built binary and two starts. That is T078, still open. | **not run** |
| 1.3 wrong key fails closed | `sso_state = unreadable`, message names the profile, `200`, rows/profiles/agents intact | `llm/sso/session.rs:377` `an_unreadable_row_says_so_and_is_left_alone` asserts `SessionUnreadable { profile }` and `storage.len() == 1` (nothing deleted, SC-019); `storage/sqlite/sso_session.rs:226` `another_profiles_envelope_does_not_decrypt`; the state is produced by `service/providers.rs:150-163` (`Err(_) => SsoView { state: Unreadable, ..configured }`, `has_session: false`, no expiry). | partial: decryption failure and non-deletion proven by test; the `200` GET carrying `unreadable` is only traced in code - no test asserts that projection. |

## §2 Session storage and the API surface

| Step | Expected | Evidence | Verdict |
|---|---|---|---|
| 2.1 `PUT` six fields, no session | `200`, `sso_state = configured` | `crates/vizier-http/src/api/v1/providers/mod.rs:608` - asserts `StatusCode::OK`, `sso_state == Configured`, `!has_sso_session`, `session_expires_at.is_none()`, and `SELECT COUNT(*) FROM bedrock_sso_session == 0` ("saving the five fields signs nobody in"). | pass (test) |
| 2.2 `GET` carries no token/secret | `clean` | Same test, lines 646-650: serialises the row and asserts none of `"access_token":`, `"refresh_token":`, `"client_secret":`, `"device_code":` appear; `service/providers_sso.rs:992` `no_sign_in_answer_carries_a_device_code_token_or_secret_half`; `llm/sso/error.rs:273` `no_variant_can_print_a_token_or_a_secret`; `llm/sso/client.rs:729` `the_registration_and_the_flow_never_print_their_secret_half`. | pass (tests) |
| 2.3 sign-out → configured, row gone, profiles intact | state/rows | `llm/sso/session.rs:399` `signing_out_deletes_the_row_and_a_second_sign_out_is_not_an_error`; `storage/sqlite/sso_session.rs:315` `signing_out_twice_is_not_an_error`; `providers_sso.rs:1048` `sign_out_needs_no_region_and_no_saved_row`; HTTP `providers/mod.rs:589` `signing_out_when_nothing_is_signed_in_is_not_an_error_and_echoes_no_secret`. "Profiles intact" follows from sign-out being a delete of the session row only (`storage/sqlite/sso_session.rs`), exercised end-to-end by the entries test below. | pass (tests) |
| 2.4 duplicate `arn` → `400` naming it, first keeps its name | refusal + no overwrite | `service/providers_sso.rs:1239` `the_duplicate_is_judged_on_the_parsed_arn_and_the_region_only_after_it` (compares parsed ARNs, so padding differs but the profile does not). | pass (test) |
| 2.5 entry region != `region` → refused, both regions named | refusal text | `providers_sso.rs:1118` `the_shape_is_judged_before_the_duplicate_which_is_judged_before_the_region` fixes the order; `:1239` and `:1166` assert the message. | pass (test) |
| 2.6 six malformed ARNs refused with the shape message, nothing stored | `400`, value + expected shape | `providers_sso.rs:1166` `every_arn_refusal_names_the_value_and_the_expected_shape` runs 10 cases - the six the quickstart names (bare model id, another service `s3`, foundation-model ARN, console URL, truncated, bad region) plus empty, short account, no profile id, bad partition. Each asserts `status() == 400`, that the message starts with the quoted trimmed value and contains the specific cause, and that the incoming rows were **not** half-normalised (`arn`/`name`/`model` unchanged). A second block proves the first bad row is named, not the last. | pass (test) |
| 2.7 one ARN of each resource type, all five partitions, accepted unchanged | each echoed back | `llm/bedrock.rs:380` `both_resource_types_parse_in_every_partition` - 5 partitions x 2 resource types, asserting `partition`, `region`, `account`, `resource`, `id` and `raw` (`raw` is the input, so it is echoed unchanged); `:414` `the_parsed_region_is_the_arn_segment`; `:427` `surrounding_padding_is_trimmed_once`. | pass (test) |
| 2.8 an ARN as an embedding model → refused as chat-only, both forms | refusal before the family check | `llm/media/bedrock.rs:468` `an_inference_profile_arn_is_refused_before_the_family_check` loops `[SYSTEM_ARN, APPLICATION_ARN]`, asserts the grammar accepts both first, then that `BedrockEmbeddings::new` and `BedrockImages::new` both refuse; `:513` `the_refusal_names_both_arn_forms_by_the_generic_noun` asserts the message starts "an inference-profile ARN is a chat model" and never says "application-inference-profile". The guard is `refuse_inference_profile_arn` at `:31`, called at `:104` (embedding) and `:248` (images), before `EmbeddingFamily::detect`. | pass (test) |

## §2b Entry pricing (five outcomes, no AWS)

The entry map is a pure, synchronous function of the loaded catalogue and the entry -
`service/catalogue.rs:734` `pub fn entry_pricing(catalogue: &Catalogue, entry: &ProfileEntry) -> EntryPricing`.
There is no client, no endpoint and no `await` in it, which is the structural half of FR-034c; the
behavioural half is that the resolver carries no pricer and no price (`llm/resolve.rs:1369`,
`a_missing_or_stale_price_never_blocks_a_turn`).

| Step | Expected | Evidence | Verdict |
|---|---|---|---|
| 2b.1 unpaired entry | `cost: null`, `not_set`, flag "pricing not set", non-empty reason naming no model + the two ways out | `service/catalogue.rs:994` `entry_pricing_follows_the_precedence_the_data_model_fixes` - `NotSet` for `None` and for `Some("   ")`, `flag == Some("pricing not set")`, `reason` non-empty, `cost() == None`; `:1169` `every_absence_names_its_own_cause_and_a_fix_clears_it` asserts the reason contains "no model is paired with this ARN" and the way out ("pick one" / "enter a price by hand"). | pass (test) |
| 2b.1b the turn succeeds and the usage row is unpriced, not zero | row recorded, `cost_usd: None` | `crates/vizier-core/src/usage/mod.rs:1244` `an_unpriced_or_stale_arn_still_records_its_row` - both rows present with provider and ARN, tokens = input+output, `cost_usd == None` and `!row.priced`; summary is `None` ("nothing was costed at zero"); both ARNs listed under `unpriced.models`. | pass (test) |
| 2b.2 paired entry | `from_catalogue`, `cost` = the model's catalogue cost | `catalogue.rs:994` - `FromCatalogue`, `state == "from_catalogue"`, `cost().is_some()`, no flag, no reason; the model list surface is `service/catalogue.rs:1324` `the_sso_model_list_puts_entries_first_with_their_pricing`. | pass (test) |
| 2b.3 explicit price wins | `explicit`, figures as entered, `model` kept | `catalogue.rs:994` - `Explicit(hand)` for `model` in `[None, Some("amazon.nova-lite-v1:0"), Some("gone-model")]`, flag and reason both `None` (no lookup happens); `:1169` keeps the pairing after a hand price is set. | pass (test) |
| 2b.4 stale id | accepted, `not_resolved`, `model` echoed, reason contains the id, still chats | `catalogue.rs:994` - `NotResolved { model: "anthropic.retired-model-v1", in_catalogue: false }`, `flag == "pricing not resolved"`, reason contains the id, `cost()` `None`; `:1169` proves the three reasons differ and each keeps the way out; `:1250` `a_listed_model_with_no_cost_is_not_reported_as_gone`; usage side `usage/mod.rs:1244` (stale pairing still recorded, unpriced). | pass (test) |
| 2b.5 refresh reprices, never renames | name/arn/model byte-identical, cost re-resolved, explicit unchanged | `catalogue.rs:1083` `a_catalogue_refresh_relabels_and_reprices_an_entry_without_rewriting_it`. | pass (test) |
| 2b.5b no catalogue | `no_catalogue` + "pricing not set", wins over `not_set` and `not_resolved`, both variants | `catalogue.rs:1169` - `Catalogue::new(CatalogueData::empty())`, `absent.state() == "no_catalogue"` for an entry with **no** model and for one with a valid model, both reasons equal, and the reason contains "carries no Bedrock model list"; `:994` repeats it for `[None, valid, gone]` and proves `Explicit` still outranks it; `:1169` also asserts the three reasons are pairwise distinct. | pass (test) |
| 2b.6 pairing never validates or fills limits | `limit.context` stays `null`; region rule unchanged | `llm/resolve.rs:1334` `an_unset_entry_field_is_unknown_and_never_copied_from_the_paired_model` - an entry paired with a 300k-context model resolves `(context_window, context_source) == (None, Unknown)`, `max_output == None`, `tool_call == None`, `image_input == None`, `cost == None`; `providers_sso.rs:1270` `the_pricing_pairing_is_shape_checked_and_never_touches_the_arn_or_the_limits`. | pass (test) |
| 2b.7 no AWS call from a pricing read/dropdown/usage report | no `bedrock-runtime` request | `entry_pricing` is `fn`, not `async fn`, and takes no client (`catalogue.rs:734`); `ResolveContext` carries no pricer (`resolve.rs:1369`); the usage report is built from stored rows plus the catalogue (`usage/mod.rs:1244`). | pass by construction. The trace grep itself not run - no server. |

## What was not verified here

- Every `curl` step was executed as a **test** of the same handler, not as an HTTP request against a
  running binary. The handler is the same function, but the wire (auth header, JSON shape, status
  line) is unproven this round.
- §1.2 (restart) -> T078. §3 (dummyplug e2e) -> T076. §4 (secret grep on a real workspace/log) ->
  T077. §5 (live Identity Center) -> release item, [release-items.md](./release-items.md).
- §1.3's `200` + `unreadable` projection is traced in `service/providers.rs:150-163` but has no
  assertion; recommend a test on `sso_view` in the next round (not a code change).

