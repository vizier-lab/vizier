# Contract: Encryption at Rest

**Feature**: `031-encrypt-agent-secrets` | **Plan**: [../plan.md](../plan.md) | **Data model**: [../data-model.md](../data-model.md)

This is an internal storage/operator contract, not a public HTTP API contract. No HTTP endpoint changes. It fixes the key sources, file/format, envelope, AAD and failure behaviour so storage code, migration code, tests and documentation cannot drift.

## 1. Scope

Protected: every value enumerated in [data-model.md §2](../data-model.md#2-protected-field-matrix).

Not protected: conversation history, memory/CORE, session files, task text, dream journals, notifications, usage, password/API-key hashes, `.vizier.yaml`, environment variables, OS swap/full-disk encryption.

## 2. Key sources and precedence

| Order | Source | Set to | Missing/unreadable | Malformed |
|---|---|---|---|---|
| 1 | `VIZIER_ENCRYPTION_KEY` | base64 of 32 bytes | n/a | startup error; no fallback |
| 2 | `VIZIER_ENCRYPTION_KEY_FILE` | path to a key file | startup error; no generation | startup error |
| 3 | `{workspace}/.runtime/encryption.key` | generated key file | generate **only if** no protected value exists; otherwise startup error | startup error |
| — | none | — | generate only if no protected value exists | — |

A set but empty `VIZIER_ENCRYPTION_KEY` is treated as malformed, not as unset. An explicit `VIZIER_ENCRYPTION_KEY_FILE` that does not exist is an error, never a reason to create it.

### Key file format

```text
vizier-key-v1
<base64 of exactly 32 bytes>
```

- UTF-8, trailing newline allowed.
- Unsupported version line → malformed.
- Created with owner-only permissions where the platform supports it. On a platform that cannot express Unix permissions, creation succeeds and one warning is logged.
- Never overwritten by generation. Never stored in the database.
- Default path: `{workspace}/.runtime/encryption.key` (next to `vizier.db`).

## 3. Envelope

```text
enc:v1:<key-id>:<payload>
```

| Part | Rule |
|---|---|
| `enc:v1:` | Literal prefix. `v1` is the only accepted version in this release. |
| `<key-id>` | 16 lowercase hex characters = first 8 bytes of SHA-256(32-byte key), hex. |
| `<payload>` | base64url (no padding) of `nonce (12 bytes) || ciphertext || tag (16 bytes)`. |

- Algorithm: ChaCha20-Poly1305 (RFC 8439) via `ring::aead`.
- Nonce: 12 random bytes from the system CSPRNG, fresh for every encryption. Never reused with the same key.
- AAD: reconstructed, not stored (§4).
- The same plaintext encrypted twice must produce different envelopes.
- A read validates the prefix, version, key id, base64, nonce length and AEAD tag before returning anything.
- The envelope is an ordinary string in the existing TEXT column/JSON field. No column type changes.

## 4. AAD (associated data)

```text
vizier:secrets:v1 \x1f <table> \x1f <record identity> \x1f <field path>
```

| Table | Record identity | Field path example |
|---|---|---|
| `agent_secret` | `agent_id` | `LINEAR_TOKEN` |
| `provider_config` | provider variant/id | `credentials.api_key` |
| `custom_provider` | custom provider id | `headers.Authorization` |
| `integration` | integration id | `token`, `api_key`, `env.GITHUB_TOKEN` |
| `integration` (unknown kind) | integration id | `settings` |
| `agent_config` | agent id | `tools.shell.env.OPENAI_API_KEY` |
| `global_config` | config key | `value.data.env.OPENAI_API_KEY` |

Rules:

- One implementation in `src/encryption/fields.rs`; migration and storage call it, never rebuild the string by hand.
- AAD contains no plaintext and no key. It may appear only as record/field identifiers in errors.
- Moving an envelope to a different record/field must fail AEAD authentication.
- A rename that changes AAD is remove + add, not an in-place update.

## 5. Startup and failure behaviour

| Situation | Outcome |
|---|---|
| No key, no protected value | Generate key file, write `state: secrets.key_id`, continue. |
| No key, protected value exists | Exit non-zero: `encrypted data exists but no encryption key was found; set VIZIER_ENCRYPTION_KEY or restore the key file`. No write. |
| Key env/file malformed | Exit non-zero: `encryption key is malformed: expected 32 bytes encoded as base64`. No write. |
| Explicit key file missing/unreadable | Exit non-zero naming the path and the problem. No generation. |
| Key id differs from `state: secrets.key_id` | Exit non-zero: `encryption key does not match this data`. No write. |
| No state key, first envelope has a different key id | Same mismatch error. No write. |
| No state key, no envelope | No mismatch to detect; proceed (then write the state key). |
| AEAD open fails on a normal read | Return an error naming the table/record/field; no plaintext, no fallback. |
| Envelope version unknown | Return an error naming the table/record/field; no fallback. |
| Migration cannot vacuum/checkpoint | Exit non-zero; do not write the migration marker. |
| Process restarts mid-migration | The transaction rolls back or the marker is absent; the next start retries. |

Startup never prompts. It never fetches a key over the network and never uses an OS keyring.

## 6. Field-level storage contract

| Surface | Write | Read |
|---|---|---|
| Agent secret value | Encrypt the supplied value with `agent_secret` AAD | Decrypt; then mask for lists; `get` returns the exact plaintext |
| Provider credentials | Encrypt `api_key`/`access_token`; leave `endpoint`, `base_url`, `account_id` | Decrypt the two fields; list/get return the complete `ProviderEntry` |
| Custom provider | Encrypt `api_key` and every `headers` value; names stay plaintext | Decrypt; list/get return the complete `CustomProvider` |
| Integration settings | Encrypt `Secret`/`SecretMap` paths from `listing()`; leave everything else | Decrypt those paths; unknown kind decrypts the whole settings value |
| Agent shell env | Encrypt every `env` value in local/Docker shell; leave path/image/container | Decrypt every `env` value; the agent configuration API returns them to authorized managers (documented exception) |
| Global shell env | Encrypt every `env` value; leave the rest | Decrypt every `env` value |
| Legacy agent keys | Drop them during migration (`AgentConfig` re-serialization) | They no longer exist |
| Legacy pre-015 provider rows | Migrated by `migrate_provider_ids_v1` before encryption | Normal `ProviderEntry` |
| Deleted values | Delete the row/field; no key needed | Absent |

No storage read accepts plaintext after the migration marker is set. `open` (allow plaintext) is migration-only.

## 7. Non-secret siblings and the exception list

Explicitly readable without a key:

- secret names, masks, timestamps, actors, IDs, integration kind/family/name/enabled;
- provider `endpoint`, `base_url`, `account_id`, model catalogue data;
- custom-provider name, protocol, `base_url`, header names, models;
- integration `transport`, `command`, `args`, `uri`, `safesearch`, and every other non-`Secret`/`SecretMap` field;
- agent name, prompt, provider, model, tool flags, shell path/image/container, and every non-`env` field;
- global config key and the non-`env` shell fields.

The only value that is protected at rest but returned by an existing management response is `agent_config.tools.shell.env` (and the Docker equivalent), to people allowed to view/edit that agent. This is existing behaviour, recorded in the spec's assumptions. Making it write-only is a separate change.

## 8. No API/CLI change

- No new HTTP route, request/response field, OpenAPI schema or WebUI screen.
- No new CLI flag or subcommand. Key sources are environment variables and the default file.
- No OpenAPI regeneration is required.
- Docs: new `docs/src/configuration/encryption.md`; update `secrets.md` (remove the "no application-level encryption" claim), `providers.md`, `channels.md`, `mcp.md`, `storage-shell.md`, `docs/src/SUMMARY.md`, and `CLAUDE.md`.

## 9. Test obligations

1. Envelope round-trips bytes exactly, including Unicode and multi-line values.
2. The same plaintext twice produces different envelopes.
3. A wrong key id, flipped byte, truncated payload, unknown version or wrong AAD fails with a clear error and no value.
4. Key precedence: env > key file path > default generated file; malformed/missing explicit sources fail without generating.
5. Raw SQLite bytes after a write do not contain the plaintext; sibling fields do.
6. After migration and `VACUUM`, the database file and WAL contain no pre-migration plaintext.
7. Missing/wrong key fails startup before any write; restoring the key succeeds.
8. List/read of a tampered row fails rather than silently skipping it.
9. A fresh install works with zero key setup and no prompt.
10. The dummyplug end-to-end script in [quickstart.md](../quickstart.md) passes.
