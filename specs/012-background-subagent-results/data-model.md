# Data Model: Background Subagent Results

**Feature**: `012-background-subagent-results` · **Date**: 2026-10-09

## Persisted (SQLite)

Created by a new `init_background_job_schema(conn)` in `storage/sqlite/mod.rs`, next to `init_task_run_schema`.

### `background_job`

| Column | Type | Notes |
|---|---|---|
| `id` | TEXT PK | short random id, e.g. `b-7f3a9c`, quoted in the tool's acknowledgement and in the report |
| `kind` | TEXT | `batch` \| `delegation` |
| `origin_agent` | TEXT | the agent that launched the job and receives the report |
| `origin_channel` | TEXT | `VizierChannelId` as JSON |
| `origin_topic` | TEXT NULL | |
| `depth` | INTEGER | the `background_depth` of the turn that launched the job (0–2; launching is refused at 3) |
| `timeout_secs` | INTEGER | the time limit for each piece |
| `created_at` | INTEGER | Unix milliseconds |
| `finished_at` | INTEGER NULL | |
| `state` | TEXT | see the state machine below |
| `cancelled_by` | TEXT NULL | `agent:<id>` or `person:<username>` when the job was cancelled |
| `reason` | TEXT NULL | why it was cancelled, or why delivery failed |

Indexes:

- `(origin_agent, origin_channel, origin_topic, state)`: the tray read, the topic-list badge, and the cancel cascade (running jobs launched from a piece's session).
- `(origin_agent, state)`: `list_background_jobs`.
- `(state)`: the startup sweep.

### `background_piece`

| Column | Type | Notes |
|---|---|---|
| `job_id` | TEXT FK → `background_job.id` | `ON DELETE CASCADE` |
| `ordinal` | INTEGER | 0-based position in the original task order. PK is `(job_id, ordinal)` |
| `prompt` | TEXT | the task as given |
| `executor_agent` | TEXT | the agent running the piece: the origin agent for a batch, the target agent for a delegation |
| `session_channel` | TEXT | `VizierChannelId` as JSON: `Subagent` or `InterAgent([origin, target])` |
| `session_topic` | TEXT | a fresh uuid per piece (research Decision 7) |
| `started_at` | INTEGER | |
| `finished_at` | INTEGER NULL | |
| `state` | TEXT | `running` \| `answered` \| `failed` \| `timed_out` \| `cancelled` \| `interrupted` |
| `reason` | TEXT NULL | why a `failed`, `timed_out`, `cancelled` or `interrupted` piece did not answer |

There is no result column. An answered piece's text is the last message with content in its session `(executor_agent, session_channel, session_topic)`, read through the existing history storage. The report carries a truncated copy.

Deleting a topic (`delete_session`) also deletes the jobs launched from it, so no orphaned rows remain to authorize against.

### Job state machine

```
                launch
                  │
                  ▼
             ┌─────────┐  every piece final   ┌───────────┐ report sent  ┌──────────┐
             │ running │ ───────────────────► │ reporting │ ───────────► │ reported │
             └─────────┘   (runner wins the   └───────────┘              └──────────┘
               │  │  │      'running' guard)        │ delivery failed 30s ┌─────────────┐
               │  │  │                              └───────────────────► │ undelivered │
               │  │  │  cancel (wins the 'running' guard)                 └─────────────┘
               │  │  └─────────────────────────────────────────────────► ┌───────────┐
               │  │                                                       │ cancelled │
               │  │                                                       └───────────┘
               │  └─ process restarted (startup sweep: running|reporting) ┌─────────────┐
               └────────────────────────────────────────────────────────► │ interrupted │
                                                                          └─────────────┘
```

`running` and `reporting` are the non-terminal states. **Cancel and the runner race on one guard**, `UPDATE … SET state = ? WHERE id = ? AND state = 'running'`. The runner moves to `reporting` *before* delivering, so a job is either cancelled or reported, never both (FR-029, research D12). The tray and the badge count `running` and `reporting` together as in flight.

Pieces have their own states: `running` → `answered` | `failed` | `timed_out` | `cancelled`, plus `interrupted` from the sweep. A job leaves `running` only once every one of its pieces has, or when it is cancelled. Cancelling closes its still-running pieces as `cancelled` in the same transaction.

## Storage trait

`src/storage/background_job.rs`:

```rust
#[async_trait::async_trait]
pub trait BackgroundJobStorage {
    /// Job and pieces in one transaction, before any piece is dispatched.
    async fn open_background_job(&self, job: BackgroundJob) -> Result<()>;
    async fn close_background_piece(&self, job_id: &str, ordinal: u32, state: PieceState,
                                    reason: Option<String>, finished_at: DateTime<Utc>) -> Result<()>;
    /// Guarded transition out of `running` (to `reporting` or `cancelled`), or out of
    /// `reporting` (to `reported`/`undelivered`). Returns false if the guard did not match, i.e.
    /// someone else already moved the job. Cancelling also closes the job's still-running pieces.
    async fn transition_background_job(&self, job_id: &str, from: JobState, to: JobState,
                                       cancelled_by: Option<String>, reason: Option<String>,
                                       at: DateTime<Utc>) -> Result<bool>;
    async fn get_background_job(&self, job_id: &str) -> Result<Option<BackgroundJob>>;
    /// The tray read and the cancel cascade: in-flight jobs launched from `origin`, oldest first.
    async fn list_running_background_jobs(&self, origin: VizierSession) -> Result<Vec<BackgroundJob>>;
    /// `list_background_jobs`: in-flight jobs launched by `agent_id` from any of its sessions.
    async fn list_agent_running_background_jobs(&self, agent_id: AgentId) -> Result<Vec<BackgroundJob>>;
    /// The topic-list badge: running-job counts for one agent and channel, by topic.
    async fn count_running_background_jobs(&self, agent_id: AgentId, channel: VizierChannelId)
                                           -> Result<HashMap<Option<TopicId>, usize>>;
    /// Startup sweep: every `running`/`reporting` job and `running` piece becomes `interrupted`.
    async fn interrupt_open_background_jobs(&self) -> Result<usize>;
}
```

It is added to `VizierStorageProvider` and forwarded by `VizierStorage`.

## In-process types (`src/schema/background.rs`)

```rust
pub type BackgroundJobId = String;

pub enum JobKind { Batch, Delegation }
pub enum JobState { Running, Reporting, Reported, Undelivered, Cancelled, Interrupted }
pub enum PieceState { Running, Answered, Failed, TimedOut, Cancelled, Interrupted }
pub enum Canceller { Agent(AgentId), Person(String) }

pub struct BackgroundJob {
    pub id: BackgroundJobId,
    pub kind: JobKind,
    pub origin: VizierSession,
    pub depth: u8,
    pub timeout_secs: u64,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub state: JobState,
    pub cancelled_by: Option<Canceller>,
    pub reason: Option<String>,
    pub pieces: Vec<BackgroundPiece>,
}

pub struct BackgroundPiece {
    pub ordinal: u32,
    pub prompt: String,
    pub session: VizierSession,     // (executor_agent, channel, Some(topic))
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub state: PieceState,
    pub reason: Option<String>,
}

/// What `VizierRequestContent::BackgroundReport` carries; rendered for the model by `Display`.
pub struct BackgroundReport {
    pub job_id: BackgroundJobId,
    pub kind: JobKind,
    pub delegated_to: Option<AgentId>,
    pub entries: Vec<ReportEntry>,  // one per piece, in ordinal order
}

pub struct ReportEntry {
    pub ordinal: u32,
    pub prompt: String,
    pub state: PieceState,          // never Running/Interrupted; Cancelled only in a cancel's tool result
    pub text: String,               // answer (truncated) or reason
    pub truncated: bool,
}
```

`BackgroundJobSnapshot`, the WebUI wire shape, is `BackgroundJob` with `elapsed_ms` per piece and the piece session flattened to `{agent_id, topic}`. See `contracts/http-api.md`.

## Changes to existing types

| Type | Change | Default and back-compatibility |
|---|---|---|
| `VizierRequest` | `+ background_depth: u8` | `#[serde(default)]` = 0, so stored history deserializes unchanged |
| `VizierRequestContent` | `+ BackgroundReport(BackgroundReport)` | new variant only. The WebUI type union gains `{ background_report: BackgroundReport }` |
| `ToolContext` | `+ background_depth: u8` | 0 at the three construction sites that are not `chat`: session-detail titling, dream and python |
| `VizierTransport` | `+ session_events` (broadcast) | — |
| `VizierDependencies` | `+ background_jobs: BackgroundJobs` | — |
| `TopicEntry` (HTTP) | `+ running_jobs: usize` | `0` when there are none |

## Validation rules

| Rule | Source | Where |
|---|---|---|
| `tasks` is non-empty | Edge case "empty batch" | `SubtasksTool::call` → tool error |
| `timeout_secs` in 1..=3600 if given | Decision 8 | both tools → tool error |
| `ctx.background_depth < 3` | FR-012 | `BackgroundJobs::launch` → tool error naming the limit |
| target agent exists and is registered | Acceptance 2.2 | `DelegateAgent::call` before launch → tool error, no job row |
| a report has exactly one entry per piece, in ordinal order | FR-007, SC-004 | the report is built from the piece rows `ORDER BY ordinal` |
| an agent can list and cancel only jobs where `origin_agent` is itself | FR-025 | `BackgroundJobs::cancel` / list → a foreign job reads as "no running job" |
| a job ends exactly once: reported or cancelled | FR-029 | the guarded `transition_background_job` |

## In-memory state

`BackgroundJobs.cancels: Mutex<HashMap<BackgroundJobId, watch::Sender<bool>>>` holds one entry per live runner. It is inserted at launch and removed when the runner exits. It only *signals* the runner. The database guard decides whether a cancel took effect, so a lost entry (after a restart) cannot break the exactly-once rule.
