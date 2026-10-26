# Research: Encrypted Storage of Secrets at Rest

**Feature**: `031-encrypt-agent-secrets` | **Researched**: 2026-10-26 | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)

The codebase already has every secret-bearing surface this feature needs to cover, but it stores each one as an ordinary string in SQLite: `agent_secret.value`, `ProviderCredentials::{api_key, access_token}`, `CustomProvider::{api_key, headers}`, `integration.settings`, `agent_config.tools.shell.env`, and the legacy fields described in the spec. This document records the choices that turn those strings into field-level protected values without changing any user-facing behaviour.

## R1 — Field-level protection, not whole-record or whole-database encryption

**Decision**: Encrypt only the secret-bearing fields. Leave every non-secret sibling readable: provider endpoints/base URLs/model lists, custom-provider names/base URLs/header names/models, integration transport/command/arguments/URL/safe-search settings, shell paths/images, and agent names/prompts/providers/models.

**Rationale**: The spec's FR-002 makes this a requirement, and it is also the least surprising result. A provider's `base_url` is not a credential; an MCP `command` is not a credential; hiding them makes debugging, migration and key-loss recovery worse for no security gain. The existing integration layer already has a single declarative answer for "which fields are secret" (`FieldType::Secret` / `SecretMap`), so most of the classification is not new work.

**Alternatives considered**:
- *Encrypt the whole JSON column for `provider_config.data`, `custom_provider.data`, `integration.settings`, `agent_config.data`*: one wrapper instead of field paths, and future secret fields are covered automatically, but every non-secret sibling becomes unreadable. Rejected because the user explicitly asked to keep siblings readable.
- *Encrypt the whole SQLite database (SQLCipher or a filesystem-encrypted volume)*: protects everything, including transcripts and memory, but requires a different SQLite build or host-level setup, makes the key mandatory for every read, and does not satisfy the "field-level, siblings readable" requirement. Also violates the zero-external-service/portable-binary stance less directly but adds a large dependency.
- *Per-agent keys*: a larger key hierarchy with no current requirement; one workspace key is enough for the stated threat model.

## R2 — AEAD with `ring`, a versioned envelope, and record-bound AAD

**Decision**: Use `ring::aead::CHACHA20_POLY1305` with a random 32-byte key, a fresh 96-bit random nonce per value, and associated data that binds the ciphertext to its record and field. Store the result as:

```text
enc:v1:<key-id>:<base64url(nonce || ciphertext+tag)>
```

- `key-id` is the first 8 bytes of SHA-256(key), hex encoded (16 characters).
- Base64 is URL-safe, no padding.
- The associated data is a `\x1f`-joined string prefixed with `vizier:secrets:v1`, for example `vizier:secrets:v1\x1fagent_secret\x1fdp-a\x1fLINEAR_TOKEN`.
- AAD is derived, not stored; the reader supplies the same record identity and field path.

**Rationale**: `ring` is already in the dependency graph (rustls/ureq/bollard) and is the project's TLS crypto provider, so adding it as a direct dependency is weight-neutral. ChaCha20-Poly1305 avoids AES-NI assumptions and uses a simple 96-bit nonce API. The envelope is self-describing (version + key id), so a future format change can recognise old values. AAD prevents a ciphertext from being moved to a different agent, provider, integration or field and still decrypting; that is a stronger reading of the spec's tamper-evidence requirement than AEAD alone.

**Nonce collision**: a random 96-bit nonce gives a negligible collision probability for the number of values one installation stores. No per-key counter state is needed, which keeps key handling stateless and backup-friendly.

**Alternatives considered**:
- *AES-256-GCM*: equally suitable and also in `ring`; ChaCha was chosen for constant-time software behaviour and no hardware-feature question across the cross-compiled targets in `Cross.toml`.
- *XChaCha20-Poly1305*: larger nonce and no randomness concern, but requires a new crate (`chacha20poly1305`) instead of the already-present `ring`.
- *Unkeyed hash or base64 obfuscation*: not encryption; rejected outright.
- *No AAD*: simpler, but a ciphertext copied between rows would decrypt correctly. Rejected as insufficient for "altered or replaced outside Vizier".

## R3 — Key sources: env, explicit file, generated file; no keyring or passphrase

**Decision**: Resolve the key in this order:

1. `VIZIER_ENCRYPTION_KEY` — base64 of exactly 32 bytes.
2. `VIZIER_ENCRYPTION_KEY_FILE` — path to a key file; a set-but-missing/unreadable file is an error, never a reason to generate.
3. `{workspace}/.runtime/encryption.key` — the generated key file, created on first use with owner-only permissions where the platform supports it.
4. No key anywhere: generate one **only if no protected value exists**; otherwise fail with "encrypted data exists but no key was found".

The key file format is two lines:

```text
vizier-key-v1
<base64 32-byte key>
```

`VIZIER_ENCRYPTION_KEY` takes precedence over the file path, which takes precedence over the generated file. A malformed or wrong-length value is a startup error, never a fallback to generation.

**Rationale**: The project must keep config-less, headless, single-binary startup (constitution III/IV). A passphrase prompt breaks service managers and containers; an OS keyring does not exist on many servers and would add a platform-specific dependency. An auto-generated file gives zero-setup protection against a copied database, while an environment or mounted file gives operators separation when the key must not travel with the data.

**Alternatives considered**:
- *OS keyring (`keyring` crate)*: better isolation on a desktop, but unusable in a container/server, platform-specific, and an external state store. Rejected.
- *Passphrase at every startup*: strong, but breaks unattended startup and the single-binary/config-less promise. Rejected.
- *Mandatory operator-supplied key*: safest, but a fresh install would not work without setup. Rejected as the default; still available via env/file.
- *Key derived from a machine id*: not portable across restores and not a secret on the same host. Rejected.

## R4 — Key identity and fail-closed validation

**Decision**: Store a permanent `state` key `secrets.key_id` holding the key id. On startup:

1. Resolve and validate the key (length, base64, file readability).
2. If `secrets.key_id` exists and differs from the computed id, fail with "encryption key does not match this data".
3. If the state key is absent, scan the protected columns for the first `enc:v1:` envelope. A different key id is the same mismatch error; no envelope means there is nothing to validate.
4. Never generate a replacement key when any protected value or the key-id state exists.
5. Write `secrets.key_id` after a successful load/migration.

The id is a one-way hash prefix, not key material. It is safe in logs and diagnostics only as an identifier; the implementation should still not log it unless useful.

**Rationale**: A wrong key must be detected before any read or write can produce garbage or overwrite data. The state key makes the common case a single row lookup; the scan is the fallback for a restored database whose state row is missing. Distinct messages satisfy the spec's "not found / unreadable / does not match" requirement.

**Alternatives considered**:
- *Try the first decryption*: works but can fail late (after credentials have already been used) and does not distinguish "no key" from "wrong key" cleanly.
- *Key file header only*: does not help when the key comes from the environment or when the database is restored separately.
- *No key identity*: a wrong key produces integrity errors deep in a turn instead of a clear startup failure. Rejected.

## R5 — One cipher at the storage boundary; `new` stays plaintext for tests and legacy import

**Decision**: Add a concrete `SecretCipher` (not a trait) in `src/encryption/`. `SqliteStorage` holds a `SecretCipher` and an `Arc<dyn IntegrationSecretPaths>`. Production constructs it with `SqliteStorage::with_secrets(...)`; `SqliteStorage::new(...)` keeps its current signature and uses a disabled cipher plus a no-op path provider, so the 26 existing test/legacy constructors do not all change. Every storage module that touches a protected field calls the cipher directly.

**Rationale**: The storage layer is the single place every reader and writer already goes through, including the HTTP API, agent tools, channel readers and migration. Putting encryption there means no call site can forget it, and no secret has to pass through an HTTP/model boundary twice. A disabled cipher in `new` preserves the existing test suite and the filesystem-import path; production wiring uses `with_secrets` in one place. The alternative — encrypting in `integrations/service.rs` and each HTTP handler — would duplicate the field knowledge and leave readers such as channel connections and MCP clients unencrypted.

**Alternatives considered**:
- *A `VizierStorage` wrapper that encrypts before delegating*: would need to know every entity shape, and the disabled/default path would be awkward.
- *A global cipher singleton*: hidden state, hard to test, and unsafe for multiple workspaces in one process (tests, future multi-tenant). Rejected.
- *Change every `SqliteStorage::new` call*: 26 call sites, mostly tests, for no behavioural gain; a separate `with_secrets` constructor is smaller.

## R6 — Integration secret paths come from the kind declaration

**Decision**: Add `IntegrationSecretPaths` (one method: `secret_paths(kind) -> Option<Vec<String>>`) and implement it for `IntegrationKinds`. The implementation reads each kind's `listing().fields`:

- `FieldType::Secret` → the field key (`token`, `api_key`).
- `FieldType::SecretMap` → `<key>.*` (the MCP `env` map).
- Everything else → not secret.

An unknown kind returns `None`; storage then protects the entire `settings` JSON as one envelope, because it cannot know which siblings are safe to leave readable. The fallback is only for a kind the binary no longer knows; known kinds always get field-level protection.

**Rationale**: `IntegrationKind::listing` is already the single source of truth for redaction and merge-on-write. Using it for encryption means a new kind declares its secrets once, and no storage `match` over kind ids is introduced (constitution II). The unknown-kind whole-blob fallback is a safety net, not the normal path.

**Alternatives considered**:
- *Hardcode `token`/`api_key`/`env` in storage*: works today but silently misses a future kind's secret and duplicates the list in two places.
- *Encrypt every integration's whole settings blob*: rejects the sibling requirement for the common case.
- *Skip unknown kinds*: leaves legacy/unknown credentials plaintext; rejected.

## R7 — Startup migration after the shape migrations, with vacuum and a marker

**Decision**: Add `src/encryption/migrate.rs` with `migrate_secrets_to_encrypted`. Startup order becomes:

1. Open the SQLite connection and ensure the schema.
2. Load/validate the cipher (generating a key only when there is nothing protected).
3. Run the existing memory and filesystem-backend migrations. The filesystem migration writes through a cipher-enabled storage, so imported secrets are protected immediately.
4. Run `provider_migration::migrate_provider_ids_v1` (it rewrites pre-015 provider rows as plaintext `ProviderEntry`).
5. Run the encryption migration over every protected table.
6. Construct the main `VizierStorage` with the cipher and run the remaining migrations (`migrate_users`, `migrate_agent_cores`, …).

The migration is one transaction (or one per table, but all before the marker), sets `PRAGMA secure_delete=ON` before updates, then commits, runs `PRAGMA wal_checkpoint(TRUNCATE)`, `VACUUM`, and a second checkpoint. Only then does it write the marker `migration.secrets_encrypted_v1` in the existing `state` table. It is idempotent: an already-protected value is validated and skipped; a plaintext value is encrypted. It logs counts per table, never values.

**Rationale**: The order avoids a plaintext write after the marker and ensures every consumer (agents, channels, scheduler) starts only after the conversion. `secure_delete` + `VACUUM` + checkpoint is what removes the old plaintext from SQLite free pages and the WAL; an `UPDATE` alone leaves recoverable copies. A single marker lets a restart resume safely and is the one thing the future #82 cleanup removes.

**Virtual-table check**: the database contains FTS5 (`memory_fts`) and, once an indexer runs, the `sqlite-vec` `document_index` virtual table. Verify `VACUUM` works with both in the implementation. If `sqlite-vec` rejects it, use `VACUUM INTO <temp>` on the same connection, close and atomically replace `vizier.db` (and remove/recreate WAL/SHM), then reopen and validate; the post-condition is identical (no plaintext in the resulting database or WAL).

**Alternatives considered**:
- *Re-encrypt lazily on read*: leaves plaintext on disk indefinitely and makes reads slow/error-prone. Rejected.
- *Background migration after startup*: agents and channel readers could read a value mid-conversion, and the old plaintext stays longer. Rejected.
- *No vacuum*: the feature's central claim ("a copy of the database contains no secrets") would be false for pre-encryption rows. Rejected.
- *SQLCipher*: whole-DB, external build/feature, not field-level. Rejected.

## R8 — Legacy values are migrated or removed, never left readable

**Decision**:
- Pre-015 `provider_config` rows are handled by the existing `migrate_provider_ids_v1` before the encryption migration; after that, any unparseable row aborts the migration with a clear error rather than being skipped.
- `agent_config` is deserialized into `AgentConfig` and re-serialized. That drops the pre-016 unknown keys (`discord_token`, `telegram_token`, `tools.discord`, `tools.telegram`, `tools.brave_search`, `tools.mcp_servers`), so their values are removed rather than encrypted. The `core` field is preserved for `migrate_agent_cores`.
- `global_config` shell environment values are encrypted. The table currently has no reader (it is populated only by the old filesystem migration); removing it is a possible follow-up noted for #82, but this feature does not delete operator data.
- Unknown integration kinds are whole-blob-protected as in R6.

**Rationale**: Dead credentials should not be carried forward just to be encrypted; the spec allows removal. Re-serializing `AgentConfig` is the existing mechanism that already drops these keys on the next save; doing it once at migration makes the guarantee immediate. Keeping `global_config` data and encrypting its env values is the non-destructive choice.

**Alternatives considered**:
- *Delete `global_config` rows/table now*: less data, but destructive and would require changing the filesystem migration; the table is harmless once encrypted. Rejected for this feature.
- *Leave legacy agent keys in place*: would leave plaintext token values in a row nobody re-saves. Rejected.
- *Encrypt legacy provider rows without migrating their shape*: doubles the compatibility code; the existing migration already knows the legacy shapes. Rejected.

## R9 — Errors fail closed, and list reads do not silently skip corrupt rows

**Decision**:
- Startup key errors abort the process with distinct messages: key not found, key unreadable/malformed, key does not match, key file not writable.
- A read of a protected field that is plaintext after migration, malformed, or fails AEAD returns an error naming the record/field (never the value/ciphertext). No plaintext fallback is allowed in normal reads.
- List operations (`list_agents`, `list_providers`, `list_custom_providers`, `list_integrations`, `list_agent_secrets`) propagate a decryption error for the affected row instead of `filter_map`-dropping it. Existing parse-skip behaviour for genuinely unparseable non-encryption rows can stay where it is documented.
- Migration aborts on an unreadable protected table row, because it cannot prove the row is free of plaintext.

**Rationale**: The feature's promise is that no recoverable plaintext remains and a wrong key never silently becomes a new key. Silently skipping a corrupt row would hide exactly the failure the feature is meant to expose. The cost is that one corrupt agent/provider/integration can fail a list; that is the correct fail-closed behaviour for a security migration.

**Alternatives considered**:
- *Log and skip*: keeps the UI up but can hide a plaintext/failed row; rejected.
- *Return an empty value*: could silently use a wrong credential; rejected.
- *Abort the whole process on any read error*: too broad for a single corrupt integration; return the error to the caller instead.

## R10 — Key file permissions are the only platform-specific piece

**Decision**: Use `OpenOptionsExt::mode(0o600)` on Unix and set owner-only permissions after creation. On a platform that cannot express Unix permissions, create the file with the platform default and log one warning; do not fail. The crypto and envelope code is platform-neutral. The key file lives under the existing `.runtime` directory next to `vizier.db`.

**Rationale**: Constitution IV requires platform-specific behaviour to be isolated. A file permission is the only OS-specific part; the cryptographic operations and serialization are portable. The key file must not be world-readable where the OS supports it.

**Alternatives considered**:
- *Fail on Windows because permissions cannot be enforced*: breaks the supported Windows target. Rejected.
- *Add an ACL crate*: a new dependency for a warning-level guarantee; rejected.
- *Store the key in the database*: defeats the purpose. Rejected.

## R11 — Dependencies: `ring` and `zeroize` are already in the tree

**Decision**: Add direct `ring = "0.17"` and `zeroize = "1.9"` to `Cargo.toml`. Reuse `base64` 0.23, `sha2` 0.11, `rusqlite` 0.40 and `tracing`. No new crate, no new service, no new build step.

**Rationale**: `cargo tree` shows `ring` 0.17.14 and `zeroize` 1.9.1 already compiled through rustls/aws-lc-rs and other dependencies, so adding them as direct dependencies does not add a crate to the build. `ring` gives the AEAD and system randomness; `zeroize` clears raw key bytes and base64 text. This satisfies the constitution's "check whether an existing dependency covers the need" rule.

**Alternatives considered**:
- *`chacha20poly1305` / `aes-gcm` crates*: small, but add a new tree when `ring` already provides the primitive.
- *`aws-lc-rs` directly*: already present, but its AEAD API is less ergonomic for this small use and ring is the project's existing default provider for TLS.
- *`secrecy`*: useful for typed secret wrappers but adds another crate; `zeroize` covers the raw-key lifetime.

## R12 — No HTTP, OpenAPI or WebUI change

**Decision**: The feature is transparent to every existing endpoint and screen. No routes, DTOs, OpenAPI schemas or WebUI components are added. Values are decrypted in storage before existing masking/redaction runs. Documentation gains an encryption page and links; the Secrets page keeps its current masked display.

**Rationale**: The user asked for at-rest protection, not a new key-management UI. The existing provider, integration and secret screens already never return values, so their behaviour is unchanged. Adding a "reveal" or key-status surface would enlarge the attack surface and the scope.

**Alternatives considered**:
- *Show a lock badge or key status in the WebUI*: nice-to-have, no security value, extra API surface. Deferred.
- *A key-rotation/reveal UI*: explicitly out of scope.

**One documented exception**: the agent configuration API already returns `tools.shell.env` to people allowed to view/edit the agent. Those values are encrypted at rest but remain visible there, as today. Hiding them behind write-only masks is a separate change (the spec records this assumption).

## R13 — Performance: per-field AEAD, no cache

**Decision**: Encrypt/decrypt on each storage read/write, with no decrypted-value cache. Expected cost is microseconds per value; lists decrypt only the rows they already load; agent/provider/integration reads happen at build/refresh time, not per token. The one-time migration and `VACUUM` are the only potentially slow operations; log them.

**Rationale**: A cache of plaintext credentials in memory would widen the exposure and add invalidation bugs for a negligible gain. The storage layer already holds the SQLite mutex for each operation; AEAD over a small string is not the bottleneck.

**Alternatives considered**:
- *Cache decrypted values in memory*: fewer decrypts, but plaintext lives longer and must be invalidated on every write; rejected.
- *Encrypt the whole database*: slower for every unrelated query and not field-level; rejected.

## R14 — Verification: unit, storage, migration and dummyplug end-to-end

**Decision**: The test plan is:
- `src/encryption/`: key parsing/precedence/permissions, envelope round-trip, randomized ciphertext, wrong key id, tamper/AAD mismatch, malformed envelope, disabled cipher.
- Storage tests with `with_secrets`: raw database value is an envelope; read returns the exact plaintext; list masks; sibling fields remain plaintext; wrong key errors; provider/integration/agent/global paths; unknown integration kind fallback.
- Migration tests on a file-backed temp database: plaintext to encrypted, idempotent, marker, interrupted-run resume, legacy agent key removal, `VACUUM` removes the plaintext from the database bytes, wrong/missing key fails before writing.
- Dummyplug end-to-end in [quickstart.md](./quickstart.md): set/get a secret, restart, inspect the database and WAL, restore with/without the key, wrong key, sibling readability, tamper error.

**Rationale**: The constitution requires the dummyplug e2e gate for agent-observable changes, and the SQLite migration/free-page guarantee cannot be proven with an in-memory database alone. A file-backed migration test is the only way to assert the no-plaintext-bytes outcome.

**Alternatives considered**:
- *Only e2e*: too slow and cannot inspect every table/field.
- *Only in-memory storage tests*: cannot test `VACUUM`/WAL removal.
- *Live provider tests*: not needed; the feature does not depend on model output. Discord/Telegram live connections are marked optional in the quickstart.

## R15 — Documentation, backup, downgrade and release notes

**Decision**: Add `docs/src/configuration/encryption.md` and link it from `secrets.md`, `providers.md`, `channels.md`, `mcp.md` and `storage-shell.md`; change `secrets.md`'s "no application-level encryption" paragraph; update `CLAUDE.md`; include a backup/restore/key-loss section. The commit is `[**breaking**]`; downgrade is unsupported and must fail rather than overwrite. Release notes name this as the pre-1.0 migration point and state that the key must be backed up separately.

**Rationale**: Encryption changes the operator's backup obligations. Without an explicit page, an operator will either copy the database without the key (and fail to restore) or copy the whole directory and believe the backup is protected when the key travelled with it. The threat model must be stated: this protects data at rest from a copy without the key, not a compromised host or process.

## R16 — #82 inventory: the migration is temporary; the cipher is permanent

**Decision**: Add a `#82` checklist entry for:
- `migrate_secrets_to_encrypted`, its marker `migration.secrets_encrypted_v1`, and its plaintext-detection/`decrypt_legacy` branch;
- the legacy `AgentConfig` key scrub and the unknown-kind whole-blob fallback if it exists only for pre-031 data;
- the migration tests.

Do **not** list the cipher, key resolution, key id, AAD, envelope version reader or field-level storage helpers; they are permanent. The spec's FR-028/FR-029 and the plan's task list carry this requirement.

**Rationale**: After the last pre-1.0 release, all data is already protected and the one-time conversion plus plaintext fallback can be deleted, as with every other migration in that inventory. The cipher must stay.

## R17 — No new table; state keys only

**Decision**: Do not add a table or columns. Protected values live in the existing TEXT columns. The permanent key id and the one-time migration marker are `state` rows (`secrets.key_id`, `migration.secrets_encrypted_v1`). The migration itself uses raw SQL over the existing tables.

**Rationale**: The schema already has the right home for every value; encryption changes the representation, not the shape. A key table would be a second source of truth and a migration of its own. The existing `state` table is already used for migration markers and durable flags.

**Alternatives considered**:
- *A `secret_key` table or a metadata column on each table*: more schema, more migrations, no benefit.
- *Store the key id only in envelopes*: possible, but a state row makes wrong-key detection a cheap lookup and gives the migration marker a natural home.

## Open questions

None. The spec's assumptions resolve the scope; the plan's Technical Context contains no `NEEDS CLARIFICATION`. The one deliberate exception is shell environment visibility (R12), recorded in the spec because the agent configuration API already exposes it to authorized agent managers.
