# Tasks: Encrypted Storage of Secrets at Rest

**Input**: Design documents from `/specs/031-encrypt-agent-secrets/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/encryption.md, contracts/migration.md, quickstart.md

**Tests**: included because this is a security migration whose success criteria are raw-database and failure-mode assertions; the project constitution also requires `cargo test`/`cargo clippy` and a dummyplug e2e gate.

**Organization**: tasks are grouped by the five user stories from spec.md. Phase 2 is the shared crypto/key/storage foundation; each story phase is independently testable after that foundation.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel with other [P] tasks in the same phase (different files, no incomplete dependency)
- **[Story]**: `US1`–`US5` map to the spec's user stories
- Exact file paths are in every task; quoted limits/formats come from `data-model.md` and `contracts/`

## Path Conventions

Single Rust project at the repository root: `src/`, `docs/`, `specs/031-encrypt-agent-secrets/`. No WebUI/design change.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: dependency and module wiring for the encryption feature.

- [ ] T001 Add `ring = "0.17"` and `zeroize = "1.9"` to `Cargo.toml` (both already in `Cargo.lock`), and add `mod encryption;` to `src/main.rs`; run `cargo check` to confirm no new crates are downloaded.
- [ ] T002 Create `src/encryption/mod.rs` with `mod cipher; mod fields; mod key; mod migrate;`, the `IntegrationSecretPaths` trait (`secret_paths(kind: &str) -> Option<Vec<String>>`), a no-op `NoIntegrationSecretPaths` returning `Some(Vec::new())` for tests/legacy import, re-exports, and `#[cfg(test)] mod storage_tests;`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the cipher, key primitives, field helpers, integration path registry and storage plumbing that every user story uses.

**⚠️ CRITICAL**: no user story work can begin until this phase is complete.

- [ ] T003 [P] Implement `SecretCipher` in `src/encryption/cipher.rs`: `ring::aead::CHACHA20_POLY1305`, a random 32-byte key, a fresh 12-byte random nonce per value, the envelope `enc:v1:<16-hex-key-id>:<base64url(nonce‖ciphertext‖tag)>`, `encrypt`/`decrypt`/`open`/`is_protected`, `from_key_for_test`, `disabled`, and the AAD builder `vizier:secrets:v1 \x1f <table> \x1f <record> \x1f <field>`. Unit tests: exact round-trip (Unicode/multi-line), randomized ciphertext, flipped byte/truncation, wrong key id, wrong AAD, unknown version, malformed base64, disabled mode.
- [ ] T004 [P] Implement key resolution in `src/encryption/key.rs`: precedence `VIZIER_ENCRYPTION_KEY` (base64 of exactly 32 bytes) → `VIZIER_ENCRYPTION_KEY_FILE` → generated `{workspace}/.runtime/encryption.key`; the two-line key file `vizier-key-v1\n<base64>`; `SystemRandom` generation; `0600` on Unix (warning elsewhere); never overwrite; malformed/missing explicit sources are errors, not fallbacks. Unit tests with temp dirs and explicit source inputs (no process-env races).
- [ ] T005 [P] Implement `src/encryption/fields.rs`: protect/decrypt helpers for `ProviderEntry` (`credentials.api_key`, `credentials.access_token` only), `CustomProvider` (`api_key` and every `headers` value; names stay readable), `AgentConfig` (`tools.shell.env` values for local/Docker; re-serialization drops the pre-016 keys; `core` preserved), `GlobalConfigEntry` (`value.data.env` values), and `integration.settings` via `IntegrationSecretPaths` (`Secret` = top-level key, `SecretMap` = every value; unknown kind = whole `settings` in one envelope). Non-string secret values and non-object known-kind settings are errors. Unit tests for each shape, sibling readability and AAD identity.
- [ ] T006 [P] Implement `IntegrationSecretPaths` for `IntegrationKinds` in `src/integrations/mod.rs`: derive paths from `IntegrationKind::listing().fields` — `FieldType::Secret` → the key, `FieldType::SecretMap` → `<key>.*`, everything else omitted; unknown kind returns `None`. Test the exact map for `discord` (`token`), `telegram` (`token`), `brave_search` (`api_key`) and `mcp` (`env.*`).
- [ ] T007 Add the cipher and path registry to `SqliteStorage` in `src/storage/sqlite/mod.rs`: keep `new(conn, document_store)` with `SecretCipher::disabled()` and `NoIntegrationSecretPaths` so existing constructors/tests compile; add `with_secrets(conn, document_store, cipher, paths)` for production and encryption tests. No field encryption yet.
- [ ] T008 Create `src/encryption/storage_tests.rs` with a test harness that builds a file-backed temp `SqliteStorage::with_secrets` using a fixed test key and `NoIntegrationSecretPaths`, plus helpers to read a raw column and scan the database/WAL bytes for a plaintext.

**Checkpoint**: the cipher, key primitives, field helpers, integration paths and storage construction compile; `cargo test` has the new unit tests.

---

## Phase 3: User Story 1 — A copy of the data is no longer a copy of the credentials (Priority: P1) 🎯 MVP

**Goal**: every in-scope secret value stored through any existing surface is authenticated ciphertext; non-secret siblings remain readable and all existing reads/writes work.

**Independent Test**: on a fresh workspace, set an agent secret through the HTTP API, save provider/custom-provider credentials, create Brave and MCP integrations, create an agent with shell env; stop vizier and confirm none of the values appears in `vizier.db`/`vizier.db-wal`, while `base_url`, `safesearch`, `command`, `args`, `model` and names remain readable; restart and retrieve the values.

### Implementation for User Story 1

- [ ] T009 [P] [US1] In `src/storage/sqlite/secret.rs`, encrypt `agent_secret.value` on `set_agent_secret` with AAD `agent_secret` + `agent_id` + `name`; decrypt on `get_agent_secret`; decrypt before masking in `list_agent_secrets`. Add tests that the raw column starts `enc:v1:`, a value round-trips byte-for-byte, and a tampered value returns an error instead of a mask/partial value.
- [ ] T010 [P] [US1] In `src/storage/sqlite/provider.rs`, encrypt `provider_config.credentials.api_key`/`access_token` and `custom_provider.api_key`/each `headers` value on upsert; decrypt on list/get; leave `endpoint`, `base_url`, `account_id`, custom-provider name/base URL/header names/models readable. Change list/get to propagate decryption errors with the provider id instead of silently dropping the row. Add raw-byte and sibling tests.
- [ ] T011 [P] [US1] In `src/storage/sqlite/integration.rs`, encrypt the kind-declared secret paths in `integration.settings` on `save_integration` and decrypt on `load`/`get`; unknown kind → whole-settings envelope; keep `transport`, `command`, `args`, `uri`, `safesearch`, names and enabled state readable. Confirm token-uniqueness via the decrypted `load`. Add Discord/Brave/MCP raw-byte, sibling and unknown-kind tests.
- [ ] T012 [P] [US1] In `src/storage/sqlite/agent.rs`, encrypt every `tools.shell.env` value (local and Docker) on create/update and decrypt on list/get; re-serialization drops the pre-016 `discord_token`, `telegram_token`, `tools.discord`, `tools.telegram`, `tools.brave_search` and `tools.mcp_servers` keys; preserve `core` for `migrate_agent_cores`; propagate decryption errors in `list_agents`/`get_agent`. Add raw-byte and sibling (model/prompt/shell path) tests.
- [ ] T013 [P] [US1] In `src/storage/sqlite/global_config.rs`, encrypt/decrypt every shell `env` value in `global_config.data` (`value.data.env.*`), leaving the key and non-env fields readable; add a raw-byte test. Do not delete the unused table.
- [ ] T014 [US1] In `src/encryption/storage_tests.rs`, add the cross-surface test: write an agent secret, provider key, custom provider key/header, Discord token, Brave key, MCP env value and shell env value; assert every raw stored secret is an `enc:v1:` envelope, every sibling is still readable, every storage read returns the exact plaintext, and `list_agent_secrets` masks correctly.
- [ ] T015 [US1] Update masking/redaction tests so they run on decrypted values: `src/storage/sqlite/secret.rs` (list mask), `src/integrations/kind.rs` (`redact`/`merge_secrets` for Discord/Brave/MCP), and `src/channels/http/api/v1/providers/custom.rs`/`providers` responses (no key/header value). No response shape changes.
- [ ] T016 [US1] Run the User Story 1 independent test from `specs/031-encrypt-agent-secrets/quickstart.md` §1–§3 against a scratch workspace with a dummyplug agent; record the raw-scan and sibling-read results in the task notes/PR body. Do not modify `quickstart.md` unless a command is wrong.

**Checkpoint**: on a fresh install, every in-scope value is protected at rest and every existing secret workflow still works.

---

## Phase 4: User Story 2 — Upgrading an existing installation is automatic and lossless (Priority: P1)

**Goal**: a pre-encryption database is converted on first start, idempotently, with no plaintext left in the database or WAL and no consumer running mid-conversion.

**Independent Test**: start a file-backed database containing plaintext rows in all six tables; run startup; every value still resolves, every row's secret field is an envelope, siblings are unchanged, and a byte scan of `vizier.db`/`vizier.db-wal` finds none of the original values. Restart: no conversion, no value change. Interrupt at each stage: the next start completes.

### Implementation for User Story 2

- [ ] T017 [US2] Create `src/encryption/migrate.rs`: `migrate_secrets_to_encrypted(conn, cipher, paths)` checks `migration.secrets_encrypted_v1` in `state`; sets `PRAGMA secure_delete=ON`; opens one transaction; exposes per-table conversion helpers and a `ConversionCount` summary; aborts on an unparseable row with the table/record id, never a silent skip.
- [ ] T018 [US2] Implement the six conversions in `src/encryption/migrate.rs`: `agent_secret.value`; `provider_config` credentials; `custom_provider` key/headers; `integration.settings` via `IntegrationSecretPaths` (unknown-kind whole settings); `agent_config` shell env plus legacy-key removal (`core` preserved); `global_config` shell env. Already-protected values are validated and skipped; plaintext values are encrypted. Add idempotency tests.
- [ ] T019 [US2] Implement the post-commit cleanup in `src/encryption/migrate.rs`: `PRAGMA wal_checkpoint(TRUNCATE)`, `VACUUM` (or `VACUUM INTO` + atomic replace if `document_index`/`memory_fts` rejects it), second checkpoint, then write the marker. Log per-table counts; a vacuum/checkpoint failure leaves the marker absent and fails startup. Add tests that FTS5 and `sqlite-vec` tables survive and that the database/WAL bytes contain none of the fixture plaintext.
- [ ] T020 [US2] Wire startup in `src/dependencies.rs`: open connection → load/validate cipher → `migrate_memory_to_bundles` → `migrate_filesystem_backend_to_sqlite` (change its `SqliteStorage::new` to `with_secrets` so imported values are protected on write) → `provider_migration::migrate_provider_ids_v1` → `migrate_secrets_to_encrypted` → construct the main `VizierStorage` with `with_secrets` → existing `migrate_users`/`migrate_agent_cores`/sweeps. Build the `IntegrationKinds`/path registry once and reuse it.
- [ ] T021 [US2] Add migration tests in `src/encryption/migrate.rs` (file-backed temp DB): all six tables plaintext→envelopes; sibling fields unchanged; marker written; second run converts zero and changes no row; `grep` of DB+WAL returns no plaintext; legacy agent keys gone; `core` still present; unknown integration kind decrypts; wrong key aborts before writes; interrupted before commit/after commit/before marker resumes.
- [ ] T022 [US2] Add a startup-order test/fixture (in `src/dependencies.rs` tests or `src/encryption/storage_tests.rs`) proving no storage read/write consumer runs before the migration completes, that `migrate_agent_cores` sees the preserved `core`, and that a second start is a no-op.

**Checkpoint**: an existing plaintext installation upgrades with no operator action and leaves no recoverable plaintext in the data files.

---

## Phase 5: User Story 3 — Key handling is automatic, controllable and fail-safe (Priority: P1)

**Goal**: fresh installs need no key setup; operators can supply a key; missing/wrong/malformed keys fail closed with distinct messages and never regenerate or modify data.

**Independent Test**: fresh workspace → key file created and secret round trip works; delete key → startup refused, no new key; restore key → works; set a wrong key → refused; set a malformed key → refused; supply a valid env key → used; read-only workspace → clear write error.

### Implementation for User Story 3

- [ ] T023 [US3] Complete key identity/validation in `src/encryption/key.rs`: write/read `state:secrets.key_id` (JSON string), compare it with the loaded key, fall back to scanning the protected columns for the first `enc:v1:` envelope if the state row is absent, and refuse to generate when a protected value exists. Distinct errors: not found, unreadable/malformed, does not match. Load must happen before any write that could add plaintext.
- [ ] T024 [US3] Add key failure tests in `src/encryption/key.rs`: missing key with protected data aborts; missing key with only plaintext generates; wrong key id aborts with no write; malformed env/file aborts; explicit missing key-file aborts (no generation); restoring the correct key succeeds; `secrets.key_id` is written once and reused.
- [ ] T025 [US3] Add permission/read-only tests in `src/encryption/key.rs`: generated file is `0600` on Unix; existing permissive file warns (and tightens where possible); an unwritable workspace returns a clear error; the unsupported-platform path warns once and continues.
- [ ] T026 [US3] Add a fresh-install/env-precedence test (in `src/encryption/key.rs` or `src/dependencies.rs` tests): zero key config → generated file and working agent; `VIZIER_ENCRYPTION_KEY` overrides the file; `VIZIER_ENCRYPTION_KEY_FILE` overrides the default path; an empty env value is malformed, not unset.

**Checkpoint**: every key lifecycle path is fail-closed and a fresh install remains zero-setup.

---

## Phase 6: User Story 4 — Backups, restores and moves between machines are predictable (Priority: P2)

**Goal**: operators know what to back up, how to restore, and what key loss means; data+key restores work and data-only restores fail with a clear message.

**Independent Test**: copy a workspace+key to another path, start and read a secret; copy without the key, start and get the documented failure; follow the documentation's backup/restore/key-loss steps exactly.

### Implementation for User Story 4

- [ ] T027 [P] [US4] Write `docs/src/configuration/encryption.md`: key sources/precedence, key file format/location, backup/restore (key separate from data), key loss (unrecoverable, no regeneration), threat model, what is not covered, downgrade rules, the 1.0 migration path and the shell-env visibility exception. Link `contracts/encryption.md` and `contracts/migration.md`.
- [ ] T028 [P] [US4] Update `docs/src/configuration/secrets.md` (replace the "no application-level encryption" paragraph at the end), `providers.md`, `channels.md`, `mcp.md` and `storage-shell.md` with "credentials/tokens are encrypted at rest; back up the key separately" notes and a link to `encryption.md`; add `encryption.md` to `docs/src/SUMMARY.md` (2.14 or the next free section).
- [ ] T029 [US4] Update `CLAUDE.md` with the new `src/encryption/` module, the key file/env sources, the storage-boundary rule, the shell-env exception, backup guidance and the #82 entry.
- [ ] T030 [US4] Validate and, if needed, correct `specs/031-encrypt-agent-secrets/quickstart.md` §4 (backup with/without key, wrong key) against the implemented error strings; add the release-note/downgrade sentence to `encryption.md` if not already there.

**Checkpoint**: an operator can perform a correct backup/restore and understands key loss without asking for help.

---

## Phase 7: User Story 5 — Stored secrets remain hidden everywhere except their intended consumer (Priority: P2)

**Goal**: encryption does not weaken the existing non-disclosure guarantees; failures and diagnostics never expose a value, ciphertext or key.

**Independent Test**: create/read/list/replace/delete every secret type, force a wrong-key startup and a tampered row, then search logs, telemetry, error messages, API/WebUI responses and support output for the values, ciphertext and key; none appears. The shell-env exception stays documented and limited to the agent config response.

### Tests and hardening for User Story 5

- [ ] T031 [US5] Audit every new error/log path in `src/encryption/cipher.rs`, `key.rs`, `migrate.rs`, `src/storage/sqlite/*.rs` and `src/dependencies.rs`; ensure messages contain only table/record/field identifiers and no plaintext, ciphertext or key. Add unit assertions that `format!("{err}")` for wrong-key, tamper, malformed and migration errors excludes the fixture plaintext/envelope.
- [ ] T032 [US5] Ensure `src/storage/sqlite/secret.rs`, `provider.rs`, `integration.rs` and `agent.rs` propagate decryption errors from list/get instead of `filter_map`-dropping the row; add one test per surface that a tampered row produces an error, not an empty/short list.
- [ ] T033 [US5] Re-run the existing agent secret redaction path on encrypted storage: `src/secrets/` (`get`/`set`/`redact`) and `src/agents/tools/secrets.rs` tests; confirm a retrieved value is still `[redacted]` in the report, stdout, traceback and nested call arguments; no behavior change.
- [ ] T034 [US5] Add a test in `src/storage/sqlite/agent.rs` (or the agent API tests) that shell env values are envelopes at rest but still appear in the authorized agent-config response, pinning the documented exception; confirm no other protected value is returned by a management response.
- [ ] T035 [US5] Add the diagnostic scan to `specs/031-encrypt-agent-secrets/quickstart.md` and run it: search `/tmp/vizier-encryption.log`, error responses and support output for the test values and the key; expect no matches. Do not scan session history for agent-set values (documented set-input exception).

**Checkpoint**: the feature's failure paths are non-disclosing and no existing redaction regressed.

---

## Final Phase: Polish & Cross-Cutting Concerns

**Purpose**: tracking, verification and release hygiene.

- [ ] T036 Add the #82 checklist entry for `migrate_secrets_to_encrypted`, `migration.secrets_encrypted_v1`, the plaintext branch/`open`, the pre-016 agent-key scrub, the unknown-kind whole-settings fallback if it exists only for pre-031 data, and their tests; explicitly exclude the cipher, key sources, key id, AAD, envelope version reader and field helpers. Edit GitHub issue #82 (outward-facing; confirm with the user first).
- [ ] T037 Add a separate #82 note for the unused `global_config` table/reader cleanup in GitHub issue #82 (do not delete it in this feature), and update the spec's checklist/tracker reference if needed.
- [ ] T038 Run `cargo fmt`, `cargo clippy` and `cargo test`; fix all failures in `src/encryption/`, `src/storage/sqlite/`, `src/dependencies.rs` and their tests. Confirm no `unwrap`/`expect` outside tests and no `println!` in library code.
- [ ] T039 Run `specs/031-encrypt-agent-secrets/quickstart.md` §1–§6 end to end with a dummyplug agent: fresh key, agent set/get, API-set secret, provider/custom/integration/shell env, raw DB+WAL scan, sibling reads, restart, backup/restore, missing/wrong key, migration simulation, tamper and diagnostics scan.
- [ ] T040 Confirm the diff contains no `webui/`, `design/`, route, DTO or OpenAPI changes; no `scripts/openapi.mjs` regeneration is needed. Run `git diff --stat` over `src/`, `docs/`, `webui/` and `design/`; it should show only `src/`, `Cargo.toml`, `docs/`, `CLAUDE.md` and the spec artifacts.
- [ ] T041 Security review against the contract: key file never in DB, key not in logs/errors, no plaintext in DB/WAL after migration, wrong key writes nothing, tampered value fails, siblings readable, shell-env exception only where documented. Review `src/encryption/`, `src/storage/sqlite/`, `src/dependencies.rs` and `docs/src/configuration/encryption.md`; record findings in the PR description.
- [ ] T042 Commit with a conventional `[**breaking**]` message (downgrade requires a pre-upgrade backup); include the #82 reference, the key-backup warning and the release-note sentence naming the last pre-1.0 migration point. Stage `Cargo.toml`, `src/encryption/`, `src/storage/sqlite/`, `src/dependencies.rs`, `src/integrations/`, `docs/`, `CLAUDE.md` and `specs/031-encrypt-agent-secrets/`.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001 starts immediately; T002 follows T001.
- **Foundational (Phase 2)**: T003–T008 depend on T001/T002 and block all user stories.
- **US1 (Phase 3)**: T009–T016 after Foundational; T014 depends on T009–T013; T015 depends on T009/T011; T016 depends on T009–T015.
- **US2 (Phase 4)**: T017–T022 after Foundational; T018 depends on T017; T019 depends on T018; T020 depends on T017–T019; T021 depends on T018–T019; T022 depends on T020.
- **US3 (Phase 5)**: T023 after T004/T007; T024–T026 depend on T023.
- **US4 (Phase 6)**: T027/T028 can start once the contract is stable (after T003–T008); T029 after T027/T028; T030 after the implemented error strings (US3).
- **US5 (Phase 7)**: T031–T035 after US1 and US3; T035 depends on the full quickstart path.
- **Polish**: T036–T042 after all desired stories.

### User Story Dependencies

- **US1**: no dependency on US2–US5 once Foundational is done.
- **US2**: uses the field helpers from Foundational and the key validation from T023; can proceed in parallel with US1's per-surface storage work, but its end-to-end test needs US1's storage read paths for consumer checks.
- **US3**: independent of US1/US2 field coverage; its startup failure tests need at least one protected value, which the compact storage harness from T008 provides.
- **US4**: documentation depends on the final key/migration behavior; can draft earlier.
- **US5**: depends on US1's storage paths and US3's key errors.

### Within Each User Story

- Write or update the test for a behavior before/with the implementation task that introduces it; tests must fail first where practical.
- Field helpers and key primitives before storage usage.
- Storage read/write before cross-surface/redaction checks.
- Migration conversion before vacuum/marker/wiring.
- Key validation before fresh-install/env tests.
- Docs after the behavior they describe.

---

## Parallel Example: User Story 1

```bash
# Different files, no shared edits:
Task: "T009 [US1] Encrypt agent_secret.value in src/storage/sqlite/secret.rs"
Task: "T010 [US1] Encrypt provider/custom-provider fields in src/storage/sqlite/provider.rs"
Task: "T011 [US1] Encrypt integration settings in src/storage/sqlite/integration.rs"
Task: "T012 [US1] Encrypt agent shell env in src/storage/sqlite/agent.rs"
Task: "T013 [US1] Encrypt global shell env in src/storage/sqlite/global_config.rs"
```

## Parallel Example: User Story 2

```bash
# T017 must precede these; T018/T019 touch the same file, so serialize those two.
Task: "T021 [US2] Migration file-backed tests in src/encryption/migrate.rs"
Task: "T022 [US2] Startup-order/core-preservation test in src/dependencies.rs"
```

## Parallel Example: User Story 3

```bash
Task: "T024 [US3] Key failure tests in src/encryption/key.rs"
Task: "T025 [US3] Permission/read-only tests in src/encryption/key.rs"
Task: "T026 [US3] Fresh-install/env-precedence test in src/encryption/key.rs"
```

## Parallel Example: User Story 4

```bash
Task: "T027 [US4] Write docs/src/configuration/encryption.md"
Task: "T028 [US4] Update secrets/providers/channels/mcp/storage docs and SUMMARY"
```

---

## Implementation Strategy

### MVP First (User Story 1 + the key foundation)

1. Complete Phase 1 (Setup) and Phase 2 (Foundational).
2. Complete Phase 3 (US1): all in-scope values are encrypted on a fresh install, with raw-byte and sibling checks.
3. Validate US1 independently with the quickstart §1–§3 before touching migration or docs.
4. This is the MVP for a fresh install. Do not ship to an existing deployment without US2.

### Incremental Delivery

1. Setup + Foundational → cipher/key/storage plumbing ready.
2. US1 → fresh-install encryption and transparent reads. Validate.
3. US3 → fail-closed key lifecycle and zero-setup proof (if not already covered by the foundational key tests). Validate.
4. US2 → automatic upgrade of existing plaintext installations and the no-plaintext/VACUUM guarantee. Validate on a copy of a pre-encryption fixture.
5. US4 → encryption/backup/key-loss docs and the operator-facing release note.
6. US5 → non-disclosure audit, tamper propagation and the shell-env exception pin.
7. Polish → #82, full quickstart, clippy/test, security review, breaking commit.

### Parallel Team Strategy

After Foundational:
- Developer A: US1 storage surfaces (T009–T016).
- Developer B: US2 migration (T017–T022), using the Foundational field helpers.
- Developer C: US3 key validation (T023–T026).
- US4 docs can start once the contracts/behavior are stable; US5 is a review pass after US1/US3.

Do not parallelize two edits to `src/encryption/migrate.rs` (T018/T019), `src/storage/sqlite/provider.rs` (T010/T015) or `src/encryption/key.rs` (T023–T026) without coordinating.

---

## Notes

- `[P]` tasks are different files and have no incomplete dependency.
- Every task names the exact file it changes.
- Tests are part of the implementation tasks because raw-database, migration and failure-mode behavior cannot be verified by reading code; the security review in T041 depends on them.
- The only temporary code is the migration/plaintext branch and legacy scrub (T017–T022); the cipher, key handling, AAD and versioned reader are permanent.
- The shell environment values are encrypted at rest but remain visible to authorized agent managers; T034 pins that exception instead of silently changing the API.
- No WebUI, design-system or OpenAPI task exists because the feature changes storage only; T040 verifies that.
