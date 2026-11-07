//! Home's data: the counts for the greeting and the "Last 24 hours" dot grid. Everything is
//! limited to the agents the person can see (`specs/024-webui-rewrite/contracts/http-api.md`).

use chrono::{DateTime, Duration, DurationRound, Local};
use serde::{Deserialize, Serialize};

use crate::{
    access::can_view_agent,
    auth::Principal,
    schema::{AgentConfig, AgentRunState},
    service::{ServiceError, Services},
    storage::{
        agent::AgentStorage, session::SessionStorage, state::StateStorage, usage::UsageStorage,
    },
    usage::{Pricer, saved_entries},
};

/// The agents `user` can see, in id order.
pub async fn visible_agents(
    services: &Services,
    user: &Principal,
) -> Result<Vec<(String, AgentConfig)>, ServiceError> {
    let mut agents: Vec<(String, AgentConfig)> = services
        .deps()
        .storage
        .list_agents()
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?
        .into_iter()
        .filter(|(_, config)| can_view_agent(user, config))
        .collect();
    agents.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(agents)
}

/// The start of "today" on this instance: local midnight (or the first moment after it, on a day
/// whose midnight the clocks skip).
pub fn start_of_today(now: DateTime<Local>) -> DateTime<Local> {
    crate::time::start_of_local_day(crate::time::local_date_of(now))
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentCounts {
    pub total: usize,
    /// With a turn running now.
    pub live: usize,
    /// Failed to start or run, whatever their retry state.
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Overview {
    pub agents: AgentCounts,
    /// Today's estimated spend across these agents, in USD. Calls to models without a catalogue
    /// price add nothing.
    pub cost_today_usd: f64,
    /// Where complete usage accounting starts; spend from before it is partial.
    pub accounting_since: Option<DateTime<Local>>,
}

/// Ids of agents in a failed state, read from the published health views.
async fn failed_agents(services: &Services) -> std::collections::HashSet<String> {
    let deps = services.deps();
    let Ok(agents) = deps.storage.list_agents().await else {
        return Default::default();
    };
    let mut failed = std::collections::HashSet::new();
    for (id, _) in agents {
        let Some(status) = crate::agents::views::read_health(deps, &id).await else {
            continue;
        };
        if status.state == AgentRunState::Failed {
            failed.insert(id);
        }
    }
    failed
}

pub async fn counts(
    services: &Services,
    user: &Principal,
) -> Result<Overview, ServiceError> {
    let deps = services.deps();
    let agents = visible_agents(services, user).await?;
    let ids: Vec<String> = agents.iter().map(|(id, _)| id.clone()).collect();
    let live = deps.storage.thinking_agents().await.unwrap_or_default();
    let failed = failed_agents(services).await;

    let since = start_of_today(crate::time::now());
    let groups = deps.storage.usage_groups_since(&ids, since).await.unwrap_or_default();
    let mut pricer = Pricer::with_entries(&deps.catalogue, saved_entries(&deps.storage).await);
    let cost_today_usd: f64 = groups
        .iter()
        .filter_map(|g| pricer.cost(&g.provider, &g.model, &g.tokens))
        .sum::<f64>()
        .max(0.0);

    let accounting_since = deps
        .storage
        .get_state(crate::dependencies::VizierDependencies::USAGE_ACCOUNTING_SINCE.to_string())
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(str::to_string))
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Local));

    Ok(Overview {
        agents: AgentCounts {
            total: ids.len(),
            live: ids.iter().filter(|id| live.contains(*id)).count(),
            failed: ids.iter().filter(|id| failed.contains(*id)).count(),
        },
        cost_today_usd,
        accounting_since,
    })
}

// ---------------------------------------------------------------------------------------------
// Last 24 hours
// ---------------------------------------------------------------------------------------------

/// How agents are ordered when there are more rows than the grid shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Rank {
    #[default]
    MostActive,
    /// The caller's own agents first.
    Mine,
    /// Agents with a turn running first.
    Live,
    /// Failed agents first.
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ActivityRow {
    /// `null` for the "All N" row.
    pub agent_id: Option<String>,
    pub label: String,
    /// Turns started in each of the 24 hours of [`Activity::hours`].
    pub turns: Vec<u64>,
    pub live: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TodayByKind {
    pub chat: u64,
    pub scheduled: u64,
    pub dream_stages: u64,
    pub background: u64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Activity {
    /// The start of each of the last 24 hours, oldest first, ending with the current hour.
    pub hours: Vec<DateTime<Local>>,
    /// At most 6: the top 5 when `all` is present, otherwise every agent.
    pub rows: Vec<ActivityRow>,
    /// Present when there are 7 or more agents.
    pub all: Option<ActivityRow>,
    pub agent_total: usize,
    /// Present when there is exactly one agent: today's turns by kind.
    pub today_by_kind: Option<TodayByKind>,
}

/// An agent's place in a ranking.
struct Ranked {
    id: String,
    label: String,
    turns: Vec<u64>,
    mine: bool,
    live: bool,
    failed: bool,
}

/// Orders `agents` for `rank`, most notable first; ties go to the busier agent, then the id.
fn order(agents: &mut [Ranked], rank: Rank) {
    agents.sort_by(|a, b| {
        let busy = |r: &Ranked| r.turns.iter().sum::<u64>();
        let first = match rank {
            Rank::MostActive => std::cmp::Ordering::Equal,
            Rank::Mine => b.mine.cmp(&a.mine),
            Rank::Live => b.live.cmp(&a.live),
            Rank::Failed => b.failed.cmp(&a.failed),
        };
        first.then(busy(b).cmp(&busy(a))).then(a.id.cmp(&b.id))
    });
}

/// How many agents make the grid show an "All N" row and only the top few.
const ALL_ROW_FROM: usize = 7;
const TOP_WITH_ALL: usize = 5;
const MAX_ROWS: usize = 6;

/// The grid's rows and "All" row from every agent's hourly turns.
fn arrange(mut agents: Vec<Ranked>, rank: Rank) -> (Vec<ActivityRow>, Option<ActivityRow>) {
    let total = agents.len();
    let all = (total >= ALL_ROW_FROM).then(|| {
        let mut turns = vec![0u64; 24];
        for a in &agents {
            for (sum, n) in turns.iter_mut().zip(&a.turns) {
                *sum += n;
            }
        }
        ActivityRow { agent_id: None, label: format!("All {total}"), turns, live: agents.iter().any(|a| a.live) }
    });
    order(&mut agents, rank);
    let shown = if all.is_some() { TOP_WITH_ALL } else { MAX_ROWS };
    let rows = agents
        .into_iter()
        .take(shown)
        .map(|a| ActivityRow { agent_id: Some(a.id), label: a.label, turns: a.turns, live: a.live })
        .collect();
    (rows, all)
}

pub async fn activity(
    services: &Services,
    user: &Principal,
    rank: Rank,
) -> Result<Activity, ServiceError> {
    let deps = services.deps();
    let agents = visible_agents(services, user).await?;
    let ids: Vec<String> = agents.iter().map(|(id, _)| id.clone()).collect();

    let now = crate::time::now();
    // Usage is bucketed by UTC hour, so the window is too; truncating the local clock would
    // misalign it in a zone with a half-hour offset.
    let hour_ms = 3_600_000;
    let this_hour = crate::time::from_millis(now.timestamp_millis().div_euclid(hour_ms) * hour_ms);
    let hours: Vec<DateTime<Local>> = (0..24).rev().map(|back| this_hour - Duration::hours(back)).collect();
    let since = hours[0];

    let counts = deps.storage.hourly_turns(&ids, since).await.unwrap_or_default();
    let live = deps.storage.thinking_agents().await.unwrap_or_default();
    let failed = failed_agents(services).await;

    let ranked: Vec<Ranked> = agents
        .iter()
        .map(|(id, config)| {
            let mut turns = vec![0u64; 24];
            for row in counts.iter().filter(|r| &r.agent_id == id) {
                if let Some(i) = hours.iter().position(|h| *h == row.hour) {
                    turns[i] += row.turns;
                }
            }
            Ranked {
                id: id.clone(),
                label: config.name.clone(),
                turns,
                mine: config.owner_id.as_deref() == Some(user.user_id.as_str()),
                live: live.contains(id),
                failed: failed.contains(id),
            }
        })
        .collect();
    let agent_total = ranked.len();
    let (rows, all) = arrange(ranked, rank);

    let today_by_kind = if agent_total == 1 {
        let start = start_of_today(now);
        let mut kinds = TodayByKind::default();
        for (activity, n) in deps.storage.turns_by_activity(&ids, start).await.unwrap_or_default() {
            use crate::schema::usage::UsageActivity::*;
            match activity {
                // A project chat is still a person's chat turn in the today summary; the
                // usage report is where project work is broken out (FR-016).
                // A Workspace turn is a chat turn too.
                Chat | Project | Workspace => kinds.chat += n,
                Scheduled => kinds.scheduled += n,
                Dreams => kinds.dream_stages += n,
                Background => kinds.background += n,
                Compaction | Vision | Other => {}
            }
        }
        Some(kinds)
    } else {
        None
    };

    Ok(Activity { hours, rows, all, agent_total, today_by_kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(id: &str, turns: u64, mine: bool, live: bool, failed: bool) -> Ranked {
        let mut hourly = vec![0; 24];
        hourly[23] = turns;
        Ranked { id: id.into(), label: id.to_uppercase(), turns: hourly, mine, live, failed }
    }

    fn ids(rows: &[ActivityRow]) -> Vec<&str> {
        rows.iter().map(|r| r.agent_id.as_deref().unwrap()).collect()
    }

    #[test]
    fn up_to_six_agents_each_get_a_row_and_there_is_no_all_row() {
        let agents = (0..6).map(|i| agent(&format!("a{i}"), i, false, false, false)).collect();
        let (rows, all) = arrange(agents, Rank::MostActive);
        assert_eq!(rows.len(), 6);
        assert!(all.is_none());
        assert_eq!(ids(&rows), ["a5", "a4", "a3", "a2", "a1", "a0"]);
    }

    #[test]
    fn seven_or_more_show_the_top_five_and_an_all_row_that_counts_everyone() {
        let agents: Vec<Ranked> = (0..9).map(|i| agent(&format!("a{i}"), i + 1, false, i == 0, false)).collect();
        let (rows, all) = arrange(agents, Rank::MostActive);
        assert_eq!(ids(&rows), ["a8", "a7", "a6", "a5", "a4"]);
        let all = all.expect("an All row");
        assert_eq!(all.label, "All 9");
        assert_eq!(all.turns[23], (1..=9).sum::<u64>(), "the All row includes agents not listed");
        assert!(all.live);
        assert_eq!(all.agent_id, None);
    }

    #[test]
    fn rank_puts_the_asked_for_agents_first_then_the_busiest() {
        let make = || {
            vec![
                agent("busy", 9, false, false, false),
                agent("mine", 1, true, false, false),
                agent("live", 2, false, true, false),
                agent("down", 0, false, false, true),
            ]
        };
        assert_eq!(ids(&arrange(make(), Rank::Mine).0)[0], "mine");
        assert_eq!(ids(&arrange(make(), Rank::Live).0)[0], "live");
        assert_eq!(ids(&arrange(make(), Rank::Failed).0)[0], "down");
        assert_eq!(ids(&arrange(make(), Rank::MostActive).0)[0], "busy");
    }

    #[test]
    fn today_starts_at_the_instances_midnight() {
        let now = DateTime::parse_from_rfc3339("2026-10-23T20:30:00Z").unwrap().with_timezone(&Local);
        let start = start_of_today(now);
        assert_eq!(start.date_naive(), now.date_naive());
        assert_eq!(start.naive_local().time(), chrono::NaiveTime::MIN);
        assert!(start <= now && now - start < Duration::hours(25));
    }

    #[test]
    fn a_day_starts_at_local_midnight_in_any_zone() {
        use chrono_tz::{America::New_York, Asia::Jakarta};
        let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        // +07:00: the 24th began at 17:00Z on the 23rd.
        assert_eq!(
            crate::time::start_of_day_in(date, &Jakarta).to_utc().to_rfc3339(),
            "2026-10-23T17:00:00+00:00"
        );
        // The clocks go back on 1 Nov, so that day is 25 hours long and still starts at 00:00.
        let day = chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let start = crate::time::start_of_day_in(day, &New_York);
        let next = crate::time::start_of_day_in(day.succ_opt().unwrap(), &New_York);
        assert_eq!(next - start, Duration::hours(25));
    }
}
