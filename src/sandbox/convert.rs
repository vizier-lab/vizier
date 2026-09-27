//! `serde_json::Value` ⇄ `MontyObject`. Monty's own `Serialize` is an externally
//! tagged snapshot format (`{"Int": 42}`), so tools never see it.

use base64::Engine;
use monty_types::{MontyDateTime, MontyObject, MontyTime, MontyTimeDelta};
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
        Value::Null => MontyObject::None,
        Value::Bool(b) => MontyObject::Bool(b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => MontyObject::Int(i),
            None => MontyObject::Float(n.as_f64().unwrap_or(f64::NAN)),
        },
        Value::String(s) => MontyObject::String(s),
        Value::Array(items) => MontyObject::List(items.into_iter().map(json_to_monty).collect()),
        Value::Object(map) => MontyObject::dict(
            map.into_iter()
                .map(|(k, v)| (MontyObject::String(k), json_to_monty(v)))
                .collect::<Vec<_>>(),
        ),
    }
}

/// Script → host, for a script's result and for tool arguments.
pub fn monty_to_json(value: &MontyObject) -> Result<Value, ConversionError> {
    Ok(match value {
        MontyObject::None => Value::Null,
        MontyObject::Bool(b) => Value::Bool(*b),
        MontyObject::Int(i) => Value::from(*i),
        MontyObject::BigInt(bi) => {
            let digits = bi.to_string();
            match digits.parse::<i64>() {
                Ok(i) => Value::from(i),
                Err(_) => Value::String(digits),
            }
        }
        MontyObject::Float(f) => float_to_json(*f),
        MontyObject::String(s) | MontyObject::Path(s) => Value::String(s.clone()),
        MontyObject::Bytes(b) => {
            Value::String(base64::engine::general_purpose::STANDARD.encode(b))
        }
        MontyObject::List(items)
        | MontyObject::Tuple(items)
        | MontyObject::NamedTuple { values: items, .. } => array(items)?,
        MontyObject::Set(items) | MontyObject::FrozenSet(items) => {
            let mut sorted: Vec<&MontyObject> = items.iter().collect();
            sorted.sort_by_cached_key(|item| item.py_repr());
            Value::Array(
                sorted
                    .into_iter()
                    .map(monty_to_json)
                    .collect::<Result<_, _>>()?,
            )
        }
        MontyObject::Dict(pairs) => {
            let mut map = Map::new();
            for (key, value) in pairs {
                let key = match key {
                    MontyObject::String(s) => s.clone(),
                    other => other.py_repr(),
                };
                map.insert(key, monty_to_json(value)?);
            }
            Value::Object(map)
        }
        MontyObject::Date(d) => Value::String(format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)),
        MontyObject::DateTime(dt) => Value::String(datetime_iso(dt)),
        MontyObject::Time(t) => Value::String(time_iso(t)),
        MontyObject::TimeDelta(td) => Value::String(timedelta_iso(td)),
        MontyObject::TimeZone(tz) => Value::String(match &tz.name {
            Some(name) => name.clone(),
            None => format!("UTC{}", offset_iso(tz.offset_seconds)),
        }),
        other => {
            return Err(ConversionError {
                python_type: other.type_name().to_string(),
            });
        }
    })
}

fn array(items: &[MontyObject]) -> Result<Value, ConversionError> {
    Ok(Value::Array(
        items.iter().map(monty_to_json).collect::<Result<_, _>>()?,
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
        let MontyObject::Dict(pairs) = json_to_monty(json!({"a": 1, "b": 2})) else {
            panic!("object converts to dict");
        };
        let keys: Vec<_> = pairs.into_iter().map(|(k, _)| k.py_repr()).collect();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn big_ints_become_decimal_strings() {
        let big = monty::MontyRun::new("2 ** 100".into(), "t.py", vec![], Default::default())
            .unwrap()
            .run_no_limits(vec![])
            .unwrap();
        assert!(matches!(big, MontyObject::BigInt(_)));
        assert_eq!(monty_to_json(&big).unwrap(), json!("1267650600228229401496703205376"));
    }

    #[test]
    fn non_finite_floats_become_strings() {
        assert_eq!(monty_to_json(&MontyObject::Float(f64::NAN)).unwrap(), json!("NaN"));
        assert_eq!(
            monty_to_json(&MontyObject::Float(f64::NEG_INFINITY)).unwrap(),
            json!("-Infinity")
        );
    }

    #[test]
    fn non_string_dict_keys_use_their_repr() {
        let dict = MontyObject::dict(vec![(MontyObject::Int(1), MontyObject::Bool(true))]);
        assert_eq!(monty_to_json(&dict).unwrap(), json!({"1": true}));
    }

    #[test]
    fn functions_cannot_be_converted() {
        let err = monty_to_json(&MontyObject::Function {
            name: "f".into(),
            docstring: None,
        })
        .unwrap_err();
        assert_eq!(err.python_type, "function");
        assert!(err.to_string().contains("cannot return function"));
    }

    #[test]
    fn dates_and_durations_are_iso_strings() {
        let date = MontyObject::Date(monty_types::MontyDate {
            year: 2026,
            month: 9,
            day: 16,
        });
        assert_eq!(monty_to_json(&date).unwrap(), json!("2026-09-16"));
        let delta = MontyObject::TimeDelta(MontyTimeDelta {
            days: 1,
            seconds: 30,
            microseconds: 500_000,
        });
        assert_eq!(monty_to_json(&delta).unwrap(), json!("PT86430.5S"));
    }
}
