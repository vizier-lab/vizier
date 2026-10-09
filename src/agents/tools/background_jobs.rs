use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    agents::{
        background::{CancelOutcome, CancelScope, report::render_cancel_result},
        tools::{ToolContext, VizierTool},
    },
    dependencies::VizierDependencies,
    error::VizierError,
    schema::{BackgroundJob, Canceller, JobKind, PieceState, VizierChannelId},
    storage::{background_job::BackgroundJobStorage, session::SessionStorage},
};

pub struct ListBackgroundJobs {
    deps: VizierDependencies,
}

impl ListBackgroundJobs {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }

    /// Where a job was launched from, as a person would name it.
    async fn origin_label(&self, job: &BackgroundJob) -> String {
        let origin = &job.origin;
        match &origin.1 {
            VizierChannelId::HTTP(..) => {
                let title = self
                    .deps
                    .storage
                    .get_session_detail_by_topic(origin.0.clone(), origin.1.clone(), origin.2.clone())
                    .await
                    .ok()
                    .flatten()
                    .map(|detail| detail.title)
                    .filter(|title| !title.is_empty());
                match (title, &origin.2) {
                    (Some(title), _) => format!("webui topic \"{title}\""),
                    (None, Some(topic)) => format!("webui topic {topic}"),
                    (None, None) => "webui".to_string(),
                }
            }
            VizierChannelId::DiscordChanel(id) => format!("discord channel {id}"),
            VizierChannelId::TelegramChannel(id) => format!("telegram chat {id}"),
            other => other.to_slug(),
        }
    }
}

fn elapsed(job: &BackgroundJob) -> String {
    let secs = (Utc::now() - job.created_at).num_seconds().max(0);
    format!("{}m {:02}s", secs / 60, secs % 60)
}

fn first_line(prompt: &str) -> &str {
    prompt.lines().next().unwrap_or("").trim()
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListBackgroundJobsArgs {}

#[async_trait::async_trait]
impl VizierTool for ListBackgroundJobs {
    type Input = ListBackgroundJobsArgs;
    type Output = String;

    fn name() -> String {
        "list_background_jobs".to_string()
    }

    fn description(&self) -> String {
        "List the background jobs you have running (from paralel_subtasks or delegate_agent), across all your conversations, with their progress. Use it to find a job id to cancel.".into()
    }

    async fn call(&self, _args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let jobs = self
            .deps
            .storage
            .list_agent_running_background_jobs(ctx.session.0.clone())
            .await
            .map_err(|err| VizierError(format!("failed to list background jobs: {err}")))?;

        if jobs.is_empty() {
            return Ok("No background jobs running.".into());
        }

        let mut out = format!(
            "{} background job{} running:\n",
            jobs.len(),
            if jobs.len() == 1 { "" } else { "s" }
        );
        for job in &jobs {
            let origin = self.origin_label(job).await;
            let line = match (job.kind, job.delegated_to()) {
                (JobKind::Delegation, Some(target)) => format!(
                    "{} · delegation to {} · from {} · {}",
                    job.id,
                    target,
                    origin,
                    elapsed(job)
                ),
                _ => {
                    let done = job
                        .pieces
                        .iter()
                        .filter(|p| p.state != PieceState::Running)
                        .count();
                    format!(
                        "{} · batch · from {} · {}/{} done · {}",
                        job.id,
                        origin,
                        done,
                        job.pieces.len(),
                        elapsed(job)
                    )
                }
            };
            out.push('\n');
            out.push_str(&line);
            for piece in &job.pieces {
                out.push_str(&format!(
                    "\n  {}. {:<8} — {}",
                    piece.ordinal + 1,
                    piece.state.label(),
                    first_line(&piece.prompt)
                ));
            }
        }

        Ok(out)
    }
}

pub struct CancelBackgroundJob {
    deps: VizierDependencies,
}

impl CancelBackgroundJob {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CancelBackgroundJobArgs {
    #[schemars(description = "the job id, e.g. b-7f3a9c")]
    pub job_id: String,
    #[schemars(description = "[optional] why the job is being cancelled")]
    #[serde(default)]
    pub reason: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for CancelBackgroundJob {
    type Input = CancelBackgroundJobArgs;
    type Output = String;

    fn name() -> String {
        "cancel_background_job".to_string()
    }

    fn description(&self) -> String {
        "Cancel one of your running background jobs. Its unfinished pieces are stopped, any background work they started is cancelled too, and no report will arrive for it. Returns the results of the pieces that had already finished.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let agent = ctx.session.0.clone();
        let job_id = args.job_id.trim().to_string();
        let reason = args.reason.filter(|r| !r.trim().is_empty());

        // A job another agent launched reads exactly like an unknown id (FR-025).
        match self
            .deps
            .background_jobs
            .cancel(
                &job_id,
                Canceller::Agent(agent.clone()),
                reason.clone(),
                CancelScope::Agent(agent),
            )
            .await?
        {
            CancelOutcome::Cancelled {
                job,
                entries,
                nested_ids,
            } => Ok(render_cancel_result(
                &job,
                &entries,
                reason.as_deref(),
                &nested_ids,
            )),
            CancelOutcome::AlreadyFinished(job) => Ok(format!(
                "Background job {} already finished ({}).",
                job.id,
                job.state.as_str()
            )),
            CancelOutcome::NotFound => Err(VizierError(format!(
                "no running job {job_id} launched by you"
            ))),
        }
    }
}
