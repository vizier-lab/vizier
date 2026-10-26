# Contract: Plaintext-to-Protected Migration

**Feature**: `031-encrypt-agent-secrets` | **Plan**: [../plan.md](../plan.md) | **Data model**: [../data-model.md](../data-model.md)

This contract fixes the one-time conversion's order, scope, marker, idempotency and failure rules. It is the part of the feature that the pre-1.0 removal tracker (#82) will delete after the last pre-1.0 release.

## 1. Trigger and ordering

The migration runs inside `VizierDependencies::new`, after the existing storage-shape migrations and before any agent, channel reader, scheduler or background job starts.

```text
open SQLite connection + schema
  → load/validate the cipher (may generate a key only when nothing is protected)
  → migrate_memory_to_bundles               (unchanged; no secrets)
  → migrate_filesystem_backend_to_sqlite    (writes through a cipher-enabled storage)
  → provider_migration::migrate_provider_ids_v1
  → migrate_secrets_to_encrypted            (this contract)
  → construct the main VizierStorage with the cipher
  → migrate_users / migrate_agent_cores / migrate_task_requester / sweeps ...
  → start agents, channels, scheduler, command server
```

- The cipher must be loaded before the filesystem import so imported provider/agent/global values are protected as they are written, and so a missing/wrong key fails before new plaintext is added.
- `migrate_provider_ids_v1` runs before this migration because it rewrites pre-015 provider rows into the current `ProviderEntry` shape. The encryption migration can then work on one provider shape.
- No consumer runs during the migration. The main storage is constructed only after it completes.

## 2. Scope

| Table | Column / JSON path | Action |
|---|---|---|
| `agent_secret` | `value` | Encrypt with `agent_secret` AAD. |
| `provider_config` | `credentials.api_key`, `credentials.access_token` | Encrypt field-level; leave `endpoint`, `base_url`, `account_id`. |
| `custom_provider` | `api_key`, every `headers` value | Encrypt field-level; header names stay. |
| `integration` | kind-declared `Secret`/`SecretMap` paths, or the whole `settings` for an unknown kind | Encrypt field-level normally; whole-settings envelope for unknown kinds. |
| `agent_config` | every `tools.shell.env` value (local/Docker) | Encrypt field-level; re-serializing `AgentConfig` also drops the pre-016 moved keys. |
| `global_config` | every shell `env` value | Encrypt field-level; the table has no current reader, so this is non-destructive protection. |

Not touched: history, memory/CORE, session files, tasks, dream journals, notifications, usage, users/roles/API-key hashes, `state` values other than the two keys in §4, catalogue data.

## 3. Idempotency and interruption

- A value already starting with a valid `enc:v1:<active-key-id>:` envelope is validated (or at least parsed) and skipped.
- A plaintext value at a protected path is encrypted and written back.
- The update of all tables and the deletion/rewrite of legacy keys happen before the marker. If the process dies:
  - before commit → SQLite rolls back; the next start sees the original plaintext and retries;
  - after commit but before vacuum → the next start sees protected values (skips them) and redoes vacuum;
  - after vacuum but before the marker → the next start redoes vacuum and writes the marker;
  - after the marker → the migration is skipped.
- Running the migration twice in a row changes no value and logs zero conversions the second time.

## 4. Marker and key identity

| State key | Written by | Meaning |
|---|---|---|
| `secrets.key_id` | Cipher load / this migration | The key id protecting this workspace. A different id is a startup error, not a migration. |
| `migration.secrets_encrypted_v1` | This migration, after vacuum/checkpoint | The one-time conversion is complete. |

- The marker is a normal `state` row (`value` is JSON `true`), read/written with the existing `migration_done`/`mark_migration_done` helpers or an equivalent.
- The marker is not written if any table conversion, `secure_delete`, checkpoint or `VACUUM` failed.
- Do not use the marker to decide whether a value needs encryption; inspect the value. The marker only skips the whole pass.

## 5. Removing plaintext from the data files

`UPDATE` alone leaves the old plaintext in SQLite free pages and the WAL. The migration must:

1. Set `PRAGMA secure_delete=ON` on the connection before the updates.
2. Perform all writes in a transaction and commit.
3. Run `PRAGMA wal_checkpoint(TRUNCATE)`.
4. Run `VACUUM` outside the transaction.
5. Run `PRAGMA wal_checkpoint(TRUNCATE)` again.
6. Only then write the marker.

The post-condition is: searching `vizier.db` and `vizier.db-wal` for any migrated plaintext returns no match. A `VACUUM` failure (for example insufficient disk) is a startup failure; the data remains encrypted, the marker is absent, and the next start retries.

If the existing `sqlite-vec` `document_index` virtual table rejects `VACUUM`, use `VACUUM INTO <temp>` on the same connection, close and atomically replace `vizier.db` (remove `-wal`/`-shm`), reopen and validate; the post-condition is the same. Verify both FTS5 (`memory_fts`) and `document_index` in the migration test.

`PRAGMA secure_delete=ON` is a connection setting; it does not need to be persisted. Do not run `VACUUM` while other storage handles or prepared statements are active. The migration holds the one `Arc<Mutex<Connection>>` for its duration.

## 6. Legacy handling

| Legacy data | Action | Why |
|---|---|---|
| Pre-015 `provider_config` row (`LegacyProviderEntry`) | Already rewritten by `migrate_provider_ids_v1`; encryption then covers the current `ProviderEntry`. | One provider shape. |
| Pre-016 `AgentConfig` keys (`discord_token`, `telegram_token`, `tools.discord`, `tools.telegram`, `tools.brave_search`, `tools.mcp_servers`) | Drop by deserializing/serializing `AgentConfig`; do not encrypt dead keys. | The current code already ignores them; leaving them would leave plaintext tokens forever. |
| Unknown integration kind | Whole `settings` JSON in one envelope. | No field classification is available. |
| `global_config` shell env | Encrypt. | Non-destructive; the table is unused but may still hold old credentials. |

An unparseable row in a protected table aborts the migration with an actionable error naming the table and record id. It is not skipped: a skip could leave a plaintext credential in place. The operator can fix or remove the row (or restore a backup) and start again.

## 7. Failure messages

Errors name the table/record/field and the action, and never include a value, ciphertext or key:

- `encrypted data exists but no encryption key was found; set VIZIER_ENCRYPTION_KEY or restore the key file at {path}`
- `encryption key is malformed: expected 32 bytes encoded as base64`
- `encryption key is unreadable: {path}: {io error}`
- `encryption key does not match this data`
- `encrypted value in {table} ({record}, {field}) failed its integrity check`
- `encrypted value in {table} ({record}, {field}) uses an unknown format version`
- `could not read {table} row {record} during the encryption migration; fix or remove the row`
- `encryption migration could not reclaim plaintext from the database (VACUUM/checkpoint failed): {error}`
- `could not write the encryption key file at {path}: {error}`

## 8. #82 removal unit

Add one checklist entry to #82:

- `migrate_secrets_to_encrypted`, its marker `migration.secrets_encrypted_v1`, the plaintext branch (`open`/`decrypt_legacy`), the pre-016 agent-key scrub, the unknown-kind whole-settings fallback if it exists only for pre-031 data, and their tests.
- **Check before removing:** the `SecretCipher`, key sources, key id, AAD, envelope parser/version reader and field-level storage helpers are permanent; do not list them. The `global_config` table/reader cleanup may be a separate #82 item.
- The 1.0 upgrade notes must tell operators to install the last pre-1.0 release (which runs this migration) before 1.0.0.

## 9. Test obligations

1. Plaintext fixture → run → all protected fields are envelopes; every non-secret sibling is unchanged and readable.
2. Raw `vizier.db` and `vizier.db-wal` do not contain the fixture's secret strings after the run.
3. Second run logs zero conversions and changes no row.
4. Interrupted before commit, after commit, after vacuum and before marker: each restart completes safely.
5. Wrong key aborts before any write; missing key aborts before generation; malformed key aborts.
6. Legacy agent keys are gone; `core` is preserved for `migrate_agent_cores`.
7. Unknown integration kind settings decrypt and load again.
8. A tampered/invalid row fails the migration with a record-level error, not a silent skip.
9. `VACUUM` failure leaves the marker absent and the next start retries.
10. The dummyplug quickstart covers the user-visible set/get/restart path.
