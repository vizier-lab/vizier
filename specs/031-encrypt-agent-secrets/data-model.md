# Data Model: Encrypted Storage of Secrets at Rest

**Feature**: `031-encrypt-agent-secrets` | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

The feature adds no table and no column. It changes the representation of selected values in existing TEXT columns and adds two rows to the existing `state` table. Everything else — IDs, names, timestamps, actors, masks, non-secret settings — stays where it is and stays readable.

## 1. In-memory entities

### `SecretCipher`

The one cipher for a workspace, held by `SqliteStorage` and by the startup migration.

| Field | Type | Rules |
|---|---|---|
| `key` | `ring::aead::LessSafeKey` (behind `Arc`) | 32-byte ChaCha20-Poly1305 key. Never serialized, never logged, never returned by an API. |
| `key_id` | `String` | 16 hex characters: first 8 bytes of `SHA-256(key)`. Safe to store and show as an identifier. |
| `source` | `KeySource` | Where the key came from, for diagnostics (`Env`, `File(PathBuf)`, `Generated(PathBuf)`, `Disabled`). Never contains key material. |
| `enabled` | `bool` (or `Option<Arc<Inner>>`) | `false` only for the test/legacy-import constructor; production always enabled. |

Methods: `load_or_generate(workspace, conn)`, `from_key_for_test([u8; 32])`, `disabled()`, `encrypt(plaintext, aad)`, `decrypt(stored, aad) -> Result<String>`, `open(stored, aad) -> Result<Option<String>>` (used by migration; `None` means plaintext), `is_protected(stored)`, `key_id()`.

### `KeySource`

| Variant | When | Behaviour |
|---|---|---|
| `Env` | `VIZIER_ENCRYPTION_KEY` is set and non-empty | Decode base64; malformed is a startup error. Never fall back. |
| `File(PathBuf)` | `VIZIER_ENCRYPTION_KEY_FILE` is set | Read the file; missing/unreadable/malformed is a startup error. Never generate over it. |
| `Generated(PathBuf)` | default `{workspace}/.runtime/encryption.key` | Load if present; generate only when absent **and** no protected value exists. |
| `Disabled` | tests and the pre-encryption filesystem import | No encryption; reads/writes are plaintext. Production never uses it. |

### `ProtectedEnvelope`

| Part | Example | Rules |
|---|---|---|
| Prefix | `enc:v1:` | Reserved. `v1` is the format version. |
| Key id | `3f1c0a9b7d2e4f60` | 16 lowercase hex chars, first 8 bytes of SHA-256(key). A different id means "wrong key", not "corrupt value". |
| Payload | base64url(nonce ‖ ciphertext ‖ tag) | URL-safe, no padding. Nonce is 12 random bytes; tag is 16 bytes. |

The AAD is not stored. It is reconstructed from the record identity and field path (see §4). A ciphertext moved to another row/field fails AEAD authentication.

### `IntegrationSecretPaths`

| Method | Result |
|---|---|
| `secret_paths(kind) -> Option<Vec<String>>` | `Some(paths)` for a known kind; `None` for an unknown kind (whole-settings fallback). |

Paths for the current kinds:

| Kind | Field type | Paths |
|---|---|---|
| `discord` | `Secret` | `token` |
| `telegram` | `Secret` | `token` |
| `brave_search` | `Secret` | `api_key` |
| `mcp` | `SecretMap` | `env.*` |

`*` means "every value of that object"; names stay readable.

### `EncryptionMigration`

| Field | Value |
|---|---|
| Marker key | `migration.secrets_encrypted_v1` in `state` |
| Marker value | JSON `true` (same convention as the existing `migrate_*` helpers) |
| Scope | `agent_secret`, `provider_config`, `custom_provider`, `integration`, `agent_config`, `global_config` |
| Re-runnable | Yes: protected values are validated and skipped; plaintext values are encrypted. The marker is written only after vacuum/checkpoint succeed. |

## 2. Protected-field matrix

| Record (existing storage) | Protected field(s) | Left readable (examples) | AAD context |
|---|---|---|---|
| Agent secret (`agent_secret.value`) | the whole value | name, created/updated times, actor; mask computed after decrypt | `agent_secret` + `agent_id` + `name` |
| Provider credentials (`provider_config.data` → `ProviderEntry.credentials`) | `api_key`, `access_token` | `endpoint`, `base_url`, `account_id`, provider id, model catalogue data | `provider_config` + variant + `credentials.api_key` / `credentials.access_token` |
| Custom provider (`custom_provider.data`) | `api_key`, every `headers` value | id, name, protocol, `base_url`, header names, models, `models_fetched_at` | `custom_provider` + id + `api_key` / `headers.<name>` |
| Integration (`integration.settings`) | the kind's `Secret`/`SecretMap` fields (table above) | id, kind, family, name, enabled, transport, command, arguments, URL, safe-search flag | `integration` + integration id + field path (`token`, `api_key`, `env.<NAME>`) |
| Integration, unknown kind | the whole `settings` JSON, as one envelope in a JSON string | nothing inside the settings (only the plaintext columns id/kind/family/name/enabled) | `integration` + id + `settings` |
| Agent config (`agent_config.data` → `AgentConfig.tools.shell`) | every value in the local or Docker shell `env` map | agent id, name, prompt, provider, model, all other tool settings, shell path/image/container | `agent_config` + agent id + `tools.shell.env.<NAME>` |
| Global shell config (`global_config.data` → `GlobalConfigEntry.value` shell) | every value in the shell `env` map | key, shell path/image/container, all other fields | `global_config` + key + `value.data.env.<NAME>` |
| Legacy agent keys in `agent_config.data` | **removed**, not encrypted: `discord_token`, `telegram_token`, `tools.discord`, `tools.telegram`, `tools.brave_search`, `tools.mcp_servers` | everything the current `AgentConfig` keeps | n/a |
| Legacy pre-015 providers (`provider_config.data`) | migrated by `migrate_provider_ids_v1` before this feature; then `api_key`/`access_token` as above | variant/id, non-secret fields | `provider_config` + variant + field |
| Deleted/absent values | nothing | — | — |

Rules:

- Only `Value::String` at a secret path is protected. A non-string value at a `Secret` or `SecretMap` path is an error during migration/write, not silently encrypted.
- A known kind with no secret paths (none today) is left alone.
- A known kind whose `settings` is not an object is treated as an unknown shape and whole-blob-protected; this is defensive, not a supported shape.
- A record with no secret field is not protected merely because it lives in a table that can hold one.
- Existing masks and redaction run after decryption; no endpoint returns a stored value except the documented shell-env configuration exception.

## 3. State keys

| Key | Written | Read | Meaning |
|---|---|---|---|
| `secrets.key_id` | After a successful cipher load and before the first protected read/write; refreshed by migration | Startup, before storage construction | The id of the key that protects this workspace. Missing is tolerated only when no protected value exists; different is a startup error. |
| `migration.secrets_encrypted_v1` | After the migration transaction, vacuum and checkpoint succeed | Startup | The one-time plaintext conversion completed. Removed with the migration after the last pre-1.0 release (#82). |

Both live in the existing `state(key TEXT PRIMARY KEY, value TEXT NOT NULL)` table; no schema change.

## 4. AAD (associated data)

AAD is a `\x1f`-joined, prefixed string:

```text
vizier:secrets:v1 \x1f <table> \x1f <record identity> \x1f <field path>
```

| Table | Record identity | Field path |
|---|---|---|
| `agent_secret` | `agent_id` | `name` (e.g. `LINEAR_TOKEN`) |
| `provider_config` | provider variant/id | `credentials.api_key` or `credentials.access_token` |
| `custom_provider` | custom provider id | `api_key` or `headers.<header-name>` |
| `integration` | integration id | `token`, `api_key`, or `env.<ENV_NAME>` |
| `integration` (unknown kind) | integration id | `settings` |
| `agent_config` | agent id | `tools.shell.env.<ENV_NAME>` |
| `global_config` | config key | `value.data.env.<ENV_NAME>` |

Rules:

- Record identity must be stable for the lifetime of the value. The ids above are stable; a delete and re-create is a new record with a new ciphertext.
- A rename that changes the AAD (for example a header rename) is implemented as remove + add, not as an update of the protected value.
- AAD never contains key material or the plaintext. It may appear in an error message only as a record/field identifier, never with the value.
- Migration and normal reads use the same AAD builders from `src/encryption/fields.rs`; there is one implementation, not two.

## 5. Migration state machine

| State | Detected by | Action |
|---|---|---|
| Fresh workspace | `secrets.key_id` absent, no `enc:v1:` value | Generate a key file, write `secrets.key_id`, run the (empty) migration, record the marker. |
| Existing plaintext workspace | no protected values, no key file | Generate a key, write `secrets.key_id`, convert every row, vacuum, record the marker. |
| Partially converted | no marker; a mix of plaintext and `enc:v1:` values | Validate every envelope's key id, encrypt the plaintext rows, vacuum, record the marker. |
| Converted | marker present | Verify `secrets.key_id` if needed; skip the migration; normal reads require envelopes. |
| Key missing, data protected | key resolution found nothing, scan/state found an envelope | Abort startup: "encrypted data exists but no encryption key was found". Do not generate. |
| Key unreadable/malformed | env/file parse error | Abort startup: "encryption key is unreadable/malformed". Do not generate. |
| Wrong key | `secrets.key_id` or the first envelope differs | Abort startup: "encryption key does not match this data". No writes. |
| Tampered value | AEAD open fails | Read error naming the record/field; no value returned. The migration aborts if it encounters one while scanning. |
| Migration interrupted | marker absent | Next start re-runs; already-protected rows are skipped; vacuum/checkpoint/marker eventually succeed. |
| Downgrade | an older binary sees an envelope it does not understand | The spec requires fail-closed; the older release has no decryption path, so it must not overwrite. Upgrade notes say downgrade needs a pre-upgrade backup. |

## 6. Validation rules

- **Key**: exactly 32 bytes. Env/file encoding is standard base64 (whitespace trimmed); a key file has the two-line `vizier-key-v1` header. A wrong length/header/character is an error.
- **Key file**: created under `{workspace}/.runtime/encryption.key`; owner-only where supported; parent directory created if missing. A generated key is never overwritten; a supplied key path is never silently replaced.
- **Envelope**: `v1` only; unknown versions are an error; the key id must equal the active key id before AEAD is attempted; base64 must decode; the nonce must be 12 bytes and the remainder at least 16 bytes (tag).
- **Secret fields**: only strings; empty strings are still protected (provider empty means "not configured", integration validation handles emptiness); `null`/absent is left absent.
- **Shell env**: map keys stay readable; every string value is protected. `null` env maps stay `None`.
- **Integration paths**: `Secret` = top-level key; `SecretMap` = every value of the named object. Nested secret paths are not supported until a kind declares one; the kind registry is the extension point.
- **Unknown integration kind**: whole-settings envelope; a future binary that learns the kind must decrypt the whole blob before it can use it, or the data must be re-saved. This is a pre-1.0 compatibility fallback.
- **No plaintext fallback after the marker**: normal reads call `decrypt` (require), not `open` (allow plaintext). Only the migration calls `open`.
- **No value/key in errors or logs**: record/field names and counts are allowed; ciphertext, plaintext and key bytes are not.

## 7. What is deliberately not modeled

- No key rotation, key escrow, recovery code, expiry or per-agent key.
- No encryption of conversation history, memory/CORE documents, session files, task text, dream journals, notifications, usage records, password hashes or API-key hashes.
- No whole-database encryption and no OS-level encryption.
- No new API or WebUI entity. The shell-env configuration exception is documented in the spec (R12).
- No cache of decrypted values.
