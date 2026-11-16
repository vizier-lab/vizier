# Data Model: HTML Generative UI (063)

## `ui_page` (SQL)

| Column | Type | Notes |
|---|---|---|
| `id` | text PK | `pg_` + 22-char base62 random |
| `agent_id` | text | owner agent; indexed |
| `channel` | text | the conversation's channel slug (`VizierChannelId::to_slug`) |
| `topic` | text | the conversation's topic (`''` for none) |
| `title` | text | 1–120 chars, trimmed; may change with a full replacement |
| `latest_version` | integer | ≥ 1 |
| `state` | text (JSON) null | the page's saved state, shared by its versions |
| `state_seq` | integer | +1 on every state save (cross-tab ordering) |
| `created_at`, `updated_at` | text | `time::stored` (UTC; sorted as text) |
| `owner_node`, `owner_token` | text, integer null | fence on `agent:{agent_id}` (spec 057) |

Indexes:
- `(agent_id, channel, topic)` for delete-by-conversation and listing.
- `(agent_id)` for delete-by-agent.

## `ui_page_version` (SQL)

| Column | Type | Notes |
|---|---|---|
| `page_id` | text | FK → `ui_page.id` `ON DELETE CASCADE` |
| `version` | integer | 1, 2, 3, …; PK `(page_id, version)` |
| `size` | integer | bytes of the HTML, ≤ 1 MiB |
| `sha256` | text | hex of the HTML |
| `origin` | text | `full` \| `edits` |
| `base_version` | integer null | null for version 1 |
| `edit_count` | integer null | for `edits` |
| `verification` | text (JSON) | see below; starts as `{"state":"pending"}` |
| `history_uid` | text null | the `UiPage` history entry that drew it |
| `created_at` | text | `time::stored` |
| `owner_node`, `owner_token` | text, integer null | fence |

The HTML itself is in the blob store at `ui-pages/{agent_id}/{page_id}/v{version}.html`.

### Verification (JSON)

```json
{ "state": "pending" }
{ "state": "unverified", "reason": "no_tab" | "timeout" | "closed" }
{ "state": "clean", "observed_ms": 2000 }
{ "state": "problems", "observed_ms": 2000, "problems": [Problem, …] }
```

`Problem` (also in `contracts/verify-protocol.md`):

```json
{ "kind": "error" | "rejection" | "console" | "blocked" | "kit" | "unresponsive",
  "message": "TypeError: board is undefined",
  "source": "page" | "kit",
  "line": 214, "column": 17,
  "detail": "img-src http://example.com/a.png" }
```

- `message` is cut to 500 characters.
- At most 20 problems per report; the rest are counted as `"more": N`.

## Transitions

`ui_page.latest_version`:

```
(new page)  ──create──▶ 1
N           ──full or edits based on N──▶ N+1      (guarded: WHERE latest_version = N)
N           ──based on M < N──▶ refused stale(latest = N)
```

`ui_page_version.verification`:

```
pending ──no responder──────────▶ unverified(no_tab)
pending ──deadline──────────────▶ unverified(timeout)
pending ──socket closed─────────▶ unverified(closed)
pending ──report, no problems───▶ clean
pending ──report, problems──────▶ problems
```

A recovered turn (`recover_agent`) does not re-verify. A `pending` row left by a stopped instance is set to `unverified(closed)` in the same sweep that claims the agent's scope.

## History: `SessionHistoryContent::UiPage`

```rust
UiPage {
    page_id: String,
    version: u32,
    title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    verification: Option<serde_json::Value>, // filled on read (fill_ui_pages), never stored
}
```

- It is saved by `record_step` after the version row commits.
- It is skipped by `history_entries_to_messages`.
- Its uid is written back to `ui_page_version.history_uid`.

## Requests

- `VizierRequestContent::ClientEvent` gains an optional `page: Option<PageSource { page_id, version }>`, mutually exclusive with the existing source call. It is frontmattered `via: client_event`, with `page: <id> v<n>`.

## Storage trait: `UiPageStorage`

Added to the `VizierStorageProvider` supertrait. Every write is fenced on `agent:{agent_id}`.

| Method | Purpose |
|---|---|
| `create_ui_page(agent, session, title, html_meta) -> UiPageRecord` | page + version 1 in one transaction |
| `add_ui_page_version(page_id, base, meta, title?) -> Result<u32, Stale(latest)>` | guarded increment and version row |
| `get_ui_page(page_id) -> Option<UiPageRecord>` | read |
| `list_ui_page_versions(page_id) -> Vec<UiPageVersionRecord>` | read |
| `set_ui_page_verification(page_id, version, json)` | the verification outcome |
| `set_ui_page_history_uid(page_id, version, uid)` | link the version to its history entry |
| `save_ui_page_state(page_id, state, expected_seq?) -> u64` | the page's state; returns the new `state_seq` |
| `fill_ui_pages(entries)` | history read helper |
| `delete_ui_pages_for_session` / `delete_ui_pages_for_agent` | return `(agent, page)` pairs for blob cleanup |
| `sweep_pending_ui_verifications(agent, older_than_token)` | recovery |

The HTML bytes go through `BlobStore` from the service (`crates/vizier-core/src/ui_pages/mod.rs::UiPages`), not through the trait.

## Validation rules (from the spec)

| Rule | Source |
|---|---|
| `title` 1–120 chars after trim | R15 |
| `html` non-empty, ≤ 1 MiB UTF-8 | FR-011 |
| `edits` 1–100; `find` non-empty; each `find` exactly once when applied | FR-007, FR-008 |
| `base_version` = `latest_version` | FR-009 |
| page's `(agent, channel, topic)` = caller's session | FR-010 |
| state JSON ≤ `MAX_STATE_BYTES` | R15 |
