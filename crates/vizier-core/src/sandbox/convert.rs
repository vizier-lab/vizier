//! `serde_json::Value` ⇄ `MontyObject`. Monty's own `Serialize` is an externally
//! tagged snapshot format (`{"Int": 42}`), so tools never see it.

use base64::Engine;
use monty_types::{
    MontyDateTime, MontyObject, MontyTime, MontyTimeDelta, ObjectRef,
    unstable::{self, MontyNode},
};
use serde_json::{Map, Number, Value};

/// A script value that has no plain-data JSON form.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversionError {
    pub python_type: String,
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cannot return {}; return plain data (str, int, float, bool, None, list, dict)",
            self.python_type
        )
    }
}

/// Host → script. Total: every JSON value has a Python form.
pub fn json_to_monty(value: Value) -> MontyObject {
    match value {
        Value::Null => MontyObject::none(),
        Value::Bool(b) => MontyObject::bool(b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => MontyObject::int(i),
            None => MontyObject::float(n.as_f64().unwrap_or(f64::NAN)),
        },
        Value::String(s) => MontyObject::string(s),
        Value::Array(items) => MontyObject::list(items.into_iter().map(json_to_monty)),
        Value::Object(map) => MontyObject::dict(
            map.into_iter()
                .map(|(k, v)| (MontyObject::string(k), json_to_monty(v))),
        ),
    }
}

/// Script → host, for a script's result and for tool arguments.
pub fn monty_to_json(value: &MontyObject) -> Result<Value, ConversionError> {
    ref_to_json(value.as_ref())
}

/// Matches on Monty's storage nodes through `unstable`, which carries no
/// compatibility guarantee: the stable accessors cannot reach bytes, big ints or
/// date/time values. `monty` is pinned exactly in `Cargo.toml`, so a change here
/// surfaces as a compile error on the next bump.
pub fn ref_to_json(value: ObjectRef<'_>) -> Result<Value, ConversionError> {
    Ok(match unstable::node(value) {
        MontyNode::None => Value::Null,
        MontyNode::Bool(b) => Value::Bool(*b),
        MontyNode::Int(i) => Value::from(*i),
        MontyNode::BigInt(bi) => {
            let digits = bi.to_string();
            match digits.parse::<i64>() {
                Ok(i) => Value::from(i),
                Err(_) => Value::String(digits),
            }
        }
        MontyNode::Float(f) => float_to_json(*f),
        MontyNode::String(s) | MontyNode::Path(s) => Value::String(s.clone()),
        MontyNode::Bytes(b) => Value::String(base64::engine::general_purpose::STANDARD.encode(b)),
        MontyNode::List(_) | MontyNode::Tuple(_) | MontyNode::NamedTuple { .. } => {
            array(value.items().unwrap_or_default())?
        }
        MontyNode::Set(_) | MontyNode::FrozenSet(_) => {
            let mut sorted = value.items().unwrap_or_default();
            sorted.sort_by_cached_key(|item| item.py_repr());
            array(sorted)?
        }
        MontyNode::Dict(_) => {
            let mut map = Map::new();
            for (key, value) in value.pairs().unwrap_or_default() {
                let key = match key.as_str() {
                    Some(s) => s.to_string(),
                    None => key.py_repr(),
                };
                map.insert(key, ref_to_json(value)?);
            }
            Value::Object(map)
        }
        MontyNode::Date(d) => Value::String(format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)),
        MontyNode::DateTime(dt) => Value::String(datetime_iso(dt)),
        MontyNode::Time(t) => Value::String(time_iso(t)),
        MontyNode::TimeDelta(td) => Value::String(timedelta_iso(td)),
        MontyNode::TimeZone(tz) => Value::String(match &tz.name {
            Some(name) => name.clone(),
            None => format!("UTC{}", offset_iso(tz.offset_seconds)),
        }),
        _ => {
            return Err(ConversionError {
                python_type: value.type_name().to_string(),
            });
        }
    })
}

fn array(items: Vec<ObjectRef<'_>>) -> Result<Value, ConversionError> {
    Ok(Value::Array(
        items.into_iter().map(ref_to_json).collect::<Result<_, _>>()?,
    ))
}

fn float_to_json(f: f64) -> Value {
    if f.is_nan() {
        Value::String("NaN".into())
    } else if f.is_infinite() {
        Value::String(if f > 0.0 { "Infinity" } else { "-Infinity" }.into())
    } else {
        Number::from_f64(f).map_or(Value::Null, Value::Number)
    }
}

fn offset_iso(offset_seconds: i32) -> String {
    let sign = if offset_seconds < 0 { '-' } else { '+' };
    let abs = offset_seconds.unsigned_abs();
    format!("{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
}

fn clock_iso(hour: u8, minute: u8, second: u8, microsecond: u32, offset: Option<i32>) -> String {
    let mut out = format!("{hour:02}:{minute:02}:{second:02}");
    if microsecond > 0 {
        out.push_str(&format!(".{microsecond:06}"));
    }
    if let Some(offset) = offset {
        out.push_str(&offset_iso(offset));
    }
    out
}

fn datetime_iso(dt: &MontyDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{}",
        dt.year,
        dt.month,
        dt.day,
        clock_iso(dt.hour, dt.minute, dt.second, dt.microsecond, dt.offset_seconds)
    )
}

fn time_iso(t: &MontyTime) -> String {
    clock_iso(t.hour, t.minute, t.second, t.microsecond, t.offset_seconds)
}

/// ISO-8601 duration in seconds, e.g. `PT90S`, `-PT0.5S`.
fn timedelta_iso(td: &MontyTimeDelta) -> String {
    let micros = (i64::from(td.days) * 86_400 + i64::from(td.seconds)) * 1_000_000
        + i64::from(td.microseconds);
    let sign = if micros < 0 { "-" } else { "" };
    let abs = micros.unsigned_abs();
    let (secs, frac) = (abs / 1_000_000, abs % 1_000_000);
    if frac == 0 {
        format!("{sign}PT{secs}S")
    } else {
        let frac = format!("{frac:06}");
        format!("{sign}PT{secs}.{}S", frac.trim_end_matches('0'))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn round_trip(value: Value) -> Value {
        monty_to_json(&json_to_monty(value)).expect("plain JSON always converts back")
    }

    #[test]
    fn plain_json_round_trips() {
        for value in [
            json!(null),
            json!(true),
            json!(42),
            json!(-7),
            json!(1.5),
            json!("text"),
            json!([1, [2, [3, "x"]], {"k": null}]),
            json!({"a": {"b": [1, 2.5, false]}, "c": "d"}),
        ] {
            assert_eq!(round_trip(value.clone()), value);
        }
    }

    #[test]
    fn object_key_order_is_preserved_into_python() {
        let dict = json_to_monty(json!({"a": 1, "b": 2}));
        let pairs = dict.as_ref().pairs().expect("object converts to dict");
        let keys: Vec<_> = pairs.iter().map(|(k, _)| k.py_repr()).collect();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn big_ints_become_decimal_strings() {
        let big = monty::MontyRun::new("2 ** 100".into(), "t.py", vec![], Default::default())
            .unwrap()
            .run_no_limits(vec![])
            .unwrap();
        assert!(matches!(unstable::root_node(&big), MontyNode::BigInt(_)));
        assert_eq!(monty_to_json(&big).unwrap(), json!("1267650600228229401496703205376"));
    }

    #[test]
    fn non_finite_floats_become_strings() {
        assert_eq!(monty_to_json(&MontyObject::float(f64::NAN)).unwrap(), json!("NaN"));
        assert_eq!(
            monty_to_json(&MontyObject::float(f64::NEG_INFINITY)).unwrap(),
            json!("-Infinity")
        );
    }

    #[test]
    fn non_string_dict_keys_use_their_repr() {
        let dict = MontyObject::dict(vec![(MontyObject::int(1), MontyObject::bool(true))]);
        assert_eq!(monty_to_json(&dict).unwrap(), json!({"1": true}));
    }

    #[test]
    fn functions_cannot_be_converted() {
        let err = monty_to_json(&MontyObject::function("f", None))
        .unwrap_err();
        assert_eq!(err.python_type, "function");
        assert!(err.to_string().contains("cannot return function"));
    }

    #[test]
    fn dates_and_durations_are_iso_strings() {
        let date = MontyObject::date(monty_types::MontyDate {
            year: 2026,
            month: 9,
            day: 16,
        });
        assert_eq!(monty_to_json(&date).unwrap(), json!("2026-09-16"));
        let delta = MontyObject::timedelta(MontyTimeDelta {
            days: 1,
            seconds: 30,
            microseconds: 500_000,
        });
        assert_eq!(monty_to_json(&delta).unwrap(), json!("PT86430.5S"));
    }
}
