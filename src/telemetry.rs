//! Tracing context the engine carries across its own channels. Exporting it (OTLP) is the
//! host's job: the server binary sets it up in `cli::telemetry_init`.

/// The trace a request belongs to, carried across the transport's channels so the turn it
/// starts joins its sender's trace. In-process only: never serialized or stored.
#[derive(Debug, Clone, Default)]
pub struct RequestTrace {
    /// The span that was current where the request was sent, if it was being traced.
    pub parent: Option<opentelemetry::trace::SpanContext>,
    /// When the request first entered an agent's queue; a re-queued request keeps it.
    pub enqueued_at: Option<std::time::Instant>,
}

impl RequestTrace {
    /// Called by `VizierTransport::send_request`: fills whatever is not set yet.
    pub fn capture(&mut self) {
        use opentelemetry::trace::TraceContextExt;
        use tracing_opentelemetry::OpenTelemetrySpanExt;

        if self.parent.is_none() {
            let cx = tracing::Span::current().context();
            let span_context = cx.span().span_context().clone();
            if span_context.is_valid() {
                self.parent = Some(span_context);
            }
        }
        self.enqueued_at.get_or_insert_with(std::time::Instant::now);
    }

    /// Make `span` a child of the request's sender, when there was one.
    pub fn adopt(&self, span: &tracing::Span) {
        use opentelemetry::trace::TraceContextExt;
        use tracing_opentelemetry::OpenTelemetrySpanExt;

        if let Some(parent) = &self.parent {
            let _ = span.set_parent(
                opentelemetry::Context::new().with_remote_span_context(parent.clone()),
            );
        }
    }

    pub fn queue_wait_ms(&self) -> Option<u64> {
        self.enqueued_at.map(|t| t.elapsed().as_millis() as u64)
    }
}
