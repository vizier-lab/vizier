# Contract: Project Channel and Migration

**Feature**: `041-project-coding-workspace` (FR-014–FR-016a)

---

## 1. Shapes

| | 040 | 041 |
|---|---|---|
| Channel | `HTTP("alice", "project__p3x")` | `Project("alice", "p3x")` |
| Slug | `http__alice__project__p3x` | `project__alice__p3x` |
| JSON | `{"HTTP":["alice","project__p3x"]}` | `{"Project":["alice","p3x"]}` |
| Wire `channel_id` (URLs, socket, DTOs) | `project__p3x` | `project__p3x` (unchanged) |
| Usage activity / place | `chat` / `http:http__alice__project__p3x:{topic}` | `project` / `project:project__alice__p3x:{topic}` |

## 2. One constructor, one accessor

```rust
impl VizierChannelId {
    /// The channel a web client means by `channel_id`, for the signed-in `username`.
    pub fn from_web(username: &str, channel_id: &str) -> Self;
    /// The person whose WebUI or project chat this is.
    pub fn web_user(&self) -> Option<&str>;
    /// The wire `channel_id` of a web channel (`HTTP` → its id, `Project` → `project__<id>`).
    pub fn web_channel_id(&self) -> Option<String>;
    /// The project id of a `Project` channel.
    pub fn project_id(&self) -> Option<&str>;
}
```

Every site that built `VizierChannelId::HTTP(user, channel_id)` from request data calls
`from_web`; every site that matched `HTTP(user, _)` to find the person calls `web_user()`.
`projects::channel` keeps only the wire prefix (`PREFIX`, `channel_id`, `project_id` on a wire
id); `project_of_slug` / `project_channel_of_slug` are deleted, and their callers
(`conversation_rows`, `kind_of`, `notifications`, `topics`) match the variant.

## 3. Behaviour by consumer

| Consumer | Behaviour for `Project` |
|---|---|
| Regular sessions list, search, picker (`conversation_rows`, `kind_of`) | excluded (by variant) |
| `project_sessions` | the caller's `project__{user}__{id}` slug |
| Notifications `chat_reply` | audience = `web_user()`; href `/<agent>/projects/<id>/chat/<topic>` |
| Client tools, deferral, socket broadcast | as `HTTP` (via `web_user()`/`web_channel_id()`) |
| Usage | `UsageActivity::Project`, place `project:…`, label = project name + chat title |
| Dream cycle | included (`is_non_user_channel` false); dream sessions get no `ProjectTurnContext` |
| `ProjectTurnContext::for_session` | only for `Project`; re-reads owner permission each turn (040) |
| Access (`service/topics`) | project chats authorised by `projects::access` (040) via the variant |

A test pins: `is_non_user_channel("project__alice__p3x") == false`, and a dream over a project chat
gets no `coding` tools.

## 4. Migration `migrate_project_channels`

- Location: `crates/vizier-core/src/projects/migrate.rs`, called from `dependencies.rs` after the
  schema is initialised and before the agents start.
- Guard: state key `migration.project_channel_v1`; one transaction; a failure aborts startup
  (as other migrations).
- Old-slug discovery: `channel LIKE 'http\_\_%\_\_project\_\_%' ESCAPE '\'` in `session_history`,
  `session_detail`, `session_turn`, parsed by the old-shape parser (user = between `http__` and
  `__project__`, id validated as 040).

| Table | Column | Rewrite |
|---|---|---|
| `session_history` | `channel` | exact, and `REPLACE` inside `DREAM__…` slugs |
| `session_detail` | `channel` | exact, and inside `DREAM__…` |
| `session_detail` | `id` | `REPLACE(old, new)` |
| `session_detail` | `data` | JSON `channel` → `{"Project":[user,id]}` |
| `session_turn` | `channel` | exact |
| `session_turn` | `request` | JSON `session[1]` → `{"Project":[user,id]}` |
| `session_file` | `session_slug` | `REPLACE` |
| `scratch_file` | `channel` | exact |
| `client_tool_call` | `channel` | exact |
| `artifact` | `origin_channel` | exact |
| `artifact_event` | `source_channel` | exact |
| `artifact_reference` | `channel` | exact |
| `usage_event` | `channel`, `place`, `activity` | exact; place `http:<old>:` → `project:<new>:`; activity → `project` |
| `background_job` | `origin_channel` | exact |
| `background_piece` | `session_channel` | exact |

The implementation task re-derives this list from every `CREATE TABLE` in `storage/sqlite/` and
from every struct serialised with a `VizierSession`/`VizierChannelId` into a column; any table
found beyond this list is added here.

- Test: build a database in the 040 shape with one row per table above for
  `http__alice__project__p3x` plus an ordinary `http__alice__vizier-webui` row; run; assert no
  `http__alice__project__` remains anywhere, every row now says `project__alice__p3x` (or the JSON
  form), and the ordinary row is untouched. Run twice: the second run is a no-op.
- #82: add "`projects::migrate::migrate_project_channels` and its marker
  `migration.project_channel_v1` (rewrites 040 project slugs); remove with the 040-shaped test
  fixture."
