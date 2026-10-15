//! A whole-body Server-Sent Events parser. Only the ChatGPT backend requires streaming, and
//! like rig we read the full body and parse it once (research R6): no streaming API.

/// One event: its `event:` name, if any, and its `data:` (multi-line fields joined with `\n`).
#[derive(Debug, Clone, PartialEq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Parse a complete SSE body. Comments (`:`) and `[DONE]` sentinels are skipped, as are
/// events without data.
pub fn parse_sse_body(body: &str) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut event: Option<String> = None;
    let mut data: Vec<&str> = Vec::new();

    let mut flush = |event: &mut Option<String>, data: &mut Vec<&str>| {
        if !data.is_empty() {
            let joined = data.join("\n");
            if joined.trim() != "[DONE]" {
                events.push(SseEvent {
                    event: event.take(),
                    data: joined,
                });
            }
        }
        *event = None;
        data.clear();
    };

    for raw in body.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.is_empty() {
            flush(&mut event, &mut data);
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => event = Some(value.to_string()),
            "data" => data.push(value),
            _ => {}
        }
    }
    flush(&mut event, &mut data);
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events_comments_and_done() {
        let body = ": keep-alive\n\
                    event: response.created\n\
                    data: {\"a\":1}\n\
                    \n\
                    data: line one\n\
                    data: line two\n\
                    \n\
                    data: [DONE]\n\
                    \n";
        let events = parse_sse_body(body);
        assert_eq!(
            events,
            vec![
                SseEvent {
                    event: Some("response.created".into()),
                    data: "{\"a\":1}".into()
                },
                SseEvent {
                    event: None,
                    data: "line one\nline two".into()
                },
            ]
        );
    }

    #[test]
    fn handles_crlf_and_a_missing_trailing_blank_line() {
        let body = "event: x\r\ndata:{\"b\":2}\r\n\r\ndata: tail";
        let events = parse_sse_body(body);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "{\"b\":2}");
        assert_eq!(events[1].data, "tail");
        assert_eq!(events[1].event, None);
    }
}
