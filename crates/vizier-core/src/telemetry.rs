//! Tracing context the engine carries across its own channels. Exporting it (OTLP) is the
//! host's job: the server binary sets it up in `cli::telemetry_init`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// The trace a request belongs to, carried across the bus so the turn it starts joins its
/// sender's trace. It is plain strings and a timestamp, so it survives a hop between processes
/// (the W3C `traceparent` / `tracestate` headers travel in the envelope).
///
/// Without the `telemetry` feature there is no trace to carry; only the queue wait is kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestTrace {
    /// W3C `traceparent` of the span that was current where the request was sent, if traced.
    pub traceparent: Option<String>,
    /// W3C `tracestate`, when the sender had one.
    pub tracestate: Option<String>,
    /// Unix milliseconds when the request first entered an agent's queue; a re-queued request
    /// keeps it.
    pub enqueued_at_ms: Option<u64>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl RequestTrace {
    /// Called by `VizierTransport::send_request`: fills whatever is not set yet.
    pub fn capture(&mut self) {
        #[cfg(feature = "telemetry")]
        if self.traceparent.is_none() {
            use opentelemetry::trace::TraceContextExt;
            use tracing_opentelemetry::OpenTelemetrySpanExt;

            let cx = tracing::Span::current().context();
            let span_context = cx.span().span_context().clone();
            if span_context.is_valid() {
                self.traceparent = Some(format!(
                    "00-{}-{}-{:02x}",
                    span_context.trace_id(),
                    span_context.span_id(),
                    span_context.trace_flags().to_u8()
                ));
                let state = span_context.trace_state().header();
                if !state.is_empty() {
                    self.tracestate = Some(state);
                }
            }
        }
        self.enqueued_at_ms.get_or_insert_with(now_ms);
    }

    /// Make `span` a child of the request's sender, when there was one.
    pub fn adopt(&self, span: &tracing::Span) {
        #[cfg(feature = "telemetry")]
        if let Some(parent) = self.span_context() {
            use opentelemetry::trace::TraceContextExt;
            use tracing_opentelemetry::OpenTelemetrySpanExt;

            let _ = span.set_parent(opentelemetry::Context::new().with_remote_span_context(parent));
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = span;
    }

    /// The sender's span context, parsed back from the headers; `None` when absent or malformed.
    #[cfg(feature = "telemetry")]
    fn span_context(&self) -> Option<opentelemetry::trace::SpanContext> {
        use opentelemetry::trace::{SpanContext, SpanId, TraceFlags, TraceId, TraceState};

        let mut parts = self.traceparent.as_deref()?.split('-');
        let (version, trace_id, span_id, flags) =
            (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
        if version != "00" || parts.next().is_some() {
            return None;
        }
        let trace_id = TraceId::from_hex(trace_id).ok()?;
        let span_id = SpanId::from_hex(span_id).ok()?;
        let flags = TraceFlags::new(u8::from_str_radix(flags, 16).ok()?);
        let state = self
            .tracestate
            .as_deref()
            .and_then(|s| s.parse::<TraceState>().ok())
            .unwrap_or_default();
        let context = SpanContext::new(trace_id, span_id, flags, true, state);
        context.is_valid().then_some(context)
    }

    pub fn queue_wait_ms(&self) -> Option<u64> {
        self.enqueued_at_ms.map(|at| now_ms().saturating_sub(at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_stamps_the_queue_time_once() {
        let mut trace = RequestTrace::default();
        trace.capture();
        let first = trace.enqueued_at_ms.expect("stamped");
        trace.capture();
        assert_eq!(trace.enqueued_at_ms, Some(first));
        assert!(trace.queue_wait_ms().is_some());
    }

    #[test]
    fn the_queue_wait_counts_from_the_stamp() {
        let trace = RequestTrace { enqueued_at_ms: Some(now_ms().saturating_sub(1_500)), ..Default::default() };
        let waited = trace.queue_wait_ms().expect("a wait");
        assert!((1_500..10_000).contains(&waited), "{waited}");
        assert_eq!(RequestTrace::default().queue_wait_ms(), None);
    }

    #[test]
    fn a_trace_survives_json() {
        let trace = RequestTrace {
            traceparent: Some("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".into()),
            tracestate: Some("vendor=1".into()),
            enqueued_at_ms: Some(42),
        };
        let back: RequestTrace = serde_json::from_str(&serde_json::to_string(&trace).unwrap()).unwrap();
        assert_eq!(back, trace);
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn traceparent_headers_round_trip_into_a_span_context() {
        let trace = RequestTrace {
            traceparent: Some("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".into()),
            ..Default::default()
        };
        let context = trace.span_context().expect("parsed");
        assert_eq!(context.trace_id().to_string(), "0af7651916cd43dd8448eb211c80319c");
        assert_eq!(context.span_id().to_string(), "b7ad6b7169203331");
        assert!(context.is_sampled());
        assert!(context.is_remote());
        assert!(RequestTrace { traceparent: Some("garbage".into()), ..Default::default() }.span_context().is_none());
    }
}
