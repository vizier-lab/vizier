//! The one HTTP `Range` parser (`specs/035-add-artifacts` T029), shared by session-file and
//! artifact content downloads.

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RangeRequest {
    /// Not a single `bytes=a-b` range: answered with the whole file, as without the header.
    Ignore,
    Unsatisfiable,
    /// Inclusive byte positions, already clamped to the file.
    Bytes {
        start: u64,
        end: u64,
    },
}

/// Parses one `Range: bytes=a-b`, `bytes=a-` or `bytes=-n` header against a file of `len` bytes.
pub(crate) fn parse_range(header: &str, len: u64) -> RangeRequest {
    let Some(spec) = header.trim().strip_prefix("bytes=") else {
        return RangeRequest::Ignore;
    };
    if spec.contains(',') {
        return RangeRequest::Ignore;
    }
    let Some((from, to)) = spec.split_once('-') else {
        return RangeRequest::Ignore;
    };
    let (from, to) = (from.trim(), to.trim());
    let range = match (from.is_empty(), to.is_empty()) {
        (true, true) => return RangeRequest::Ignore,
        // The last n bytes.
        (true, false) => {
            let Ok(n) = to.parse::<u64>() else {
                return RangeRequest::Ignore;
            };
            if n == 0 || len == 0 {
                return RangeRequest::Unsatisfiable;
            }
            (len.saturating_sub(n), len - 1)
        }
        (false, to_empty) => {
            let Ok(start) = from.parse::<u64>() else {
                return RangeRequest::Ignore;
            };
            let end = if to_empty {
                len.saturating_sub(1)
            } else {
                match to.parse::<u64>() {
                    Ok(e) => e.min(len.saturating_sub(1)),
                    Err(_) => return RangeRequest::Ignore,
                }
            };
            (start, end)
        }
    };
    if len == 0 || range.0 >= len || range.0 > range.1 {
        return RangeRequest::Unsatisfiable;
    }
    RangeRequest::Bytes {
        start: range.0,
        end: range.1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_single_range_forms() {
        assert_eq!(
            parse_range("bytes=0-99", 1000),
            RangeRequest::Bytes { start: 0, end: 99 }
        );
        assert_eq!(
            parse_range("bytes=900-", 1000),
            RangeRequest::Bytes {
                start: 900,
                end: 999
            }
        );
        assert_eq!(
            parse_range("bytes=-100", 1000),
            RangeRequest::Bytes {
                start: 900,
                end: 999
            }
        );
        assert_eq!(
            parse_range("bytes=990-5000", 1000),
            RangeRequest::Bytes {
                start: 990,
                end: 999
            }
        );
        assert_eq!(
            parse_range("bytes=-5000", 1000),
            RangeRequest::Bytes { start: 0, end: 999 }
        );
    }

    #[test]
    fn refuses_out_of_bounds() {
        assert_eq!(
            parse_range("bytes=1000-", 1000),
            RangeRequest::Unsatisfiable
        );
        assert_eq!(parse_range("bytes=5-2", 1000), RangeRequest::Unsatisfiable);
        assert_eq!(parse_range("bytes=0-1", 0), RangeRequest::Unsatisfiable);
    }

    #[test]
    fn ignores_what_it_does_not_support() {
        assert_eq!(parse_range("items=0-5", 100), RangeRequest::Ignore);
        assert_eq!(parse_range("bytes=0-5,10-20", 100), RangeRequest::Ignore);
        assert_eq!(parse_range("bytes=a-b", 100), RangeRequest::Ignore);
        assert_eq!(parse_range("bytes=-", 100), RangeRequest::Ignore);
    }
}
