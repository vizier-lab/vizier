//! Lease-based recovery (`specs/057-fleet-row-ownership/contracts/recovery.md`): what a new owner
//! of a scope does with the in-flight work its predecessor left. It replaces the startup sweeps
//! of `VizierDependencies::new` and `AgentHost::resend_pending_turns`.
//!
//! Every step acts only on rows whose `owner_token` is null or older than the new fence, and
//! re-owns each row in the statement that resolves it, so a pass repeated, or raced by another
//! recoverer of the same scope, resolves nothing twice. In a single node one start claims every
//! agent and `control` once, which reproduces what the sweeps did.

use crate::dependencies::VizierDependencies;
use crate::schema::AgentId;
use crate::storage::client_call::ClientCallStorage;
use crate::storage::fence::{Fence, is_ownership_lost};
use crate::storage::question::SensitiveAskStorage;
use crate::storage::scratch::ScratchStorage;
use crate::storage::session::SessionStorage;
use crate::storage::task::TaskStorage;
use crate::storage::task_run::TaskRunStorage;
use crate::storage::turn::{TurnState, TurnStorage};

/// The outcome of one recovery pass. `complete` is false when a step failed, and the caller
/// should run the pass again later; a repeat does nothing twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recovered {
    pub complete: bool,
}

/// Log a failed step; a lost fence needs no retry, because the owner is being released.
fn failed(step: &str, scope: &str, err: &anyhow::Error, recovered: &mut Recovered) {
    if is_ownership_lost(err) {
        tracing::debug!(scope, "recovery stopped at {step}: {err}");
    } else {
        tracing::warn!(scope, "recovery step '{step}' failed: {err}");
        recovered.complete = false;
    }
}

/// After `claim(Scope::Agent(id), ..)` and before the agent's instance starts.
pub async fn recover_agent(
    deps: &VizierDependencies,
    agent_id: &AgentId,
    fence: &Fence,
) -> Recovered {
    let mut recovered = Recovered { complete: true };
    let (storage, below) = (&deps.storage, fence.token);
    let scope = fence.scope.to_string();

    // 1. A client call is awaited by the turn that made it, so one still pending died with its
    //    owner (specs/019-client-tools R16). Deferred calls survive owners as they survive
    //    restarts.
    match storage.interrupt_expired_client_calls(agent_id, below).await {
        Ok(0) => {}
        Ok(swept) => tracing::info!(
            agent_id = %agent_id,
            "marked {swept} client tool call(s) left pending by a previous owner as interrupted"
        ),
        Err(e) => failed("client calls", &scope, &e, &mut recovered),
    }

    // 2. A dialog cannot outlive the process awaiting it (specs/042 research R9).
    match storage.abandon_open_sensitive_asks(agent_id, below).await {
        Ok(0) => {}
        Ok(count) => tracing::info!(
            agent_id = %agent_id,
            "abandoned {count} sensitive ask(s) left open by a previous owner"
        ),
        Err(e) => failed("sensitive asks", &scope, &e, &mut recovered),
    }

    // 3. A session with a turn waiting to resume keeps its scratch files and its thinking mark;
    //    everything else of this agent is a leftover (specs/020 research R5).
    match storage.sessions_with_pending_turns_of(agent_id).await {
        Ok(keep) => {
            match storage.reset_thinking_except(agent_id, &keep).await {
                Ok(0) => {}
                Ok(reset) => tracing::info!(
                    agent_id = %agent_id,
                    "cleared the thinking mark of {reset} session(s) left by a previous owner"
                ),
                Err(e) => failed("thinking marks", &scope, &e, &mut recovered),
            }
            match storage.clear_scratch_except(agent_id, &keep).await {
                Ok(0) => {}
                Ok(cleared) => tracing::info!(
                    agent_id = %agent_id,
                    "cleared {cleared} scratch file(s) left by a previous owner"
                ),
                Err(e) => failed("scratch files", &scope, &e, &mut recovered),
            }
        }
        Err(e) => failed("pending sessions", &scope, &e, &mut recovered),
    }

    // 5. The agent's turns, in the order they were accepted. They are not re-owned here: the owner
    //    takes each on receipt (research R6), which also drops a duplicate delivery.
    match storage.expired_turns(agent_id, below).await {
        Ok(pending) => {
            if !pending.is_empty() {
                tracing::info!(
                    agent_id = %agent_id,
                    count = pending.len(),
                    "re-sending turns that were pending when their owner stopped"
                );
            }
            for turn in pending {
                let envelope = crate::transport::VizierRequestEnvelope {
                    response_tx: deps.transport.broadcast_sender(turn.session.clone()).await.ok(),
                    session: turn.session,
                    request: turn.request,
                    turn_id: Some(turn.id),
                    resumed: turn.state == TurnState::Running,
                };
                if let Err(e) = deps.transport.resend(envelope).await {
                    tracing::warn!(agent_id = %agent_id, "could not re-send a pending turn: {e}");
                    recovered.complete = false;
                }
            }
        }
        Err(e) => failed("pending turns", &scope, &e, &mut recovered),
    }

    // 6. Detached scripts (spec 058): one left running by the previous owner is interrupted, and
    //    every conversation with something to deliver is poked once. After the turns above, so a
    //    poke queues behind a resumed turn and a round boundary of it may take the rows first.
    deps.detached.recover_agent(&agent_id.to_string(), below).await;

    recovered
}

/// After `claim(Scope::Control, ..)` and before the scheduler's first tick.
///
/// A task run whose session has a pending turn is resumed with it and closed by
/// `VizierScheduler::recover`; only the rest died with their owner. A run left `running` would
/// display as perpetually running and, since the row *is* the overlap lock, block its task for
/// good.
pub async fn recover_control(deps: &VizierDependencies, fence: &Fence) -> Recovered {
    let mut recovered = Recovered { complete: true };
    let scope = fence.scope.to_string();
    let below = fence.token;

    let expired = match deps.storage.expired_task_runs(below).await {
        Ok(expired) => expired,
        Err(e) => {
            failed("task runs", &scope, &e, &mut recovered);
            return recovered;
        }
    };
    if expired.is_empty() {
        return recovered;
    }
    let pending = deps
        .storage
        .sessions_with_pending_turns()
        .await
        .unwrap_or_default();
    let keep: Vec<i64> = expired
        .iter()
        .filter(|run| pending.contains(&crate::storage::task_run::task_run_session(run)))
        .map(|run| run.id)
        .collect();
    match deps
        .storage
        .interrupt_expired_task_runs_except(below, &keep)
        .await
    {
        Ok(swept) => {
            if !swept.is_empty() {
                tracing::info!(
                    "marked {} task run(s) left open by a previous owner as interrupted",
                    swept.len()
                );
            }
            // A task's title is looked up; a deleted or unreadable task falls back to its slug.
            let now = crate::time::now();
            for run in swept {
                let title = match deps
                    .storage
                    .get_task(run.agent_id.clone(), run.task_slug.clone())
                    .await
                {
                    Ok(Some(task)) => task.title,
                    _ => run.task_slug.clone(),
                };
                deps.notifications
                    .task_run_ended(
                        &run.agent_id,
                        &run.task_slug,
                        &title,
                        run.id,
                        crate::schema::TaskRunState::Interrupted,
                        now,
                    )
                    .await;
            }
        }
        Err(e) => failed("task runs", &scope, &e, &mut recovered),
    }
    recovered
}
