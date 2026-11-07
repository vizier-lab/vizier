//! The instance's one clock (`specs/052-unify-local-timezone`).
//!
//! Vizier runs in the timezone of the machine or container it is on, as set through the operating
//! system. Every time the engine holds is a [`Timestamp`], and everything a person or an agent
//! reads carries that zone's offset.
//!
//! Two things stay on purpose outside it:
//!
//! - Text columns used for sorting and keyset paging are written in UTC through [`stored`],
//!   because text with mixed offsets does not sort chronologically (FR-006).
//! - External protocols (AWS signing, HTTP dates) use the format their protocol names.
//!
//! Logic that depends on a zone is written over `chrono::TimeZone` and exercised with `chrono_tz`
//! zones, because a test cannot change the process's `Local`.

use std::sync::OnceLock;

use chrono::{
    DateTime, Datelike, Duration, FixedOffset, Local, LocalResult, NaiveDate, NaiveDateTime,
    NaiveTime, Offset, TimeZone, Timelike, Utc,
};

use crate::error::VizierError;

/// A moment, shown in the instance's zone.
///
/// Compares and hashes by instant. Serializes as RFC 3339 with the local offset (`Z` at a zero
/// offset, so a UTC instance's output is unchanged), and reads any RFC 3339 offset.
///
/// A type deriving `utoipa::ToSchema` must spell this `DateTime<Local>`: the derive recognises a
/// time by its syntactic name and cannot see through an alias.
pub type Timestamp = DateTime<Local>;

/// The current moment.
pub fn now() -> Timestamp {
    Local::now()
}

/// The Unix epoch, in the instance's zone.
pub fn epoch() -> Timestamp {
    DateTime::<Utc>::UNIX_EPOCH.with_timezone(&Local)
}

/// A moment from epoch milliseconds, or `None` when out of range.
pub fn from_millis_opt(millis: i64) -> Option<Timestamp> {
    DateTime::from_timestamp_millis(millis).map(|at| at.with_timezone(&Local))
}

/// A moment from epoch milliseconds. An out-of-range value reads as the epoch.
pub fn from_millis(millis: i64) -> Timestamp {
    from_millis_opt(millis).unwrap_or_else(epoch)
}

/// A moment from epoch seconds and nanoseconds, or `None` when out of range.
pub fn from_secs_opt(secs: i64, nanos: u32) -> Option<Timestamp> {
    DateTime::from_timestamp(secs, nanos).map(|at| at.with_timezone(&Local))
}

/// A moment for a message a person reads: `2026-11-07 09:00:00 +07:00`, or `... UTC` at a zero
/// offset, which is what the error messages said before instances had a zone of their own.
pub fn display(at: Timestamp) -> String {
    let offset = at.offset().fix().local_minus_utc();
    if offset == 0 {
        format!("{} UTC", at.naive_local())
    } else {
        format!("{} {}", at.naive_local(), at.format("%:z"))
    }
}

/// The fixed text form for a column that is sorted or compared as text.
///
/// Always UTC, so existing rows stay valid and text order is chronological order. Nothing a person
/// or an agent reads comes from this form: the column is read back into a [`Timestamp`].
pub fn stored(at: Timestamp) -> String {
    at.with_timezone(&Utc).to_rfc3339()
}

/// [`stored`] for the current moment.
pub fn stored_now() -> String {
    stored(now())
}

/// Serializes a moment that is part of a key in UTC, for `#[serde(serialize_with)]`.
///
/// A task run's channel is written as JSON into text columns and looked up by re-serializing it,
/// so its form must not depend on the instance's zone: rows written before local time, or before
/// the host changed zone, have to keep matching.
pub fn serialize_utc<S: serde::Serializer>(at: &Timestamp, serializer: S) -> Result<S::Ok, S::Error> {
    serde::Serialize::serialize(&at.with_timezone(&Utc), serializer)
}

/// Reads a time from a URL query value. A raw `+` in a query decodes to a space, so a client echoing
/// a returned `...T09:00:00+07:00` unencoded arrives as `...T09:00:00 07:00`; that form reads too.
pub fn parse_query_time(value: &str) -> Option<Timestamp> {
    let read = |text: &str| {
        DateTime::parse_from_rfc3339(text)
            .ok()
            .map(|at| at.with_timezone(&Local))
    };
    read(value).or_else(|| read(&value.replace(' ', "+")))
}

/// [`parse_query_time`] for an optional `#[serde(default, deserialize_with)]` query field.
pub fn deserialize_query_time<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Timestamp>, D::Error> {
    let value: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    value
        .map(|text| {
            parse_query_time(&text)
                .ok_or_else(|| serde::de::Error::custom(format!("invalid time: {text}")))
        })
        .transpose()
}

/// The instance's timezone, as reported to clients and the startup log.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Zone {
    /// IANA name, such as `Asia/Jakarta`. `None` only when the host's zone cannot be named.
    pub name: Option<String>,
    /// The current offset, such as `+07:00`. It changes with daylight saving, so it is computed
    /// on every read.
    pub offset: String,
}

static ZONE_NAME: OnceLock<Option<String>> = OnceLock::new();

/// The instance's zone. Before [`init_zone`] has run, the name is the host's own, unchecked.
pub fn zone() -> Zone {
    let name = ZONE_NAME
        .get()
        .cloned()
        .unwrap_or_else(|| iana_time_zone::get_timezone().ok());
    Zone {
        name,
        offset: now().format("%:z").to_string(),
    }
}

/// `2026-11-07 09:00 +07:00`-style zone suffix for confirmations: `+07:00 (Asia/Jakarta)`.
pub fn describe_zone(at: Timestamp) -> String {
    let offset = at.format("%:z");
    match zone().name {
        Some(name) => format!("{offset} ({name})"),
        None => offset.to_string(),
    }
}

/// Resolves and checks the zone. Called once, first, by `VizierRuntime::start`.
///
/// chrono's `Local` silently falls back to UTC for an unknown `TZ`, and for a valid name when the
/// host has no zone files. This refuses both rather than run on a clock nobody asked for.
pub fn init_zone() -> crate::Result<()> {
    let at = Utc::now();
    let local_offset = at.with_timezone(&Local).offset().fix().local_minus_utc();

    let name = match tz_setting() {
        Some(value) => match parse_tz_setting(&value).map_err(VizierError)? {
            Some(tz) => {
                check_offset(&value, tz, at, local_offset).map_err(VizierError)?;
                Some(tz.name().to_string())
            }
            None => host_zone_name(at, local_offset),
        },
        None => host_zone_name(at, local_offset),
    };
    let _ = ZONE_NAME.set(name);

    let zone = zone();
    match &zone.name {
        Some(name) => info!("timezone: {name} ({})", zone.offset),
        None => warn!(
            "timezone: the host's zone has no name, running on {}. Set TZ to an IANA name such as Asia/Jakarta to name it",
            zone.offset
        ),
    }
    Ok(())
}

/// The `TZ` setting chrono reads. Windows ignores it, matching chrono.
#[cfg(unix)]
fn tz_setting() -> Option<String> {
    std::env::var("TZ").ok()
}

#[cfg(not(unix))]
fn tz_setting() -> Option<String> {
    None
}

/// The name the host reports, when it can be trusted.
///
/// A mismatch here warns rather than refuses: unlike a bad `TZ` it can't be the silent UTC
/// fallback, and it has ordinary causes (Windows with automatic daylight saving switched off,
/// zone rules newer than the ones compiled in), so the instance runs on `Local`'s offset unnamed.
fn host_zone_name(at: DateTime<Utc>, local_offset: i32) -> Option<String> {
    let name = iana_time_zone::get_timezone().ok()?;
    let Ok(tz) = name.parse::<chrono_tz::Tz>() else {
        warn!("timezone: the host's zone {name:?} is not a known IANA name");
        return None;
    };
    if let Err(problem) = check_offset(&name, tz, at, local_offset) {
        warn!("timezone: {problem}");
        return None;
    }
    Some(tz.name().to_string())
}

/// Reads a `TZ` value: empty means unset, a leading `:` is dropped, and anything other than an
/// IANA name is refused, including POSIX rules such as `UTC-7`.
fn parse_tz_setting(value: &str) -> Result<Option<chrono_tz::Tz>, String> {
    let name = value.strip_prefix(':').unwrap_or(value).trim();
    if name.is_empty() {
        return Ok(None);
    }
    name.parse::<chrono_tz::Tz>().map(Some).map_err(|_| {
        format!("TZ={value:?} is not an IANA timezone name. Use a name such as Asia/Jakarta")
    })
}

/// A known name must give the offset `Local` runs on. A mismatch means the OS could not find the
/// zone and chrono fell back to UTC.
fn check_offset(
    name: &str,
    tz: chrono_tz::Tz,
    at: DateTime<Utc>,
    local_offset: i32,
) -> Result<(), String> {
    let expected = at.with_timezone(&tz).offset().fix().local_minus_utc();
    if expected == local_offset {
        return Ok(());
    }
    Err(format!(
        "timezone {name} should be at {} but the system is running at {}: zone data is missing. \
         Install the tzdata package, or the system's zone files",
        FixedOffset::east_opt(expected).unwrap_or(Utc.fix()),
        FixedOffset::east_opt(local_offset).unwrap_or(Utc.fix()),
    ))
}

/// Why a scheduling time can't be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleTimeError {
    /// Neither RFC 3339 nor a local date and time.
    Unparseable,
    /// The local time does not exist on that date, because of a daylight-saving change.
    InGap { first_valid: DateTime<FixedOffset> },
}

impl std::fmt::Display for ScheduleTimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unparseable => write!(
                f,
                "expected a local date and time such as 2026-11-07T09:00, or RFC 3339 with an offset"
            ),
            Self::InGap { first_valid } => write!(
                f,
                "that local time does not exist because the clocks change on that date; the first valid time after it is {}",
                first_valid.to_rfc3339()
            ),
        }
    }
}

impl std::error::Error for ScheduleTimeError {}

/// Reads a scheduling time. An offset means that exact moment; no offset means local time.
pub fn parse_schedule(text: &str) -> Result<Timestamp, ScheduleTimeError> {
    parse_moment_in(text, &Local).map(|at| at.with_timezone(&Local))
}

/// [`parse_schedule`] in a given zone: a time with no offset is read as that zone's wall clock, an
/// overlap takes the earlier occurrence, and a gap is an error.
pub fn parse_moment_in<Tz: TimeZone>(
    text: &str,
    zone: &Tz,
) -> Result<DateTime<Tz>, ScheduleTimeError> {
    let text = text.trim();
    if let Ok(at) = DateTime::parse_from_rfc3339(text) {
        return Ok(at.with_timezone(zone));
    }
    let naive = parse_wall_clock(text).ok_or(ScheduleTimeError::Unparseable)?;
    match resolve_wall_clock(naive, zone) {
        WallClock::Exact(at) | WallClock::Overlap(at) => Ok(at),
        WallClock::Gap { first_valid } => Err(ScheduleTimeError::InGap {
            first_valid: first_valid.fixed_offset(),
        }),
    }
}

fn parse_wall_clock(text: &str) -> Option<NaiveDateTime> {
    const FORMATS: [&str; 4] = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M",
    ];
    FORMATS
        .iter()
        .find_map(|format| NaiveDateTime::parse_from_str(text, format).ok())
}

/// What a wall-clock time means in a zone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallClock<Tz: TimeZone> {
    Exact(DateTime<Tz>),
    /// It happens twice; this is the earlier one.
    Overlap(DateTime<Tz>),
    /// It doesn't happen; this is the first moment after the gap.
    Gap { first_valid: DateTime<Tz> },
}

impl<Tz: TimeZone> WallClock<Tz> {
    /// The moment to use when a gap shouldn't be an error.
    pub fn into_moment(self) -> DateTime<Tz> {
        match self {
            Self::Exact(at) | Self::Overlap(at) => at,
            Self::Gap { first_valid } => first_valid,
        }
    }
}

/// Places a wall-clock time in a zone.
pub fn resolve_wall_clock<Tz: TimeZone>(naive: NaiveDateTime, zone: &Tz) -> WallClock<Tz> {
    match zone.from_local_datetime(&naive) {
        LocalResult::Single(at) => WallClock::Exact(at),
        LocalResult::Ambiguous(first, second) => WallClock::Overlap(first.min(second)),
        LocalResult::None => WallClock::Gap {
            first_valid: first_after_gap(naive, zone),
        },
    }
}

/// The first moment after a gap. The longest gap on record is a whole skipped day, so stepping by
/// minutes for 49 hours always lands.
fn first_after_gap<Tz: TimeZone>(naive: NaiveDateTime, zone: &Tz) -> DateTime<Tz> {
    let start = naive.with_second(0).unwrap_or(naive).with_nanosecond(0).unwrap_or(naive);
    for minutes in 1..=(49 * 60) {
        match zone.from_local_datetime(&(start + Duration::minutes(minutes))) {
            LocalResult::Single(at) => return at,
            LocalResult::Ambiguous(first, second) => return first.min(second),
            LocalResult::None => {}
        }
    }
    // Unreachable for any real zone; the UTC reading is at least a real moment.
    zone.from_utc_datetime(&naive)
}

/// The moment a calendar day starts in a zone. A day whose midnight falls in a gap starts at the
/// first moment after it.
pub fn start_of_day_in<Tz: TimeZone>(date: NaiveDate, zone: &Tz) -> DateTime<Tz> {
    resolve_wall_clock(date.and_time(NaiveTime::MIN), zone).into_moment()
}

/// The moment a calendar day starts on this instance.
pub fn start_of_local_day(date: NaiveDate) -> Timestamp {
    start_of_day_in(date, &Local)
}

/// The instance's calendar date of a moment.
pub fn local_date_of(at: Timestamp) -> NaiveDate {
    at.date_naive()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::{America::New_York, Asia::Jakarta};

    #[test]
    fn serializes_z_at_zero_offset_and_the_offset_otherwise() {
        let at = DateTime::parse_from_rfc3339("2026-11-07T02:00:00Z").unwrap();
        // The serde form is what stored JSON and the API carry. Pin it for fixed offsets, which is
        // what `Local` holds.
        assert_eq!(
            serde_json::to_string(&at.with_timezone(&FixedOffset::east_opt(0).unwrap())).unwrap(),
            "\"2026-11-07T02:00:00Z\""
        );
        assert_eq!(
            serde_json::to_string(&at.with_timezone(&FixedOffset::east_opt(7 * 3600).unwrap()))
                .unwrap(),
            "\"2026-11-07T09:00:00+07:00\""
        );
    }

    #[test]
    fn a_timestamp_serializes_with_the_process_offset() {
        let at: Timestamp = from_millis(1_794_020_400_000);
        let text = serde_json::to_string(&at).unwrap();
        let expected = if at.offset().fix().local_minus_utc() == 0 {
            "Z\"".to_string()
        } else {
            format!("{}\"", at.format("%:z"))
        };
        assert!(text.ends_with(&expected), "{text}");
    }

    #[test]
    fn a_stored_z_value_reads_as_the_same_instant() {
        let read: Timestamp = serde_json::from_str("\"2026-11-07T02:00:00Z\"").unwrap();
        assert_eq!(
            read.timestamp_millis(),
            DateTime::parse_from_rfc3339("2026-11-07T09:00:00+07:00")
                .unwrap()
                .timestamp_millis()
        );
    }

    #[test]
    fn stored_is_utc_whatever_the_offset() {
        let at = from_millis(1_794_020_400_000);
        let text = stored(at);
        assert!(text.ends_with("+00:00"), "{text}");
        assert_eq!(
            DateTime::parse_from_rfc3339(&text).unwrap().timestamp_millis(),
            at.timestamp_millis()
        );
    }

    #[test]
    fn stored_text_sorts_chronologically() {
        let earlier = from_millis(1_794_020_400_000);
        let later = earlier + Duration::minutes(1);
        assert!(stored(earlier) < stored(later));
    }

    #[test]
    fn a_query_time_survives_an_unencoded_plus() {
        let expected = DateTime::parse_from_rfc3339("2026-11-07T09:00:00+07:00").unwrap();
        for text in [
            "2026-11-07T09:00:00+07:00",
            "2026-11-07T09:00:00 07:00",
            "2026-11-07T02:00:00Z",
        ] {
            assert_eq!(parse_query_time(text).unwrap(), expected, "{text}");
        }
        assert_eq!(parse_query_time("nonsense"), None);
    }

    #[test]
    fn a_task_channel_key_keeps_its_utc_form() {
        use crate::schema::VizierChannelId;
        let at: Timestamp = "2026-11-07T09:00:00+07:00".parse().unwrap();
        let key = serde_json::to_string(&VizierChannelId::Task("t".into(), at)).unwrap();
        assert_eq!(key, r#"{"Task":["t","2026-11-07T02:00:00Z"]}"#);
        let back: VizierChannelId = serde_json::from_str(&key).unwrap();
        assert_eq!(back, VizierChannelId::Task("t".into(), at));
    }

    #[test]
    fn tz_setting_accepts_iana_names() {
        assert_eq!(parse_tz_setting("Asia/Jakarta").unwrap(), Some(Jakarta));
        assert_eq!(parse_tz_setting(":Asia/Jakarta").unwrap(), Some(Jakarta));
        assert_eq!(parse_tz_setting("").unwrap(), None);
        assert_eq!(parse_tz_setting("UTC").unwrap(), Some(chrono_tz::UTC));
    }

    #[test]
    fn tz_setting_refuses_anything_else_and_names_the_value() {
        for bad in ["Asia/Jakrta", "UTC-7", "EST5EDT4,M3.2.0,M11.1.0", "/etc/localtime"] {
            let problem = parse_tz_setting(bad).unwrap_err();
            assert!(problem.contains(bad), "{problem}");
        }
    }

    #[test]
    fn a_name_whose_offset_differs_from_local_means_missing_zone_data() {
        let at = Utc.with_ymd_and_hms(2026, 11, 7, 2, 0, 0).unwrap();
        assert!(check_offset("Asia/Jakarta", Jakarta, at, 7 * 3600).is_ok());
        let problem = check_offset("Asia/Jakarta", Jakarta, at, 0).unwrap_err();
        assert!(problem.contains("zone data is missing"), "{problem}");
        assert!(problem.contains("tzdata"), "{problem}");
    }

    #[test]
    fn a_time_with_an_offset_is_that_exact_moment() {
        let at = parse_moment_in("2026-11-07T02:00:00Z", &Jakarta).unwrap();
        assert_eq!(at.to_rfc3339(), "2026-11-07T09:00:00+07:00");
        let at = parse_moment_in("2026-11-07T09:00:00+07:00", &New_York).unwrap();
        assert_eq!(at.timestamp(), Jakarta.with_ymd_and_hms(2026, 11, 7, 9, 0, 0).unwrap().timestamp());
    }

    #[test]
    fn a_time_with_no_offset_is_local() {
        for text in [
            "2026-11-07T09:00:00",
            "2026-11-07T09:00",
            "2026-11-07 09:00:00",
            "2026-11-07 09:00",
        ] {
            assert_eq!(
                parse_moment_in(text, &Jakarta).unwrap().to_rfc3339(),
                "2026-11-07T09:00:00+07:00",
                "{text}"
            );
        }
    }

    #[test]
    fn nonsense_is_unparseable() {
        for text in ["tomorrow", "2026-13-40T09:00", "", "09:00"] {
            assert_eq!(
                parse_moment_in(text, &Jakarta).unwrap_err(),
                ScheduleTimeError::Unparseable,
                "{text}"
            );
        }
    }

    #[test]
    fn a_gap_names_the_first_valid_time() {
        let error = parse_moment_in("2030-03-10T02:30:00", &New_York).unwrap_err();
        let ScheduleTimeError::InGap { first_valid } = &error else {
            panic!("{error:?}");
        };
        assert_eq!(first_valid.to_rfc3339(), "2030-03-10T03:00:00-04:00");
        assert!(error.to_string().contains("2030-03-10T03:00:00-04:00"));
    }

    #[test]
    fn an_overlap_takes_the_earlier_occurrence() {
        let at = parse_moment_in("2030-11-03T01:30:00", &New_York).unwrap();
        assert_eq!(at.to_rfc3339(), "2030-11-03T01:30:00-04:00");
    }

    #[test]
    fn a_day_starts_at_midnight_or_after_its_gap() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        assert_eq!(start_of_day_in(date, &Jakarta).to_rfc3339(), "2026-11-07T00:00:00+07:00");
        // Havana's clocks change at midnight, so that day has no 00:00.
        let date = NaiveDate::from_ymd_opt(2026, 3, 8).unwrap();
        assert_eq!(
            start_of_day_in(date, &chrono_tz::America::Havana).to_rfc3339(),
            "2026-03-08T01:00:00-04:00"
        );
    }

    #[test]
    fn a_days_start_stays_on_that_date() {
        let date = NaiveDate::from_ymd_opt(2030, 3, 10).unwrap();
        let start = start_of_day_in(date, &New_York);
        assert_eq!(start.date_naive(), date);
        assert_eq!((start.hour(), start.minute()), (0, 0));
        assert_eq!(start.day(), 10);
    }
}
