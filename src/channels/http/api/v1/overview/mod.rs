//! Home's data (`specs/024-webui-rewrite/contracts/http-api.md` § Overview): the counts for the
//! greeting, the "Last 24 hours" dot grid and, in [`feed`], "Since you were away". Everything is
//! limited to the agents the signed-in person can see.

use axum::{
    Extension, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::{DateTime, Duration, DurationRound, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        api::v1::agents::{dream::parse_offset, user_can_view_agent},
        auth::AuthenticatedUser,
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    schema::{AgentConfig, AgentRunState},
    storage::{agent::AgentStorage, session::SessionStorage, state::StateStorage, usage::UsageStorage},
    usage::Pricer,
};

pub mod feed;

pub fn overview() -> Router<HTTPState> {
    Router::new()
        .route("/", get(overview_counts))
        .route("/activity", get(activity))
        .route("/feed", get(feed::feed))
}

/// The agents `user` can see, in id order.
pub(crate) async fn visible_agents(
    state: &HTTPState,
    user: &AuthenticatedUser,
) -> Result<Vec<(String, AgentConfig)>, String> {
    let mut agents: Vec<(String, AgentConfig)> = state
        .storage
        .list_agents()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|(_, config)| user_can_view_agent(user, config))
        .collect();
    agents.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(agents)
}

/// The start of "today" for a person `offset_minutes` east of UTC, as a UTC instant.
pub(crate) fn start_of_today(now: DateTime<Utc>, offset_minutes: i32) -> DateTime<Utc> {
    let shift = Duration::minutes(offset_minutes as i64);
    let local = now + shift;
    let midnight = local.duration_trunc(Duration::days(1)).unwrap_or(local);
    midnight - shift
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct TzQuery {
    /// The person's UTC offset, e.g. `+07:00`; default UTC.
    pub tz: Option<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AgentCounts {
    pub total: usize,
    /// With a turn running now.
    pub live: usize,
    /// Failed to start or run, whatever their retry state.
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Overview {
    pub agents: AgentCounts,
    /// Today's estimated spend across these agents, in USD. Calls to models without a catalogue
    /// price add nothing.
    pub cost_today_usd: f64,
    /// Where complete usage accounting starts; spend from before it is partial.
    pub accounting_since: Option<DateTime<Utc>>,
}

/// Ids of agents in a failed state.
async fn failed_agents(state: &HTTPState) -> std::collections::HashSet<String> {
    state
        .transport
        .agent_health()
        .await
        .map(|h| h.into_iter().filter(|s| s.state == AgentRunState::Failed).map(|s| s.agent_id).collect())
        .unwrap_or_default()
}

#[utoipa::path(
    get,
    path = "/overview",
    params(TzQuery),
    responses((status = 200, description = "Counts and today's spend across the agents the caller can see", body = APIResponse<Overview>))
)]
pub async fn overview_counts(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(query): Query<TzQuery>,
) -> models::response::Response<Overview> {
    let agents = match visible_agents(&state, &user).await {
        Ok(a) => a,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let ids: Vec<String> = agents.iter().map(|(id, _)| id.clone()).collect();
    let live = state.storage.thinking_agents().await.unwrap_or_default();
    let failed = failed_agents(&state).await;

    let since = start_of_today(Utc::now(), parse_offset(query.tz.as_deref()));
    let groups = state.storage.usage_groups_since(&ids, since).await.unwrap_or_default();
    let mut pricer = Pricer::new(&state.catalogue);
    let cost_today_usd: f64 = groups.iter().filter_map(|g| pricer.cost(&g.provider, &g.model, &g.tokens)).sum::<f64>().max(0.0);

    let accounting_since = state
        .storage
        .get_state(crate::dependencies::VizierDependencies::USAGE_ACCOUNTING_SINCE.to_string())
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(str::to_string))
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Utc));

    api_response(
        StatusCode::OK,
        Overview {
            agents: AgentCounts {
                total: ids.len(),
                live: ids.iter().filter(|id| live.contains(*id)).count(),
                failed: ids.iter().filter(|id| failed.contains(*id)).count(),
            },
            cost_today_usd,
            accounting_since,
        },
    )
}

// ---------------------------------------------------------------------------------------------
// Last 24 hours
// ---------------------------------------------------------------------------------------------

/// How agents are ordered when there are more rows than the grid shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, utoipa::ToSchema)]
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

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ActivityQuery {
    pub rank: Option<Rank>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ActivityRow {
    /// `null` for the "All N" row.
    pub agent_id: Option<String>,
    pub label: String,
    /// Turns started in each of the 24 hours of [`Activity::hours`].
    pub turns: Vec<u64>,
    pub live: bool,
}

#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct TodayByKind {
    pub chat: u64,
    pub scheduled: u64,
    pub dream_stages: u64,
    pub background: u64,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Activity {
    /// The start of each of the last 24 hours, oldest first, ending with the current hour.
    pub hours: Vec<DateTime<Utc>>,
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

#[utoipa::path(
    get,
    path = "/overview/activity",
    params(ActivityQuery, TzQuery),
    responses((status = 200, description = "Turns per hour for the last 24 hours, for the agents the caller can see", body = APIResponse<Activity>))
)]
pub async fn activity(
    State(state): State<HTTPState>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(query): Query<ActivityQuery>,
    Query(tz): Query<TzQuery>,
) -> models::response::Response<Activity> {
    let agents = match visible_agents(&state, &user).await {
        Ok(a) => a,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let ids: Vec<String> = agents.iter().map(|(id, _)| id.clone()).collect();

    let now = Utc::now();
    let this_hour = now.duration_trunc(Duration::hours(1)).unwrap_or(now);
    let hours: Vec<DateTime<Utc>> = (0..24).rev().map(|back| this_hour - Duration::hours(back)).collect();
    let since = hours[0];

    let counts = state.storage.hourly_turns(&ids, since).await.unwrap_or_default();
    let live = state.storage.thinking_agents().await.unwrap_or_default();
    let failed = failed_agents(&state).await;

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
    let (rows, all) = arrange(ranked, query.rank.unwrap_or_default());

    let today_by_kind = if agent_total == 1 {
        let start = start_of_today(now, parse_offset(tz.tz.as_deref()));
        let mut kinds = TodayByKind::default();
        for (activity, n) in state.storage.turns_by_activity(&ids, start).await.unwrap_or_default() {
            use crate::schema::usage::UsageActivity::*;
            match activity {
                Chat => kinds.chat += n,
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

    api_response(StatusCode::OK, Activity { hours, rows, all, agent_total, today_by_kind })
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
    fn today_starts_at_the_persons_midnight() {
        let now = DateTime::parse_from_rfc3339("2026-10-23T20:30:00Z").unwrap().with_timezone(&Utc);
        // +07:00: it is already 03:30 on the 24th there, so today began at 17:00Z on the 23rd.
        assert_eq!(start_of_today(now, 420).to_rfc3339(), "2026-10-23T17:00:00+00:00");
        assert_eq!(start_of_today(now, 0).to_rfc3339(), "2026-10-23T00:00:00+00:00");
        assert_eq!(start_of_today(now, -300).to_rfc3339(), "2026-10-23T05:00:00+00:00");
    }
}
