//! `current_time`, `convert_time`, `shift_time` and `time_between` (`specs/052-unify-local-timezone`,
//! `contracts/time-tools.md`).
//!
//! The prompt carries no clock, so an agent asks these. All four are read-only. A bad zone or
//! moment is a result with a `status` and the accepted forms, never an exception, so a script and
//! the turn carry on. The arithmetic is written over `chrono::TimeZone` and tested with `chrono_tz`
//! zones; the instance's own zone is `Local` when it can't be named.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Timelike, Weekday};
use chrono_tz::Tz;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    time::{self, ScheduleTimeError, WallClock},
};

const MOMENT_TEXT: &str = "Moment text is RFC 3339 with an offset (2026-11-07T09:00:00+07:00, or ...Z), which is that exact moment, or a date and time with no offset (2026-11-07T09:00, or with a space) which is local to the zone in question.";
const FORMS: &str = "2026-11-07T09:00:00+07:00 | 2026-11-07T09:00:00 | 2026-11-07 09:00";

/// One moment, described the way the other tools describe it.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct Moment {
    pub iso: String,
    pub date: String,
    pub time: String,
    pub weekday: String,
    /// IANA name; `null` only when the instance's zone can't be named.
    pub zone: Option<String>,
    pub offset: String,
}

impl Moment {
    fn of<T: TimeZone>(at: &DateTime<T>, zone: Option<String>) -> Self
    where
        T::Offset: std::fmt::Display,
    {
        Self {
            iso: at.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            date: at.format("%Y-%m-%d").to_string(),
            time: at.format("%H:%M").to_string(),
            weekday: at.format("%A").to_string(),
            zone,
            offset: at.format("%:z").to_string(),
        }
    }
}

/// What every time tool returns. Only the fields of the outcome are present.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct TimeResult {
    /// `ok`, or `unknown_zone`, `invalid_time`, `invalid_argument` or `in_gap`.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now: Option<Moment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Moment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Moment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Moment>,
    /// Set when the result had to move because its local time does not exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Signed: positive when `to` is later than `from`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// `future`, `past` or `same`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub did_you_mean: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_valid: Option<Moment>,
}

impl TimeResult {
    fn ok() -> Self {
        Self { status: "ok".into(), ..Default::default() }
    }

    fn unknown_zone(zone: &str) -> Self {
        let known: Vec<String> = chrono_tz::TZ_VARIANTS.iter().map(|tz| tz.name().to_string()).collect();
        Self {
            status: "unknown_zone".into(),
            message: Some(format!("{zone:?} is not an IANA timezone name, such as Asia/Jakarta or America/New_York")),
            did_you_mean: Some(crate::sandbox::docs::did_you_mean(zone, &known)),
            ..Default::default()
        }
    }

    fn invalid_time(input: &str) -> Self {
        Self {
            status: "invalid_time".into(),
            message: Some(format!("{input:?} is not a time I can read")),
            expected: Some(FORMS.into()),
            ..Default::default()
        }
    }

    fn invalid_argument(message: impl Into<String>, expected: &str) -> Self {
        Self { status: "invalid_argument".into(), message: Some(message.into()), expected: Some(expected.into()), ..Default::default() }
    }
}

/// A zone to compute in: a named one, or the host's own when it has no name.
#[derive(Clone)]
enum Zone {
    Named(Tz),
    Local,
}

impl Zone {
    /// The instance's zone.
    fn instance() -> Self {
        time::zone()
            .name
            .and_then(|name| name.parse::<Tz>().ok())
            .map_or(Self::Local, Self::Named)
    }

    /// A zone by name, ignoring case; `None` text is the instance's zone.
    fn named(text: Option<&str>) -> Result<Self, TimeResult> {
        let Some(text) = text.map(str::trim).filter(|t| !t.is_empty()) else {
            return Ok(Self::instance());
        };
        let wanted = text.to_ascii_lowercase();
        chrono_tz::TZ_VARIANTS
            .iter()
            .find(|tz| tz.name().to_ascii_lowercase() == wanted)
            .map(|tz| Self::Named(*tz))
            .ok_or_else(|| TimeResult::unknown_zone(text))
    }

    fn name(&self) -> Option<String> {
        match self {
            Self::Named(tz) => Some(tz.name().to_string()),
            Self::Local => time::zone().name,
        }
    }
}

/// Runs a zone-generic function in whichever zone this is.
macro_rules! in_zone {
    ($zone:expr, |$tz:ident| $body:expr) => {
        match $zone {
            Zone::Named(named) => {
                let $tz: Tz = *named;
                $body
            }
            Zone::Local => {
                let $tz = chrono::Local;
                $body
            }
        }
    };
}

fn describe_moment<T: TimeZone>(at: &DateTime<T>, zone: &Zone) -> Moment
where
    T::Offset: std::fmt::Display,
{
    Moment::of(at, zone.name())
}

/// Reads moment text, as a refusal when it can't be.
fn read_moment<T: TimeZone>(text: &str, tz: &T) -> Result<DateTime<T>, Box<TimeResult>>
where
    T::Offset: std::fmt::Display,
{
    time::parse_moment_in(text, tz).map_err(|e| {
        Box::new(match e {
            ScheduleTimeError::Unparseable => TimeResult::invalid_time(text),
            ScheduleTimeError::InGap { first_valid } => TimeResult {
                status: "in_gap".into(),
                message: Some("that local time does not exist, because the clocks change at that moment".into()),
                first_valid: Some(Moment::of(&first_valid, None)),
                ..Default::default()
            },
        })
    })
}

fn current<T: TimeZone>(tz: &T) -> DateTime<T> {
    time::now().with_timezone(tz)
}

// ---- current_time ---------------------------------------------------------------------------

pub struct CurrentTime;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CurrentTimeArgs {
    #[schemars(description = "IANA timezone name, such as Asia/Jakarta. Defaults to the instance's own zone, which is the zone every time here is in unless stated otherwise")]
    zone: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for CurrentTime {
    type Input = CurrentTimeArgs;
    type Output = TimeResult;

    fn name() -> String {
        "current_time".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Time)
    }

    fn description(&self) -> String {
        "The current date and time, with the weekday, zone and offset. Nothing else tells you the time: always call this, or shift_time, rather than guessing a date.".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(run_current(args))
    }
}

fn run_current(args: CurrentTimeArgs) -> TimeResult {
    let zone = match Zone::named(args.zone.as_deref()) {
        Ok(zone) => zone,
        Err(refusal) => return refusal,
    };
    let now = in_zone!(&zone, |tz| describe_moment(&current(&tz), &zone));
    TimeResult { now: Some(now), ..TimeResult::ok() }
}

// ---- convert_time ---------------------------------------------------------------------------

pub struct ConvertTime;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ConvertTimeArgs {
    #[schemars(description = "The moment to convert. See the tool description for the accepted forms")]
    at: String,
    #[schemars(description = "IANA timezone name to show it in, such as Europe/London")]
    to_zone: String,
    #[schemars(description = "IANA timezone name that `at` is local to when it has no offset. Defaults to the instance's zone")]
    from_zone: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for ConvertTime {
    type Input = ConvertTimeArgs;
    type Output = TimeResult;

    fn name() -> String {
        "convert_time".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Time)
    }

    fn description(&self) -> String {
        format!("Show a moment in another timezone, to answer what time it is somewhere else. {MOMENT_TEXT}")
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(run_convert(args))
    }
}

fn run_convert(args: ConvertTimeArgs) -> TimeResult {
    let from = match Zone::named(args.from_zone.as_deref()) {
        Ok(zone) => zone,
        Err(refusal) => return refusal,
    };
    let to = match Zone::named(Some(&args.to_zone)) {
        Ok(zone) => zone,
        Err(refusal) => return refusal,
    };
    let instant = match in_zone!(&from, |tz| read_moment(&args.at, &tz).map(|at| at.with_timezone(&chrono::Utc))) {
        Ok(at) => at,
        Err(refusal) => return *refusal,
    };
    let shown_from = in_zone!(&from, |tz| describe_moment(&instant.with_timezone(&tz), &from));
    let shown_to = in_zone!(&to, |tz| describe_moment(&instant.with_timezone(&tz), &to));
    TimeResult { from: Some(shown_from), to: Some(shown_to), ..TimeResult::ok() }
}

// ---- shift_time -----------------------------------------------------------------------------

pub struct ShiftTime;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ShiftTimeArgs {
    #[schemars(description = "The moment to start from. Defaults to now")]
    at: Option<String>,
    #[schemars(description = "IANA timezone name to work in. Defaults to the instance's zone")]
    zone: Option<String>,
    #[schemars(description = "Calendar days to add, which may be negative. The wall-clock time stays the same across a daylight-saving change")]
    add_days: Option<i64>,
    #[schemars(description = "Hours to add, which may be negative. This is elapsed time, so it crosses a daylight-saving change in absolute terms")]
    add_hours: Option<i64>,
    #[schemars(description = "Minutes to add, which may be negative. Elapsed time, like add_hours")]
    add_minutes: Option<i64>,
    #[schemars(description = "Move to this weekday (monday to sunday), never staying on the same day. Use it for 'next Tuesday'")]
    to_weekday: Option<String>,
    #[schemars(description = "With to_weekday: `next` (default) or `previous`")]
    direction: Option<String>,
    #[schemars(description = "Set the wall-clock time of day, as HH:MM, e.g. 09:00")]
    at_time: Option<String>,
}

fn parse_weekday(text: &str) -> Option<Weekday> {
    text.trim().to_ascii_lowercase().parse().ok()
}

/// Moves a date by whole days and puts the wall-clock time back, noting a gap.
fn move_wall_clock<T: TimeZone>(
    at: &DateTime<T>,
    date: NaiveDate,
    time_of_day: NaiveTime,
    note: &mut Option<String>,
) -> DateTime<T> {
    let tz = at.timezone();
    match time::resolve_wall_clock(date.and_time(time_of_day), &tz) {
        WallClock::Gap { first_valid } => {
            *note = Some(format!(
                "{} {} does not exist because the clocks change then, so the result is the first valid time after it",
                date,
                time_of_day.format("%H:%M")
            ));
            first_valid
        }
        other => other.into_moment(),
    }
}

/// The days from `from` to the next or previous `target` weekday, 1 to 7.
fn weekday_delta(from: Weekday, target: Weekday, previous: bool) -> i64 {
    let (f, t) = (from.num_days_from_monday() as i64, target.num_days_from_monday() as i64);
    if previous { (f - t + 6) % 7 + 1 } else { (t - f + 6) % 7 + 1 }
}

fn shift<T: TimeZone>(tz: &T, args: &ShiftTimeArgs) -> Result<(DateTime<T>, Option<String>), Box<TimeResult>>
where
    T::Offset: std::fmt::Display,
{
    let mut at = match args.at.as_deref() {
        Some(text) => read_moment(text, tz)?,
        None => current(tz),
    };
    let mut note = None;

    if let Some(days) = args.add_days {
        let date = at.date_naive().checked_add_signed(Duration::days(days));
        let Some(date) = date else {
            return Err(Box::new(TimeResult::invalid_argument("add_days is out of range", "a smaller number of days")));
        };
        at = move_wall_clock(&at, date, at.time(), &mut note);
    }
    let elapsed = Duration::try_hours(args.add_hours.unwrap_or(0))
        .zip(Duration::try_minutes(args.add_minutes.unwrap_or(0)))
        .and_then(|(h, m)| h.checked_add(&m))
        .and_then(|d| at.clone().checked_add_signed(d));
    match elapsed {
        Some(moved) => at = moved,
        None => return Err(Box::new(TimeResult::invalid_argument("add_hours or add_minutes is out of range", "a smaller amount"))),
    }
    if let Some(weekday) = args.to_weekday.as_deref() {
        let Some(target) = parse_weekday(weekday) else {
            return Err(Box::new(TimeResult::invalid_argument(
                format!("{weekday:?} is not a weekday"),
                "monday | tuesday | wednesday | thursday | friday | saturday | sunday",
            )));
        };
        let previous = match args.direction.as_deref().map(|d| d.trim().to_ascii_lowercase()) {
            None => false,
            Some(d) if d == "next" => false,
            Some(d) if d == "previous" => true,
            Some(other) => {
                return Err(Box::new(TimeResult::invalid_argument(format!("{other:?} is not a direction"), "next | previous")));
            }
        };
        let delta = weekday_delta(at.weekday(), target, previous);
        let date = at.date_naive() + Duration::days(if previous { -delta } else { delta });
        at = move_wall_clock(&at, date, at.time(), &mut note);
    }
    if let Some(clock) = args.at_time.as_deref() {
        let Ok(parsed) = NaiveTime::parse_from_str(clock.trim(), "%H:%M") else {
            return Err(Box::new(TimeResult::invalid_time(clock)));
        };
        at = move_wall_clock(&at, at.date_naive(), parsed, &mut note);
    }
    Ok((at, note))
}

#[async_trait::async_trait]
impl VizierTool for ShiftTime {
    type Input = ShiftTimeArgs;
    type Output = TimeResult;

    fn name() -> String {
        "shift_time".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Time)
    }

    fn description(&self) -> String {
        format!("Do date arithmetic: add days, hours or minutes, move to the next or previous weekday, or set a time of day, applied in that order. Use it for 'tomorrow at 9' or 'next Tuesday' instead of working the date out yourself. The result is a moment you can pass to schedule_one_time_task. A result in a daylight-saving gap moves to the first valid time, with a note. {MOMENT_TEXT}")
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(run_shift(args))
    }
}

fn run_shift(args: ShiftTimeArgs) -> TimeResult {
    if args.add_days.is_none()
        && args.add_hours.is_none()
        && args.add_minutes.is_none()
        && args.to_weekday.is_none()
        && args.at_time.is_none()
    {
        return TimeResult::invalid_argument(
            "nothing to shift: give add_days, add_hours, add_minutes, to_weekday or at_time",
            "at least one of add_days | add_hours | add_minutes | to_weekday | at_time",
        );
    }
    let zone = match Zone::named(args.zone.as_deref()) {
        Ok(zone) => zone,
        Err(refusal) => return refusal,
    };
    let outcome = in_zone!(&zone, |tz| shift(&tz, &args).map(|(at, note)| (describe_moment(&at, &zone), note)));
    match outcome {
        Ok((result, note)) => TimeResult { result: Some(result), note, ..TimeResult::ok() },
        Err(refusal) => *refusal,
    }
}

// ---- time_between ---------------------------------------------------------------------------

pub struct TimeBetween;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct TimeBetweenArgs {
    #[schemars(description = "The first moment")]
    from: String,
    #[schemars(description = "The second moment. Defaults to now")]
    to: Option<String>,
}

fn plural(n: i64, unit: &str) -> String {
    format!("{n} {unit}{}", if n == 1 { "" } else { "s" })
}

/// `1 hour 30 minutes`, `2 days 3 hours`, `45 seconds`. Seconds only show below a minute.
pub fn humanize(seconds: i64) -> String {
    let abs = seconds.unsigned_abs() as i64;
    if abs == 0 {
        return "no time".into();
    }
    let (days, hours, minutes, secs) = (abs / 86_400, abs % 86_400 / 3_600, abs % 3_600 / 60, abs % 60);
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(plural(days, "day"));
    }
    if hours > 0 {
        parts.push(plural(hours, "hour"));
    }
    if minutes > 0 {
        parts.push(plural(minutes, "minute"));
    }
    if secs > 0 && abs < 60 {
        parts.push(plural(secs, "second"));
    }
    parts.join(" ")
}

#[async_trait::async_trait]
impl VizierTool for TimeBetween {
    type Input = TimeBetweenArgs;
    type Output = TimeResult;

    fn name() -> String {
        "time_between".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Time)
    }

    fn description(&self) -> String {
        format!("The difference between two moments, signed, with `direction` saying whether `to` is in the future or past of `from`. Times with no offset are local to the instance. {MOMENT_TEXT}")
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(run_between(args))
    }
}

fn run_between(args: TimeBetweenArgs) -> TimeResult {
    let zone = Zone::instance();
    let read = |text: &str| in_zone!(&zone, |tz| read_moment(text, &tz).map(|at| at.with_timezone(&chrono::Utc)));
    let from = match read(&args.from) {
        Ok(at) => at,
        Err(refusal) => return *refusal,
    };
    let to = match args.to.as_deref() {
        Some(text) => match read(text) {
            Ok(at) => at,
            Err(refusal) => return *refusal,
        },
        None => time::now().with_timezone(&chrono::Utc),
    };
    let seconds = (to - from).num_seconds();
    let direction = match seconds {
        0 => "same",
        s if s > 0 => "future",
        _ => "past",
    };
    TimeResult {
        seconds: Some(seconds),
        text: Some(humanize(seconds)),
        direction: Some(direction.into()),
        ..TimeResult::ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::{America::New_York, Asia::Jakarta};

    fn shift_args() -> ShiftTimeArgs {
        ShiftTimeArgs {
            at: None,
            zone: None,
            add_days: None,
            add_hours: None,
            add_minutes: None,
            to_weekday: None,
            direction: None,
            at_time: None,
        }
    }

    fn shifted<T: TimeZone>(tz: &T, args: ShiftTimeArgs) -> Moment
    where
        T::Offset: std::fmt::Display,
    {
        let (at, _) = shift(tz, &args).unwrap_or_else(|e| panic!("{e:?}"));
        Moment::of(&at, None)
    }

    #[test]
    fn the_weekday_move_never_stays_on_the_same_day() {
        // 2026-11-06 is a Friday.
        for (target, previous, expected) in [
            ("friday", false, "2026-11-13"),
            ("friday", true, "2026-10-30"),
            ("saturday", false, "2026-11-07"),
            ("thursday", true, "2026-11-05"),
            ("tuesday", false, "2026-11-10"),
            ("monday", true, "2026-11-02"),
        ] {
            let moved = shifted(
                &Jakarta,
                ShiftTimeArgs {
                    at: Some("2026-11-06T10:00".into()),
                    to_weekday: Some(target.into()),
                    direction: previous.then(|| "previous".into()),
                    ..shift_args()
                },
            );
            assert_eq!(moved.date, expected, "{target} previous={previous}");
            assert_eq!(moved.time, "10:00");
        }
    }

    #[test]
    fn a_weekday_and_a_time_of_day_give_next_tuesday_at_nine() {
        let moved = shifted(
            &Jakarta,
            ShiftTimeArgs {
                at: Some("2026-11-06T10:00:00".into()),
                to_weekday: Some("tuesday".into()),
                at_time: Some("09:00".into()),
                ..shift_args()
            },
        );
        assert_eq!((moved.date.as_str(), moved.time.as_str(), moved.weekday.as_str()), ("2026-11-10", "09:00", "Tuesday"));
    }

    #[test]
    fn days_keep_the_wall_clock_and_hours_stay_absolute_across_a_change() {
        // New York's clocks go forward at 02:00 on 2026-03-08.
        let day = shifted(&New_York, ShiftTimeArgs { at: Some("2026-03-07T09:00".into()), add_days: Some(1), ..shift_args() });
        assert_eq!((day.date.as_str(), day.time.as_str(), day.offset.as_str()), ("2026-03-08", "09:00", "-04:00"));
        let hours = shifted(&New_York, ShiftTimeArgs { at: Some("2026-03-07T09:00".into()), add_hours: Some(24), ..shift_args() });
        assert_eq!((hours.date.as_str(), hours.time.as_str()), ("2026-03-08", "10:00"));
    }

    #[test]
    fn a_result_in_a_gap_moves_to_the_first_valid_time_with_a_note() {
        let (at, note) = shift(
            &New_York,
            &ShiftTimeArgs { at: Some("2026-03-07T02:30".into()), add_days: Some(1), ..shift_args() },
        )
        .unwrap();
        assert_eq!(Moment::of(&at, None).time, "03:00");
        assert!(note.unwrap().contains("does not exist"));
    }

    #[test]
    fn a_shift_comes_from_a_zone_as_well_as_the_instance() {
        let moved = shifted(&Jakarta, ShiftTimeArgs { at: Some("2026-11-06T23:30".into()), add_minutes: Some(45), ..shift_args() });
        assert_eq!((moved.date.as_str(), moved.time.as_str(), moved.offset.as_str()), ("2026-11-07", "00:15", "+07:00"));
    }

    #[test]
    fn bad_input_is_a_refusal_with_the_accepted_forms() {
        let weekday = shift(&Jakarta, &ShiftTimeArgs { to_weekday: Some("funday".into()), ..shift_args() }).unwrap_err();
        assert_eq!(weekday.status, "invalid_argument");
        let direction = shift(
            &Jakarta,
            &ShiftTimeArgs { to_weekday: Some("monday".into()), direction: Some("sideways".into()), ..shift_args() },
        )
        .unwrap_err();
        assert_eq!(direction.status, "invalid_argument");
        let clock = shift(&Jakarta, &ShiftTimeArgs { at_time: Some("nine".into()), ..shift_args() }).unwrap_err();
        assert_eq!(clock.status, "invalid_time");
        assert!(clock.expected.is_some());
        let moment = shift(&Jakarta, &ShiftTimeArgs { at: Some("tomorrow".into()), add_days: Some(1), ..shift_args() }).unwrap_err();
        assert_eq!(moment.status, "invalid_time");
    }

    #[test]
    fn an_unknown_zone_suggests_the_nearest_names() {
        let refusal = Zone::named(Some("Asia/Jakrta")).err().unwrap();
        assert_eq!(refusal.status, "unknown_zone");
        assert!(refusal.did_you_mean.unwrap().contains(&"Asia/Jakarta".to_string()));
        assert!(matches!(Zone::named(Some("asia/jakarta")), Ok(Zone::Named(tz)) if tz == Jakarta));
    }

    #[test]
    fn a_gap_in_the_input_names_the_first_valid_moment() {
        let refusal = read_moment("2030-03-10T02:30:00", &New_York).unwrap_err();
        assert_eq!(refusal.status, "in_gap");
        assert_eq!(refusal.first_valid.unwrap().iso, "2030-03-10T03:00:00-04:00");
    }

    #[test]
    fn a_difference_reads_in_plain_words() {
        assert_eq!(humanize(5_400), "1 hour 30 minutes");
        assert_eq!(humanize(-90_000), "1 day 1 hour");
        assert_eq!(humanize(45), "45 seconds");
        assert_eq!(humanize(61), "1 minute");
        assert_eq!(humanize(0), "no time");
    }

    #[test]
    fn a_moment_describes_itself() {
        let at = time::parse_moment_in("2026-11-07T09:00:00", &Jakarta).unwrap();
        assert_eq!(
            Moment::of(&at, Some("Asia/Jakarta".into())),
            Moment {
                iso: "2026-11-07T09:00:00+07:00".into(),
                date: "2026-11-07".into(),
                time: "09:00".into(),
                weekday: "Saturday".into(),
                zone: Some("Asia/Jakarta".into()),
                offset: "+07:00".into(),
            }
        );
    }

    #[test]
    fn the_tools_answer_through_their_input_types() {
        let converted = run_convert(ConvertTimeArgs {
            at: "2026-11-07T09:00:00".into(),
            to_zone: "Europe/London".into(),
            from_zone: Some("Asia/Jakarta".into()),
        });
        assert_eq!(converted.status, "ok");
        assert_eq!(converted.to.unwrap().time, "02:00");

        let between = run_between(TimeBetweenArgs {
            from: "2026-11-07T09:00:00+07:00".into(),
            to: Some("2026-11-07T10:30:00+07:00".into()),
        });
        assert_eq!((between.seconds, between.direction.as_deref()), (Some(5_400), Some("future")));

        let nothing = run_shift(shift_args());
        assert_eq!(nothing.status, "invalid_argument");

        let bad = run_current(CurrentTimeArgs { zone: Some("Asia/Jakrta".into()) });
        assert_eq!(bad.status, "unknown_zone");

        let now = run_current(CurrentTimeArgs { zone: None });
        assert_eq!(now.status, "ok");
        assert!(now.now.is_some());
    }

    #[test]
    fn the_time_of_day_is_kept_on_a_move() {
        let at = time::parse_moment_in("2026-11-06T10:15:30", &Jakarta).unwrap();
        let mut note = None;
        let moved = move_wall_clock(&at, NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(), at.time(), &mut note);
        assert_eq!((moved.hour(), moved.minute(), moved.second()), (10, 15, 30));
        assert!(note.is_none());
    }
}
