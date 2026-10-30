//! The dream cycle: trigger, status, journal, nights and cycles. Every operation needs
//! permission to edit the agent.

use chrono::{DateTime, Utc};
use serde::Serialize;

use super::{ServiceError, Services};
use crate::{
    access::can_edit_agent,
    auth::Principal,
    schema::{
        AgentConfig, DreamStatus,
        dream_journal::{self, DreamCycle, DreamJournalEntry, DreamNights},
    },
    storage::{agent::AgentStorage, dream::DreamStorage, dream_journal::DreamJournalStorage},
    transport::DreamCommand,
};

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DreamStatusResponse {
    pub status: String,
    pub total_sessions: Option<usize>,
    pub completed_sessions: Option<usize>,
    pub last_dream: Option<DateTime<Utc>>,
    pub next_dream: Option<String>,
    pub dream_provider: Option<String>,
    pub dream_model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DreamCycles {
    pub cycles: Vec<DreamCycle>,
    /// Pass as `before` for the next page; `null` at the end.
    pub next: Option<DateTime<Utc>>,
}

fn internal(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Internal(e.to_string())
}

/// The agent, when it exists and the caller may edit it.
async fn editable_agent(s: &Services, who: &Principal, agent_id: &str) -> Result<AgentConfig, ServiceError> {
    match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) if can_edit_agent(who, &config) => Ok(config),
        Ok(Some(_)) => Err(ServiceError::Forbidden("Access denied".into())),
        Ok(None) => Err(ServiceError::NotFound(format!("agent {agent_id} not found"))),
        Err(e) => Err(internal(e)),
    }
}

pub async fn trigger(s: &Services, who: &Principal, agent_id: &str) -> Result<String, ServiceError> {
    let config = editable_agent(s, who, agent_id).await?;

    if !config.dream_enabled {
        return Err(ServiceError::invalid("Dreaming is not enabled for this agent"));
    }

    s.deps()
        .transport
        .send_dream_command(DreamCommand {
            agent_id: agent_id.to_string(),
            cycle_id: None,
        })
        .await
        .map_err(internal)?;
    Ok("Dream cycle started".to_string())
}

pub async fn status(s: &Services, who: &Principal, agent_id: &str) -> Result<DreamStatusResponse, ServiceError> {
    let config = editable_agent(s, who, agent_id).await?;

    let status = s
        .deps()
        .storage
        .get_dream_status(agent_id)
        .await
        .unwrap_or(None)
        .unwrap_or(DreamStatus::Idle);

    let last_dream = s.deps().storage.get_last_dream_time(agent_id).await.unwrap_or(None);

    // Compute next scheduled dream from cron
    let next_dream = if let Some(ref cron_str) = config.dream_schedule {
        use croner::Cron;
        use std::str::FromStr;
        if let Ok(cron) = Cron::from_str(cron_str) {
            let from = last_dream.unwrap_or(Utc::now());
            cron.find_next_occurrence(&from, true).ok().map(|dt| dt.to_rfc3339())
        } else {
            None
        }
    } else {
        None
    };

    let (status_str, total_sessions, completed_sessions) = match &status {
        DreamStatus::Idle => ("idle".to_string(), None, None),
        DreamStatus::Extracting {
            total_sessions,
            completed_sessions,
            ..
        } => (
            "extracting".to_string(),
            Some(*total_sessions),
            Some(*completed_sessions),
        ),
        DreamStatus::Consolidating { .. } => ("consolidating".to_string(), None, None),
    };

    Ok(DreamStatusResponse {
        status: status_str,
        total_sessions,
        completed_sessions,
        last_dream,
        next_dream,
        dream_provider: config.dream_provider.map(|p| p.to_string()),
        dream_model: config.dream_model,
    })
}

pub async fn journal(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<DreamJournalEntry>, ServiceError> {
    editable_agent(s, who, agent_id).await?;

    s.deps()
        .storage
        .list_dream_entries(agent_id.to_string(), limit, offset)
        .await
        .map_err(internal)
}

pub async fn entry(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    entry_id: &str,
) -> Result<DreamJournalEntry, ServiceError> {
    editable_agent(s, who, agent_id).await?;

    match s
        .deps()
        .storage
        .get_dream_entry(agent_id.to_string(), entry_id.to_string())
        .await
    {
        Ok(Some(entry)) => Ok(entry),
        Ok(None) => Err(ServiceError::NotFound(format!("entry {entry_id} not found"))),
        Err(e) => Err(internal(e)),
    }
}

/// `+07:00`, `-05:30`, `Z` or `+0700` as minutes east of UTC; anything else is UTC.
pub fn parse_offset(tz: Option<&str>) -> i32 {
    let Some(tz) = tz.map(str::trim).filter(|t| !t.is_empty() && *t != "Z") else {
        return 0;
    };
    let sign = if tz.starts_with('-') { -1 } else { 1 };
    let digits: String = tz.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 2 {
        return 0;
    }
    let hours: i32 = digits[..2].parse().unwrap_or(0);
    let minutes: i32 = digits.get(2..4).and_then(|m| m.parse().ok()).unwrap_or(0);
    if hours > 14 || minutes > 59 {
        return 0;
    }
    sign * (hours * 60 + minutes)
}

/// The cycle in progress, from the stored status.
async fn running_cycle(s: &Services, agent_id: &str) -> Option<(String, DateTime<Utc>)> {
    match s.deps().storage.get_dream_status(agent_id).await.ok().flatten()? {
        DreamStatus::Extracting { cycle_id, started_at, .. } | DreamStatus::Consolidating { cycle_id, started_at } => {
            Some((cycle_id, started_at))
        }
        DreamStatus::Idle => None,
    }
}

/// What each cycle in `cycles` cost, from the usage rows its conversations wrote.
async fn price_cycles(s: &Services, agent_id: &str, cycles: &mut [DreamCycle]) {
    use crate::{
        schema::usage::UsageActivity,
        storage::usage::{UsageGrouping, UsageQuery, UsageStorage},
        usage::Pricer,
    };
    let Some(from) = cycles.iter().map(|c| c.started_at).min() else {
        return;
    };
    let query = UsageQuery {
        agent_id: agent_id.to_string(),
        start: from - chrono::Duration::hours(1),
        end: Utc::now() + chrono::Duration::minutes(1),
        activity: Some(UsageActivity::Dreams),
        conversation: None,
    };
    let Ok(groups) = s.deps().storage.usage_groups(&query, UsageGrouping::Conversation).await else {
        return;
    };
    let mut pricer = Pricer::new(&s.deps().catalogue);
    for cycle in cycles {
        let mut total: Option<f64> = None;
        for g in groups.iter().filter(|g| g.topic.as_deref() == Some(cycle.cycle_id.as_str())) {
            if let Some(c) = pricer.cost(&g.provider, &g.model, &g.tokens) {
                *total.get_or_insert(0.0) += c;
            }
        }
        cycle.cost_usd = total;
    }
}

/// One entry per night, oldest first, with what each dream changed.
pub async fn nights(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    days: Option<u32>,
    tz: Option<&str>,
) -> Result<DreamNights, ServiceError> {
    editable_agent(s, who, agent_id).await?;
    let days = days.unwrap_or(30).clamp(1, 90);
    let offset = parse_offset(tz);
    let now = Utc::now();
    // A day of slack either side, so a night near the edge of the range is whole.
    let since = now - chrono::Duration::days(days as i64 + 1);

    let stubs = s
        .deps()
        .storage
        .list_dream_entry_stubs(agent_id.to_string(), since)
        .await
        .unwrap_or_default();
    let revisions = s
        .deps()
        .storage
        .list_dream_revisions(agent_id.to_string(), since)
        .await
        .unwrap_or_default();
    let running = running_cycle(s, agent_id).await;
    let (cycles, unattributed) =
        dream_journal::assemble_cycles(&stubs, &revisions, running.as_ref().map(|(id, at)| (id.as_str(), *at)));

    Ok(DreamNights {
        nights: dream_journal::build_nights(&cycles, days, dream_journal::night_of(now, offset), offset),
        unattributed,
    })
}

/// Cycles, newest first, with their changes and cost.
pub async fn cycles(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    before: Option<DateTime<Utc>>,
    limit: Option<usize>,
) -> Result<DreamCycles, ServiceError> {
    editable_agent(s, who, agent_id).await?;
    let limit = limit.unwrap_or(20).clamp(1, 50);
    let since = DateTime::<Utc>::UNIX_EPOCH;

    let stubs = s
        .deps()
        .storage
        .list_dream_entry_stubs(agent_id.to_string(), since)
        .await
        .unwrap_or_default();
    let revisions = s
        .deps()
        .storage
        .list_dream_revisions(agent_id.to_string(), since)
        .await
        .unwrap_or_default();
    let running = running_cycle(s, agent_id).await;
    let (mut cycles, _) =
        dream_journal::assemble_cycles(&stubs, &revisions, running.as_ref().map(|(id, at)| (id.as_str(), *at)));

    if let Some(before) = before {
        cycles.retain(|c| c.started_at < before);
    }
    let next = (cycles.len() > limit).then(|| cycles[limit - 1].started_at);
    cycles.truncate(limit);
    price_cycles(s, agent_id, &mut cycles).await;

    Ok(DreamCycles { cycles, next })
}

#[cfg(test)]
mod offset_tests {
    use super::parse_offset;

    #[test]
    fn reads_the_offsets_a_browser_can_send() {
        assert_eq!(parse_offset(Some("+07:00")), 420);
        assert_eq!(parse_offset(Some("-05:30")), -330);
        assert_eq!(parse_offset(Some("+0100")), 60);
        assert_eq!(parse_offset(Some("Z")), 0);
        assert_eq!(parse_offset(None), 0);
        assert_eq!(parse_offset(Some("nonsense")), 0);
        assert_eq!(parse_offset(Some("+99:00")), 0);
    }
}
