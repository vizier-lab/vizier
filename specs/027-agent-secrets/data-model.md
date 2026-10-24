# Data Model: Agent Secrets

**Feature**: `027-agent-secrets` | **Date**: 2026-10-24

## 1. Table `agent_secret`

Created by `init_secret_schema(conn)` in `src/storage/sqlite/mod.rs`, called from `init_schema` after `init_turn_schema`.

```sql
CREATE TABLE IF NOT EXISTS agent_secret (
    agent_id   TEXT NOT NULL,
    name       TEXT NOT NULL,
    value      TEXT NOT NULL,              -- plaintext, host/disk protected (R1)
    created_at TEXT NOT NULL,              -- RFC 3339
    updated_at TEXT NOT NULL,              -- RFC 3339
    updated_by TEXT NOT NULL,              -- JSON Requester: {"user":"…"} | {"agent":"…"}
    PRIMARY KEY (agent_id, name)
);
```

| Field | Rule |
|---|---|
| `agent_id` | The owning agent; every query is scoped by it. |
| `name` | 1–128 bytes, ASCII `[A-Za-z0-9._-]`, case-sensitive (FR-003). |
| `value` | 8–65,536 bytes of UTF-8, stored exactly as supplied (FR-003, FR-007). |
| `created_at` | Set on insert; never changes on replace. |
| `updated_at` | Set on insert and every replace. |
| `updated_by` | `Requester` JSON for the actor of the last change. |

There is no extra index: the primary key's `(agent_id, name)` prefix serves get, list, set and delete, and `agent_id` alone serves agent deletion.

## 2. Table `agent_secret_event`

```sql
CREATE TABLE IF NOT EXISTS agent_secret_event (
    id       TEXT PRIMARY KEY,             -- UUID v4
    agent_id TEXT NOT NULL,
    name     TEXT NOT NULL,
    action   TEXT NOT NULL,                -- created | replaced | deleted
    actor    TEXT NOT NULL,                -- JSON Requester
    at       TEXT NOT NULL                 -- RFC 3339
);
CREATE INDEX IF NOT EXISTS idx_agent_secret_event_agent
    ON agent_secret_event(agent_id, at DESC);
```

No value, masked value or hash is stored. Events are append-only; deleting an agent deletes its events. A failed validation writes nothing.

## 3. Schema types (`src/schema/secret.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretAction {
    Created,
    Replaced,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSecretEvent {
    pub id: String,
    pub agent_id: String,
    pub name: String,
    pub action: SecretAction,
    pub actor: Requester,
    pub at: DateTime<Utc>,
}
```

`Requester` is the existing `User(String)` / `Agent(AgentId)` enum from `src/schema/task.rs`. It serialises as `{"user":"…"}` or `{"agent":"…"}`; storage parses it back with `serde_json`.

## 4. Storage values (`src/storage/secret.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretOutcome {
    Created,
    Replaced,
}

/// A secret as list/set responses may see it: never the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRecord {
    pub name: String,
    pub masked: String,          // utils::mask_secret
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Requester,
}

pub struct SecretWrite {
    pub outcome: SecretOutcome,
    pub record: SecretRecord,
}
```

`SecretRecord` is deliberately not `Serialize`: the HTTP handler maps it to the API DTO, so a future accidental serialization cannot leak a value (there is no value field to leak).

## 5. Storage trait

```rust
#[async_trait::async_trait]
pub trait SecretStorage {
    /// Create or replace one value, and record the event, in one transaction.
    async fn set_agent_secret(
        &self,
        agent_id: &str,
        name: &str,
        value: &str,
        actor: &Requester,
    ) -> Result<SecretWrite>;

    /// The only method that returns a stored value.
    async fn get_agent_secret(&self, agent_id: &str, name: &str) -> Result<Option<String>>;

    /// Metadata for every secret of one agent, sorted by name. No values.
    async fn list_agent_secrets(&self, agent_id: &str) -> Result<Vec<SecretRecord>>;

    /// Delete one value and record the event when a row existed.
    async fn delete_agent_secret(
        &self,
        agent_id: &str,
        name: &str,
        actor: &Requester,
    ) -> Result<bool>;

    /// Newest first, capped by `limit`.
    async fn list_agent_secret_events(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<AgentSecretEvent>>;
}
```

`SecretStorage` joins the `VizierStorageProvider` supertrait list; `VizierStorage` forwards every method; `SqliteStorage` implements it in `src/storage/sqlite/secret.rs`.

## 6. Service (`src/secrets/service.rs`)

The one place validation and actor handling meet, used by both tools and HTTP:

```rust
pub enum SecretError {
    InvalidName(String),
    InvalidValue(String),
    NotFound,
    Storage(anyhow::Error),
}

pub async fn set(
    storage: &VizierStorage,
    agent_id: &str,
    name: &str,
    value: &str,
    actor: &Requester,
) -> Result<SecretWrite, SecretError>;

pub async fn get(
    storage: &VizierStorage,
    agent_id: &str,
    name: &str,
) -> Result<String, SecretError>;

pub async fn delete(
    storage: &VizierStorage,
    agent_id: &str,
    name: &str,
    actor: &Requester,
) -> Result<(), SecretError>;

pub async fn list(
    storage: &VizierStorage,
    agent_id: &str,
) -> Result<Vec<SecretRecord>, SecretError>;

pub async fn events(
    storage: &VizierStorage,
    agent_id: &str,
    limit: usize,
) -> Result<Vec<AgentSecretEvent>, SecretError>;
```

`get` validates the name first so an invalid name is a readable refusal rather than a misleading not-found. A missing valid name is `NotFound`.

## 7. Validation (`src/secrets/mod.rs`)

```rust
pub const MAX_NAME_BYTES: usize = 128;
pub const MIN_VALUE_BYTES: usize = 8;
pub const MAX_VALUE_BYTES: usize = 65_536;

pub fn validate_name(name: &str) -> Result<(), String>;
pub fn validate_value(value: &str) -> Result<(), String>;
```

- Name: 1–128 bytes, every byte an ASCII letter, digit, `.`, `_` or `-`.
- Value: 8–65,536 bytes of UTF-8. No trimming, normalisation or newline conversion.
- Messages name the rule and are safe to show to a person or return to a script.

## 8. Redactor (`src/secrets/redact.rs`)

```rust
pub const MARKER: &str = "[redacted]";

#[derive(Default)]
pub struct SecretRedactor {
    values: parking_lot::Mutex<Vec<String>>,
}

impl SecretRedactor {
    /// Called only by set_secret and get_secret. Ignores values shorter than
    /// MIN_VALUE_BYTES and duplicates. Keeps at most 64 values per turn.
    pub fn arm(&self, value: &str);

    pub fn is_armed(&self) -> bool;

    /// Exact substring replacement, longest value first.
    pub fn text(&self, input: &str) -> String;

    /// Recursively scrubs every string in a JSON value.
    pub fn json(&self, value: &mut serde_json::Value);

    /// result, stdout, error.message, error.traceback and every nested
    /// tool call's arguments/error.
    pub fn report(&self, report: &mut ExecutionReport);
}
```

`ToolContext` gains `pub secret_redactor: Arc<SecretRedactor>`. It is constructed empty in every context; the chat turn creates one and shares it with `execute_python`, the bridge and telemetry. It is dropped with the turn. Before `arm` is called, `text`/`json`/`report` return immediately.

Redaction is exact-string only. Transformed values (base64, URL-encoding, hashing) are outside the guarantee (R3, spec assumptions).

## 9. Lifecycle

```text
              set_secret (new name)              set_secret (existing name)
  (absent) ────────────────────────▶ present ────────────────────────────▶ present
       ▲                                │                                   │
       │  delete_secret                 │  delete_secret                    │  value replaced
       └────────────────────────────────┘                                   │
                                                                              │
  agent deleted ──▶ both tables' rows for that agent are gone                 │
```

- A set never leaves two rows: the primary key plus `ON CONFLICT` enforce one.
- A replace preserves `created_at` and changes `updated_at`/`updated_by`.
- A value already read by a running script is unaffected by a later replace or delete.
- Agent update/restart and vizier restart need no hook; the next get reads the current row.
- `SqliteStorage::delete_agent` deletes both tables before deleting the agent's other rows, on its held connection.
- No startup migration or sweep: the tables only ever hold complete rows.

## 10. Explicitly not stored

- Secret values in `agent_secret_event`, logs, telemetry, history, reports or DTOs.
- Secret names or values in `AgentConfig`, so an agent update cannot restart because of a secret.
- A per-agent switch or grant: the tools are always available.
- Version history, expiry, rotation metadata or sharing.
