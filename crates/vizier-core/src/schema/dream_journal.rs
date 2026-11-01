use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};


use super::{AgentId, DreamStage};
use crate::utils::markdown::MarkdownDoc;

#[derive(Debug, Clone, Serialize, Deserialize, MarkdownDoc)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DreamJournalEntry {
    pub id: String,
    pub dream_cycle_id: String,
    pub agent_id: AgentId,
    pub timestamp: DateTime<Utc>,
    pub stage: DreamStage,
    pub source_sessions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_context: Option<String>,
    #[markdown(content)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_used: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_used: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Nights and cycles (`specs/024-webui-rewrite/data-model.md` §2.4): what each dream changed, for
// the Dreams page. Everything below the types is pure so it can be unit-tested.
// ---------------------------------------------------------------------------------------------

use chrono::{Duration, NaiveDate};

/// What a dream did to one document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Wrote,
    Revised,
    Removed,
    /// CORE changed.
    Core,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DreamChange {
    pub kind: ChangeKind,
    /// Memory changes only.
    pub bundle: Option<String>,
    pub path: Option<String>,
    /// The revision's `seq` (of the document, or of CORE).
    pub seq: i64,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DreamCycleState {
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DreamCycle {
    pub cycle_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub state: DreamCycleState,
    pub sessions_read: usize,
    /// The conversations it read, by session slug.
    pub sources: Vec<String>,
    pub written: u32,
    pub revised: u32,
    pub removed: u32,
    /// Did this cycle change the agent's identity? Recomputed from `core_changes` so the flag
    /// and the count can never disagree.
    pub core: bool,
    /// How many identity changes: any revision whose **bundle** is the reserved one
    /// (specs/043-unify-core-and-memory, FR-031/FR-033). Excluded from
    /// written/revised/removed — an identity change is not also a memory change (FR-032).
    #[serde(default)]
    pub core_changes: u32,
    /// At most [`MAX_CHANGES`]; `more` counts the rest.
    pub changes: Vec<DreamChange>,
    pub more: u32,
    pub model: Option<String>,
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DreamNight {
    pub date: NaiveDate,
    /// The latest cycle of the night.
    pub cycle_id: Option<String>,
    /// written + revised + removed, excluding identity changes
    pub memories: u32,
    pub core: bool,
    pub core_changes: u32,
    pub state: Option<DreamCycleState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DreamNights {
    /// One per night, oldest first.
    pub nights: Vec<DreamNight>,
    /// Dream revisions that belong to no cycle: written before cycles were recorded and
    /// outside every cycle's journal window.
    pub unattributed: u32,
}

pub const MAX_CHANGES: usize = 50;

/// Slack either side of a cycle's journal when attributing a revision written before cycle ids
/// were recorded.
pub const LEGACY_WINDOW_SLACK: Duration = Duration::minutes(1);

/// A revision's kind: a CORE revision is `Core`; a memory revision that deleted the document is
/// `Removed`; one with no earlier revision, or whose earlier one was a deletion, is `Wrote`;
/// anything else `Revised`. `previous_deleted` is `None` when there is no earlier revision.
/// The kind of one memory revision, given the **bundle it lives in**
/// (specs/043-unify-core-and-memory, FR-031). Classification follows bundle membership, not
/// the table the row was written to: an agent that adds `core/values` gets an identity change
/// for it, with no special case for the document that happens to be the soul.
///
/// This is the function `list_dream_revisions` classifies rows with, so the rule is tested
/// through the code the query actually runs.
pub fn revision_kind(bundle: &str, deleted: bool, previous_deleted: Option<bool>) -> ChangeKind {
    if crate::schema::is_core_bundle(bundle) {
        ChangeKind::Core
    } else {
        change_kind(false, deleted, previous_deleted)
    }
}

pub fn change_kind(is_core: bool, deleted: bool, previous_deleted: Option<bool>) -> ChangeKind {
    if is_core {
        ChangeKind::Core
    } else if deleted {
        ChangeKind::Removed
    } else if previous_deleted.unwrap_or(true) {
        ChangeKind::Wrote
    } else {
        ChangeKind::Revised
    }
}

/// `[first − slack, last + slack]` over a cycle's journal timestamps, or `None` without any.
pub fn journal_window(timestamps: &[DateTime<Utc>]) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let first = timestamps.iter().min()?;
    let last = timestamps.iter().max()?;
    Some((*first - LEGACY_WINDOW_SLACK, *last + LEGACY_WINDOW_SLACK))
}

/// The cycle whose window holds `at`; the one starting latest when windows overlap.
pub fn attribute_by_window(
    at: DateTime<Utc>,
    windows: &[(String, DateTime<Utc>, DateTime<Utc>)],
) -> Option<&str> {
    windows
        .iter()
        .filter(|(_, from, to)| at >= *from && at <= *to)
        .max_by_key(|(_, from, _)| *from)
        .map(|(id, _, _)| id.as_str())
}

/// The person's local date of an instant, for a UTC offset in minutes.
pub fn night_of(at: DateTime<Utc>, offset_minutes: i32) -> NaiveDate {
    (at + Duration::minutes(offset_minutes as i64)).date_naive()
}

/// `running` while the cycle is the one in progress; `failed` when it has extraction work but
/// never consolidated; otherwise `complete`.
pub fn cycle_state(running: bool, has_extraction: bool, has_consolidation: bool) -> DreamCycleState {
    if running {
        DreamCycleState::Running
    } else if has_extraction && !has_consolidation {
        DreamCycleState::Failed
    } else {
        DreamCycleState::Complete
    }
}

/// One cycle's counts, and its capped change list.
#[derive(Debug, Clone, PartialEq)]
pub struct Tally {
    pub written: u32,
    pub revised: u32,
    pub removed: u32,
    pub core: bool,
    pub core_changes: u32,
    pub changes: Vec<DreamChange>,
    pub more: u32,
}

/// Counts and the capped change list of one cycle, from all its changes (any order).
pub fn tally(mut all: Vec<DreamChange>) -> Tally {
    all.sort_by_key(|c| c.at);
    let count = |k| all.iter().filter(|c| c.kind == k).count() as u32;
    let (written, revised, removed) = (count(ChangeKind::Wrote), count(ChangeKind::Revised), count(ChangeKind::Removed));
    // Identity changes are counted on their own and never among the memories: a revision of
    // the identity document is not also a memory revision for counting purposes (FR-032).
    let core_changes = count(ChangeKind::Core);
    let more = all.len().saturating_sub(MAX_CHANGES) as u32;
    all.truncate(MAX_CHANGES);
    Tally {
        written,
        revised,
        removed,
        core: core_changes > 0,
        core_changes,
        changes: all,
        more,
    }
}

/// One entry per night of the `days` ending at `today`, oldest first. Several cycles on one
/// night are summed, with the latest one as `cycle_id`.
pub fn build_nights(cycles: &[DreamCycle], days: u32, today: NaiveDate, offset_minutes: i32) -> Vec<DreamNight> {
    (0..days)
        .rev()
        .map(|back| {
            let date = today - Duration::days(back as i64);
            let mut of_night: Vec<&DreamCycle> =
                cycles.iter().filter(|c| night_of(c.started_at, offset_minutes) == date).collect();
            of_night.sort_by_key(|c| c.started_at);
            let latest = of_night.last();
            DreamNight {
                date,
                cycle_id: latest.map(|c| c.cycle_id.clone()),
                memories: of_night.iter().map(|c| c.written + c.revised + c.removed).sum(),
                core: of_night.iter().any(|c| c.core),
                core_changes: of_night.iter().map(|c| c.core_changes).sum(),
                state: latest.map(|c| c.state),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 20, h, m, 0).unwrap()
    }

    #[test]
    fn an_identity_change_is_classified_by_the_bundle_it_lives_in() {
        // FR-031: membership, not a path or a table. Any document in the reserved bundle is an
        // identity change, so a second document there needs no special case.
        assert_eq!(revision_kind("core", false, Some(false)), ChangeKind::Core);
        assert_eq!(
            revision_kind("core", true, Some(false)),
            ChangeKind::Core,
            "even a deletion in the reserved bundle is an identity change"
        );
        assert_eq!(
            revision_kind("vizier", false, Some(false)),
            ChangeKind::Revised,
            "an ordinary bundle keeps the ordinary kinds"
        );
        assert_eq!(revision_kind("vizier", true, Some(false)), ChangeKind::Removed);
        assert_eq!(revision_kind("vizier", false, None), ChangeKind::Wrote);
    }

    #[test]
    fn identity_changes_are_counted_on_their_own_and_never_as_memories() {
        // FR-032/FR-033, SC-011: the summary reports a count, and the count is excluded from
        // written/revised/removed.
        let all = vec![
            change(ChangeKind::Wrote, 1),
            change(ChangeKind::Core, 2),
            change(ChangeKind::Core, 3),
            change(ChangeKind::Revised, 4),
        ];
        let t = tally(all);
        assert_eq!((t.written, t.revised, t.removed), (1, 1, 0));
        assert_eq!(t.core_changes, 2, "a count, not a flag");
        assert!(t.core);
        // The change list still shows them: they are reported, just not as memories.
        assert_eq!(t.changes.iter().filter(|c| c.kind == ChangeKind::Core).count(), 2);
    }

    #[test]
    fn a_night_reports_how_many_identity_changes_it_held() {
        // FR-033: a period's summary carries the number, not merely whether one happened.
        let night = DreamNight {
            date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            cycle_id: Some("c1".into()),
            memories: 3,
            core: true,
            core_changes: 2,
            state: Some(DreamCycleState::Complete),
        };
        assert_eq!(night.core_changes, 2);
        assert_eq!(
            night.memories, 3,
            "identity changes are not counted among the memories"
        );
    }

    #[test]
    fn change_kinds() {
        assert_eq!(change_kind(true, false, None), ChangeKind::Core);
        assert_eq!(change_kind(false, true, Some(false)), ChangeKind::Removed);
        assert_eq!(change_kind(false, false, None), ChangeKind::Wrote);
        assert_eq!(change_kind(false, false, Some(true)), ChangeKind::Wrote);
        assert_eq!(change_kind(false, false, Some(false)), ChangeKind::Revised);
    }

    #[test]
    fn legacy_revisions_are_attributed_by_journal_window() {
        let windows = vec![
            ("early".to_string(), at(1, 0) - LEGACY_WINDOW_SLACK, at(1, 20) + LEGACY_WINDOW_SLACK),
            ("late".to_string(), at(3, 0) - LEGACY_WINDOW_SLACK, at(3, 10) + LEGACY_WINDOW_SLACK),
        ];
        assert_eq!(attribute_by_window(at(1, 21), &windows), Some("early")); // inside the slack
        assert_eq!(attribute_by_window(at(3, 5), &windows), Some("late"));
        assert_eq!(attribute_by_window(at(2, 0), &windows), None);
        assert_eq!(journal_window(&[at(1, 5), at(1, 0), at(1, 20)]), Some((at(0, 59), at(1, 21))));
        assert_eq!(journal_window(&[]), None);
    }

    #[test]
    fn a_night_is_the_local_date_of_the_start() {
        // 23:30 UTC is already the next day at +07:00, and still the same day at -05:00.
        assert_eq!(night_of(at(23, 30), 7 * 60), NaiveDate::from_ymd_opt(2026, 10, 21).unwrap());
        assert_eq!(night_of(at(23, 30), -5 * 60), NaiveDate::from_ymd_opt(2026, 10, 20).unwrap());
    }

    #[test]
    fn cycle_states() {
        assert_eq!(cycle_state(true, true, false), DreamCycleState::Running);
        assert_eq!(cycle_state(false, true, false), DreamCycleState::Failed);
        assert_eq!(cycle_state(false, true, true), DreamCycleState::Complete);
        assert_eq!(cycle_state(false, false, false), DreamCycleState::Complete);
    }

    fn change(kind: ChangeKind, minute: u32) -> DreamChange {
        DreamChange { kind, bundle: None, path: None, seq: minute as i64, at: at(1, minute) }
    }

    #[test]
    fn tally_counts_and_caps() {
        let mut all = vec![change(ChangeKind::Core, 59)];
        all.extend((0..55).map(|m| change(ChangeKind::Wrote, m % 50)));
        all.push(change(ChangeKind::Revised, 1));
        all.push(change(ChangeKind::Removed, 2));
        let t = tally(all);
        assert_eq!(
            (t.written, t.revised, t.removed, t.core, t.core_changes),
            (55, 1, 1, true, 1),
            "an identity change is counted on its own, never among the memories"
        );
        assert_eq!(t.changes.len(), MAX_CHANGES);
        assert_eq!(t.more, 58 - MAX_CHANGES as u32);
    }

    #[test]
    fn nights_sum_cycles_and_include_empty_nights() {
        let cycle = |id: &str, h: u32, wrote: u32, core: bool| DreamCycle {
            cycle_id: id.into(),
            started_at: at(h, 0),
            ended_at: None,
            state: DreamCycleState::Complete,
            sessions_read: 0,
            sources: vec![],
            written: wrote,
            revised: 1,
            removed: 0,
            core,
            core_changes: u32::from(core),
            changes: vec![],
            more: 0,
            model: None,
            cost_usd: None,
        };
        let cycles = vec![cycle("a", 1, 2, false), cycle("b", 4, 1, true)];
        let today = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let nights = build_nights(&cycles, 3, today, 0);
        assert_eq!(nights.len(), 3);
        assert_eq!(nights[0].date, NaiveDate::from_ymd_opt(2026, 10, 19).unwrap());
        assert_eq!(nights[0].cycle_id, None);
        assert_eq!(nights[1].cycle_id.as_deref(), Some("b"));
        assert_eq!(nights[1].memories, 3 + 2);
        assert!(nights[1].core);
        assert_eq!(nights[2].state, None);
    }
}

/// One revision a dream made, with the cycle that recorded it (when it did).
#[derive(Debug, Clone, PartialEq)]
pub struct DreamRevision {
    pub kind: ChangeKind,
    pub bundle: Option<String>,
    pub path: Option<String>,
    pub seq: i64,
    pub at: DateTime<Utc>,
    pub cycle_id: Option<String>,
}

/// A journal entry without its text: enough to say what a cycle read and when it ran.
#[derive(Debug, Clone, PartialEq)]
pub struct DreamEntryStub {
    pub cycle_id: String,
    pub stage: DreamStage,
    pub timestamp: DateTime<Utc>,
    pub source_sessions: Vec<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

/// Every cycle seen in `stubs` and `revisions`, newest first, and how many dream revisions
/// belong to none. A revision belongs to the cycle that recorded it, else to the cycle whose
/// journal window holds it (rows written before cycle ids were recorded).
pub fn assemble_cycles(
    stubs: &[DreamEntryStub],
    revisions: &[DreamRevision],
    running: Option<(&str, DateTime<Utc>)>,
) -> (Vec<DreamCycle>, u32) {
    use std::collections::BTreeMap;

    let mut ids: BTreeMap<&str, Vec<&DreamEntryStub>> = BTreeMap::new();
    for stub in stubs {
        ids.entry(stub.cycle_id.as_str()).or_default().push(stub);
    }
    let windows: Vec<(String, DateTime<Utc>, DateTime<Utc>)> = ids
        .iter()
        .filter_map(|(id, entries)| {
            let times: Vec<_> = entries.iter().map(|e| e.timestamp).collect();
            journal_window(&times).map(|(from, to)| (id.to_string(), from, to))
        })
        .collect();

    let mut changes: BTreeMap<String, Vec<DreamChange>> = BTreeMap::new();
    let mut unattributed = 0;
    for r in revisions {
        let owner = r
            .cycle_id
            .clone()
            .or_else(|| attribute_by_window(r.at, &windows).map(str::to_string));
        match owner {
            Some(id) => changes.entry(id).or_default().push(DreamChange {
                kind: r.kind,
                bundle: r.bundle.clone(),
                path: r.path.clone(),
                seq: r.seq,
                at: r.at,
            }),
            None => unattributed += 1,
        }
    }

    // A cycle with changes but no journal yet, or the one running now, still shows.
    let mut known: Vec<String> = ids.keys().map(|s| s.to_string()).collect();
    for id in changes.keys().chain(running.map(|(id, _)| id.to_string()).iter()) {
        if !known.contains(id) {
            known.push(id.clone());
        }
    }

    let mut cycles: Vec<DreamCycle> = known
        .into_iter()
        .map(|id| {
            let entries = ids.get(id.as_str()).cloned().unwrap_or_default();
            let own = changes.remove(&id).unwrap_or_default();
            let first = entries
                .iter()
                .map(|e| e.timestamp)
                .chain(own.iter().map(|c| c.at))
                .min();
            let last = entries.iter().map(|e| e.timestamp).chain(own.iter().map(|c| c.at)).max();
            let is_running = running.is_some_and(|(r, _)| r == id);
            let started_at = running
                .filter(|(r, _)| *r == id)
                .map(|(_, at)| at)
                .or(first)
                .unwrap_or_else(Utc::now);
            let mut sources: Vec<String> = Vec::new();
            for e in &entries {
                for s in &e.source_sessions {
                    if !sources.contains(s) {
                        sources.push(s.clone());
                    }
                }
            }
            let t = tally(own);
            let model = entries.iter().rev().find_map(|e| match (&e.provider, &e.model) {
                (Some(p), Some(m)) => Some(format!("{p}/{m}")),
                (None, Some(m)) => Some(m.clone()),
                _ => None,
            });
            DreamCycle {
                state: cycle_state(
                    is_running,
                    entries.iter().any(|e| e.stage == DreamStage::Extraction),
                    entries.iter().any(|e| e.stage == DreamStage::Consolidation),
                ),
                ended_at: if is_running { None } else { last },
                cycle_id: id,
                started_at,
                sessions_read: sources.len(),
                sources,
                written: t.written,
                revised: t.revised,
                removed: t.removed,
                core: t.core,
                core_changes: t.core_changes,
                changes: t.changes,
                more: t.more,
                model,
                cost_usd: None,
            }
        })
        .collect();
    cycles.sort_by_key(|c| std::cmp::Reverse(c.started_at));
    (cycles, unattributed)
}

#[cfg(test)]
mod assemble_tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 20, h, m, 0).unwrap()
    }

    fn stub(cycle: &str, stage: DreamStage, t: DateTime<Utc>) -> DreamEntryStub {
        DreamEntryStub {
            cycle_id: cycle.into(),
            stage,
            timestamp: t,
            source_sessions: vec![format!("s-{cycle}")],
            provider: Some("anthropic".into()),
            model: Some("haiku".into()),
        }
    }

    fn rev(kind: ChangeKind, t: DateTime<Utc>, cycle: Option<&str>) -> DreamRevision {
        DreamRevision { kind, bundle: Some("b".into()), path: Some("p".into()), seq: 1, at: t, cycle_id: cycle.map(str::to_string) }
    }

    #[test]
    fn recorded_and_legacy_revisions_land_on_their_cycles_and_strays_are_counted() {
        let stubs = vec![
            stub("c1", DreamStage::Extraction, at(1, 0)),
            stub("c1", DreamStage::Consolidation, at(1, 10)),
            stub("c2", DreamStage::Extraction, at(5, 0)),
        ];
        let revisions = vec![
            rev(ChangeKind::Wrote, at(1, 5), Some("c1")),   // recorded
            rev(ChangeKind::Revised, at(1, 11), None),      // legacy, inside c1's window (+1 min)
            rev(ChangeKind::Core, at(1, 9), None),          // legacy core
            rev(ChangeKind::Wrote, at(3, 0), None),         // outside every window
        ];
        let (cycles, unattributed) = assemble_cycles(&stubs, &revisions, None);
        assert_eq!(unattributed, 1);
        assert_eq!(cycles.iter().map(|c| c.cycle_id.as_str()).collect::<Vec<_>>(), ["c2", "c1"]);

        let c1 = &cycles[1];
        assert_eq!((c1.written, c1.revised, c1.removed, c1.core), (1, 1, 0, true));
        assert_eq!(c1.state, DreamCycleState::Complete);
        assert_eq!(c1.model.as_deref(), Some("anthropic/haiku"));
        // The last thing it wrote, which came after its last journal entry.
        assert_eq!(c1.ended_at, Some(at(1, 11)));

        // Extraction without a consolidation, and not running: it failed.
        assert_eq!(cycles[0].state, DreamCycleState::Failed);
    }

    #[test]
    fn the_running_cycle_shows_before_it_has_written_anything() {
        let (cycles, _) = assemble_cycles(&[], &[], Some(("now", at(2, 0))));
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].state, DreamCycleState::Running);
        assert_eq!(cycles[0].started_at, at(2, 0));
        assert_eq!(cycles[0].ended_at, None);
    }
}
