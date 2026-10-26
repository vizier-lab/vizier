# Implementation Plan: Encrypted Storage of Secrets at Rest

**Branch**: `031-encrypt-agent-secrets` | **Date**: 2026-10-26 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/031-encrypt-agent-secrets/spec.md`

## Summary

Stop storing credentials in plaintext in the embedded SQLite database. Add a small, field-level encryption layer used by the storage boundary: agent secret values, provider API keys and access tokens, custom-provider keys and header values, the Discord/Telegram tokens, the Brave Search key, MCP environment values, and shell environment values are stored as authenticated ciphertext; every non-secret sibling (provider base URL, integration command/args/URL, safe-search flag, shell path, agent model/prompt, names, timestamps, masks) stays readable.

The workspace gets one key, resolved from `VIZIER_ENCRYPTION_KEY`, `VIZIER_ENCRYPTION_KEY_FILE`, or an auto-generated `.runtime/encryption.key`; a fresh install needs no setup and no prompt. Missing, malformed or wrong keys fail closed with distinct messages and never regenerate over data. Existing plaintext is converted once at startup, idempotently, and the old bytes are reclaimed with `secure_delete` + `VACUUM` + WAL truncation. The conversion and its plaintext branch are temporary and tracked in #82; the cipher, key handling and versioned reader are permanent.

No HTTP, OpenAPI or WebUI surface changes: masking and redaction run after decryption exactly as they do now. The one documented exception is shell environment values, which remain visible to people allowed to view/edit an agent, as today.

## Technical Context

**Language/Version**: Rust edition 2024 (single `vizier` binary; tokio multi-thread; rusqlite 0.40 with bundled SQLite).

**Primary Dependencies**: `ring = "0.17"` (ChaCha20-Poly1305 AEAD + system CSPRNG) and `zeroize = "1.9"` (clear raw key material), both already present in `Cargo.lock` through rustls/ureq/other dependencies; existing `base64 = "0.23"`, `sha2 = "0.11"`, `serde`/`serde_json`, `rusqlite`, `tracing`. No new crate, no new external service, no new build step.

**Storage**: Embedded SQLite. No new table and no new column. Protected values stay in their existing TEXT columns as `enc:v1:<key-id>:<base64url(nonce‖ciphertext‖tag)>`; the existing `state` table gains `secrets.key_id` (permanent) and `migration.secrets_encrypted_v1` (temporary).

**Testing**: `cargo test` (cipher/key parsing and precedence, envelope tamper/wrong-AAD/wrong-key, per-surface storage raw-byte checks, migration on a file-backed temp DB including `VACUUM`, key failure modes, legacy scrub); `cargo clippy`; end-to-end: dummyplug steps in [quickstart.md](./quickstart.md) §1–§6, including a raw database/WAL scan, restart, backup/restore, missing/wrong key and tamper checks. Discord/Telegram live connection checks are marked optional/live; no live model output is needed.

**Target Platform**: Linux, macOS and Windows via the single binary; Docker. The only platform-specific code is the Unix `0600` key-file permission; elsewhere creation succeeds with a warning.

**Project Type**: Single-binary Rust service with an embedded WebUI. No WebUI or design-system change.

**Performance Goals**: AEAD per field adds microseconds; secret reads remain visually instantaneous (SC-009). Agent/provider/integration configs are read at build/refresh time, not per token. The one-time migration and `VACUUM` may take seconds on a large database; log progress and counts, never values. Startup must stay within the existing budget aside from that one pass.

**Constraints**: Zero-setup, headless, offline, single binary (constitution III); no OS keyring, passphrase prompt or network key fetch; no whole-database encryption; no new API; no key/value in logs, telemetry, errors or support output; field-level only, siblings readable; no key rotation in this release; downgrade is unsupported without a pre-upgrade backup.

**Scale/Scope**: One new `src/encryption/` module (cipher, key source, field helpers, migration), changes to five SQLite storage modules plus `integrations` and `dependencies`, one trait (`IntegrationSecretPaths`), two `state` keys, a Cargo dependency line, docs and tests. No WebUI/design work.

## Constitution Check

*GATE: must pass before Phase 0 research; re-checked after Phase 1 design.*

| Principle | Assessment |
|---|---|
| **I. Lean by default** | PASS. No new crate (`ring`/`zeroize` are already in the tree), no new table, no new service, no new API. The cipher is a concrete struct, not a trait. Field-level protection avoids encrypting data that has no security value. The only new abstraction, `IntegrationSecretPaths`, exists to keep storage from importing the integration registry and to derive paths from the existing `FieldKind` declarations; it is not a speculative second implementation. |
| **II. DRY via trait-based extensibility** | PASS. `IntegrationSecretPaths` is implemented once by `IntegrationKinds` and reads `IntegrationKind::listing().fields`; no `match` over kind ids in storage. A new kind declares `Secret`/`SecretMap` once for redaction, merge and encryption. AAD and field helpers live in one module used by both storage and migration. `SqliteStorage` remains the single storage implementation; `VizierStorage` forwarding is unchanged. |
| **III. Self-contained runtime** | PASS. The key is local (generated file or operator-supplied env/file); no external key service, network fetch, keyring or sidecar. SQLite remains embedded. A fresh `vizier run` works with zero setup. |
| **IV. Portability by default** | PASS. Crypto and envelope code are platform-neutral. Only key-file permissions are `cfg(unix)`-isolated; unsupported platforms get a warning, not a failure. No new native dependency or cross-compilation change; `ring` is already built for every target in `Cross.toml`. |
| **V. Unified errors & observability** | PASS. Storage/encryption return `anyhow::Result` per the existing storage convention; startup errors are actionable and distinct; all logging is `tracing`, counts and record ids only, never values or key material. No `unwrap`/`expect` outside tests. |
| **Workflow gates** | PASS. `cargo clippy`, `cargo test`, dummyplug quickstart and docs are in the phases. A #82 entry is mandatory for the one-time migration, plaintext branch, legacy scrub and tests; permanent cipher/key handling are excluded. The change is `[**breaking**]` because downgrade cannot read protected data. |

**Post-design re-check**: no violations. Watch items: (a) normal reads must require envelopes after the marker — no plaintext fallback; (b) migration must vacuum **and** checkpoint before writing the marker; (c) key load must happen before any write that could add plaintext; (d) shell-env values stay readable through the agent configuration API (documented exception); (e) unknown integration kinds must not be skipped — whole-settings fallback; (f) no key id or value in logs beyond the identifier.

## Project Structure

### Documentation (this feature)

```text
specs/031-encrypt-agent-secrets/
├── plan.md                       # this file
├── research.md                   # R1–R17 decisions and alternatives
├── data-model.md                 # entities, envelope, field matrix, AAD, state machine
├── quickstart.md                 # dummyplug + raw-DB validation, key-loss/backup/tamper
├── contracts/
│   ├── encryption.md             # key sources, envelope, AAD, errors, field contract
│   └── migration.md              # ordering, scope, marker, vacuum, legacy, #82
├── checklists/
│   └── requirements.md
└── tasks.md                      # /speckit-tasks (not created here)
```

### Source Code (repository root)

```text
Cargo.toml                        # + ring, zeroize (both already in Cargo.lock)

src/
├── main.rs                       # + mod encryption
├── encryption/
│   ├── mod.rs                    # SecretCipher, KeySource, IntegrationSecretPaths re-exports
│   ├── cipher.rs                 # ring ChaCha20-Poly1305, envelope, key id, AAD helpers
│   ├── key.rs                    # env/file/default resolution, generate, 0600 (cfg unix)
│   ├── fields.rs                 # provider/custom/agent/global/integration field helpers + AAD
│   └── migrate.rs                # startup conversion, secure_delete/checkpoint/vacuum/marker
├── integrations/
│   └── mod.rs                    # impl IntegrationSecretPaths for IntegrationKinds
├── storage/
│   ├── sqlite/
│   │   ├── mod.rs                # SqliteStorage fields + new/with_secrets; schema unchanged
│   │   ├── secret.rs             # encrypt value on set; decrypt on get/list
│   │   ├── provider.rs           # provider_config + custom_provider field encryption
│   │   ├── integration.rs        # encrypt/decrypt settings via secret paths; unknown-kind fallback
│   │   ├── agent.rs              # agent_config shell env; legacy keys dropped; propagate decrypt errors
│   │   └── global_config.rs      # global shell env field encryption
│   ├── secret.rs                 # doc comment: values are protected at rest
│   └── ...                       # no trait/schema changes
└── dependencies.rs               # key load, cipher-enabled storage, migration order, main storage

docs/src/
├── SUMMARY.md                    # + encryption page
├── configuration/
│   ├── encryption.md             # NEW: key sources, backup/restore, key loss, threat model
│   ├── secrets.md                # replace "no application-level encryption" paragraph
│   ├── providers.md              # credentials are encrypted at rest; key backup link
│   ├── channels.md               # bot tokens are encrypted at rest
│   ├── mcp.md                    # env values are encrypted at rest
│   └── storage-shell.md          # workspace/key backup note
CLAUDE.md                         # architecture: encryption module, key file/env, #82 entry
```

**Structure Decision**: use the existing single-binary layout. Encryption belongs at the storage boundary so every existing reader/writer (HTTP, agent tools, channel readers, MCP, migrations) goes through it without touching call sites. The integration registry supplies secret paths; no WebUI/design change is needed. No new table or endpoint.

## Phases

### Phase 0 — Research (complete)

Decisions R1–R17 in [research.md](./research.md): field-level scope, `ring` AEAD + AAD/envelope, key sources and fail-closed validation, storage-boundary injection, integration paths, migration/vacuum, legacy handling, dependency reuse, docs/backup, #82, no new table, no API/UI change.

### Phase 1 — Design artifacts (complete)

[data-model.md](./data-model.md), [contracts/encryption.md](./contracts/encryption.md), [contracts/migration.md](./contracts/migration.md) and [quickstart.md](./quickstart.md). Post-design constitution re-check above.

### Phase 2 — Crypto and key foundation

1. Add `ring`/`zeroize` to `Cargo.toml`; add `mod encryption;` to `main.rs`.
2. `encryption/cipher.rs`: `SecretCipher` (`Arc`-backed `LessSafeKey`, key id, source, disabled mode), `encrypt`/`decrypt`/`open`/`is_protected`, envelope parse/format, random nonce, AAD builder, key-id derivation. Unit tests: round-trip, randomized output, tamper, wrong key id, wrong AAD, malformed/unknown version, disabled mode.
3. `encryption/key.rs`: precedence env → explicit file → default generated file; two-line key file format; 32-byte validation; generate with `SystemRandom`; owner-only permissions (`cfg(unix)`), warning otherwise; never overwrite; error taxonomy. Tests with tempdirs and explicit source injection (avoid process-env races).
4. `encryption/mod.rs`: re-exports; `IntegrationSecretPaths` trait; `NoIntegrationSecretPaths` for tests/legacy.
5. `Integrations`: implement `IntegrationSecretPaths` from `IntegrationKind::listing().fields` (`Secret` → key, `SecretMap` → `key.*`); unit test the mapping for all four current kinds.

### Phase 3 — Field-level storage

1. `SqliteStorage`: add cipher and secret-path fields; keep `new(conn, document_store)` with disabled cipher/no paths; add `with_secrets(conn, document_store, cipher, paths)`; production uses the latter. No schema change.
2. `encryption/fields.rs`: typed helpers for `AgentConfig` shell env, `ProviderEntry` credentials, `CustomProvider` key/headers, `GlobalConfigEntry` shell env, and `integration.settings` via paths; every helper has a matching decrypt and uses the AAD builders. A known kind with a non-string secret value or a non-object settings value is an error; unknown kind gets whole-settings envelope. `AgentConfig` re-serialization drops the pre-016 keys.
3. `storage/sqlite/secret.rs`: encrypt on set, decrypt on get/list; lists fail on a tampered value (no silent skip); raw DB assertion test.
4. `storage/sqlite/provider.rs`: encrypt `api_key`/`access_token` on write and decrypt on read for both provider tables; change list/get to propagate decryption errors with the provider id; sibling-field raw test.
5. `storage/sqlite/integration.rs`: encrypt/decrypt settings in `save_integration`, `load`, `get`; token uniqueness already uses the decrypted `load`, so no extra query change; unknown-kind whole-blob path; raw test with Discord/Brave/MCP settings.
6. `storage/sqlite/agent.rs`: encrypt/decrypt `tools.shell.env`; propagate decryption errors in `list_agents`/`get_agent`; re-serialization drops legacy keys; raw test preserving model/prompt/shell path.
7. `storage/sqlite/global_config.rs`: encrypt/decrypt shell env values; raw test.
8. Update doc comments in `storage/secret.rs`, `storage/provider.rs`, `storage/integration.rs`, `storage/agent.rs` and `docs` references so the source no longer claims plaintext storage.

### Phase 4 — Startup migration and wiring

1. `encryption/migrate.rs`: `migrate_secrets_to_encrypted(conn, cipher, paths)`; marker check; `secure_delete=ON`; one transaction over the six tables; validate already-protected values; encrypt plaintext; re-serialize agent configs (legacy keys gone, `core` preserved); write marker only after commit + `wal_checkpoint(TRUNCATE)` + `VACUUM` + checkpoint; idempotent; per-record error messages; counts logged. Verify `VACUUM` with `memory_fts` and `document_index`; if the `sqlite-vec` virtual table rejects it, use the `VACUUM INTO` + atomic replace fallback from research R7 and assert the same no-plaintext post-condition.
2. `dependencies.rs`: load/validate cipher after opening the connection; pass it and the path registry into the filesystem migration's storage; run `provider_migration` then the encryption migration; construct the main `VizierStorage` with `with_secrets`; write `secrets.key_id`; keep the rest of the startup order. Add tests for fresh/plaintext/partial/wrong-key paths.
3. `migrate_filesystem_backend_to_sqlite`: use `with_secrets` so imported values are protected on write; no other behavior change.
4. Ensure `migrate_agent_cores` still sees `core` after the encryption pass (test), and that a second start is a no-op.
5. Add the #82 entry and note the `global_config` cleanup candidate.

### Phase 5 — Docs, verification and release

1. Write `docs/src/configuration/encryption.md` (key sources, precedence, key file, backup/restore, key loss, threat model, what is not covered, migration/downgrade, shell-env exception); add to `SUMMARY.md`.
2. Update `secrets.md` (remove the plaintext/no-encryption paragraph), and add short "credentials are encrypted at rest; back up the key" notes to `providers.md`, `channels.md`, `mcp.md`, `storage-shell.md`. Update `CLAUDE.md`.
3. Run `cargo fmt`, `cargo clippy`, `cargo test`; run the quickstart §1–§6 against a scratch workspace with a dummyplug agent.
4. Verify raw `vizier.db` + `vizier.db-wal` contain none of the test values after shutdown, sibling fields are still readable, missing/wrong key fails closed, restore works with the key, tampered value errors, and a second start does not re-encrypt.
5. Commit as `[**breaking**]` with the #82 note; state that downgrade requires a pre-upgrade backup and that release notes must name this as the pre-1.0 migration point.

## Complexity Tracking

No constitution violations.

| Decision | Why needed | Simpler alternative rejected because |
|---|---|---|
| `IntegrationSecretPaths` trait (one impl) | Storage cannot import the integration registry without a storage→integrations dependency; paths must come from `IntegrationKind::listing()` | Hardcoding `token`/`api_key`/`env` in storage duplicates the kind declarations and misses future kinds; encrypting every settings blob violates the sibling requirement |
| `SqliteStorage::with_secrets` alongside `new` | Production needs the cipher; 26 existing constructors are tests/legacy import and should not all change | Changing every constructor adds noise without testing encryption; a global singleton hides state and breaks test isolation |
| Direct `ring`/`zeroize` dependencies | AEAD and key zeroization are security-critical; both are already compiled in the tree | A new pure-Rust AEAD crate adds a new tree for the same primitive; hand-rolled crypto is unacceptable |
| One-time `VACUUM` migration | `UPDATE` alone leaves plaintext in SQLite free pages/WAL, which would falsify SC-003 | Lazy migration leaves plaintext indefinitely; skipping vacuum fails the feature's central claim |
