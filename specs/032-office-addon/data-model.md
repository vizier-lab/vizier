# Data Model: Office Addon (DOCX and XLSX)

No new tables, columns or migrations. The feature adds one integration *kind* (code), reuses four existing records, and adds one persisted flag.

## Existing records reused

### Integration (table `integration` + `integration_attachment`)

| Field | Value for the office addon |
|---|---|
| `kind` | `"office"` |
| `family` | `Addon` |
| `name` | `"Office"` (default; unique per agent) |
| `settings` | `{}` — no fields; `null`/`{}` both valid |
| `enabled` | `true` when seeded |
| `creator` | `Requester::User(<agent owner id>)`, or `Requester::User("system")` when the agent has no owner |
| attachment | exactly one agent (017 ownership); at most one `office` per agent (`409 addon_installed`) |

State transitions are the existing ones: **seeded/installed → (disabled ↔ enabled) → deleted**. Deleted is terminal until a person installs it again; nothing automatic leaves or re-enters that state.

### Integration event (table `integration_event`)

One `created` row per seeding, written in the same transaction as the integration (existing `save_integration`). `actor` is the requester above; `fields` empty.

### Session file (table `session_file`, blob in `FileManager`)

Written by the write tools. Relevant fields:

| Field | Value |
|---|---|
| `filename` | normalised name ending `.docx` / `.xlsx` |
| `mime_type` | the explicit OOXML type (never guessed from the extension) |
| `sender` | the agent id (set by `save_session_file`); not used for any decision |
| `size`, `file_id` | from `FileManager::upload` |

Rule (FR-027): a write for an existing session and filename replaces the record, as the other file tools do. No ownership check, no schema change.

### State key (table `state`)

| Key | Meaning |
|---|---|
| `office.default_seeded` | Present once the one-time backfill of pre-existing agents has finished. Never cleared. Constant `OFFICE_DEFAULT_SEEDED` in `dependencies.rs`. |

## New in-memory types (no persistence)

These are the tool I/O shapes; the JSON contracts are in [contracts/office-tools.md](contracts/office-tools.md).

### `Cell` (workbook read/write)

A tagged scalar. Variants and their JSON forms:

| Variant | JSON | Notes |
|---|---|---|
| Text | `"abc"` | on write, always stored as text (FR-028) |
| Number | `12.5`, `3` | |
| Bool | `true` / `false` | |
| Date | `{"date": "2026-10-25"}` or `{"date": "2026-10-25T14:30:00"}` | ISO-8601; read returns the same object form |
| Empty | `null` | blank cell |
| Formula | `{"value": <Cell or null>, "formula": "=SUM(A1:A3)"}` | read only, and only when `formulas: true`; never written, never executed (FR-028) |
| Error | `{"error": "#DIV/0!"}` | read only |

### `Sheet`

`{ "name": string, "rows": Cell[][] }`. Name rules (write): 1–31 characters, none of `[ ] : * ? / \`, unique case-insensitively within the workbook. Rows may be ragged on write (shorter rows are padded blank).

### `WorkbookRead`

`{ "sheets": [ { "name", "used_range": string | null, "dimensions": {"rows": n, "cols": n}, "start"?: string, "rows"?: Cell[][] } ], "truncated": bool, "note"?: string }`. `dimensions` reports the *full* size of the used range so the agent knows what was cut; `used_range` is that area's A1 address and `start` the A1 address of the first cell of `rows`. A read with `info: true` carries neither `start` nor `rows`.

### `DocxRead`

`{ "markdown": string, "truncated": bool, "note"?: string }`.

### `WriteResult`

`{ "filename": string, "size": u64 }` — size in bytes of the stored file.

### Limits (constants in `office/mod.rs`; starting values, see research R7)

| Constant | Value | Origin |
|---|---|---|
| `READ_MAX_CHARS` | 200 000 | issue (~200 KB) |
| `READ_MAX_SHEETS` / `READ_MAX_ROWS` / `READ_MAX_COLS` | 20 / 1 000 / 100 | issue says "cap sheets/rows"; numbers are ours |

No other limits (owner's decision for the proof of concept). Excel's own format limits come back as ordinary writer errors.

## Relationships

```text
Agent 1 ──── 0..1 Integration(kind="office", family=Addon)
Integration 1 ──── 1..n IntegrationEvent  (created, edited, enabled, disabled, deleted)
Agent/session 1 ──── 0..n SessionFile      (produced by office_*_write_*, sender = agent id)
state["office.default_seeded"]  — global, set once after the backfill
```
