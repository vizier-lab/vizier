//! OpenTelemetry export: traces and metrics over OTLP/HTTP, switched on only when
//! `OTEL_EXPORTER_OTLP_ENDPOINT` is set (e.g. `http://localhost:4318`, see `just jaeger`).
//! The exporters read the rest of the standard `OTEL_*` variables themselves;
//! `OTEL_METRICS_EXPORTER=none` exports traces only (Jaeger takes no metrics).

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::{SdkTracer, SdkTracerProvider};

/// Flushes and stops the exporters when dropped.
pub struct Telemetry {
    tracer: SdkTracerProvider,
    meter: Option<SdkMeterProvider>,
}

impl Telemetry {
    /// `Ok(None)` when no endpoint is configured. Runs before the subscriber exists, so a
    /// failure is returned for the caller to log.
    pub fn init() -> Result<Option<(Self, SdkTracer)>, String> {
        if std::env::var_os("OTEL_EXPORTER_OTLP_ENDPOINT").is_none() {
            return Ok(None);
        }

        let resource = Resource::builder()
            .with_service_name("vizier")
            .with_attribute(opentelemetry::KeyValue::new(
                "service.version",
                env!("CARGO_PKG_VERSION"),
            ))
            .build();

        let spans = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .build()
            .map_err(|e| format!("cannot build the OTLP span exporter: {e}"))?;
        let tracer = SdkTracerProvider::builder()
            .with_batch_exporter(spans)
            .with_resource(resource.clone())
            .build();
        let meter = match std::env::var("OTEL_METRICS_EXPORTER").as_deref() {
            Ok("none") => None,
            _ => {
                let metrics = opentelemetry_otlp::MetricExporter::builder()
                    .with_http()
                    .build()
                    .map_err(|e| format!("cannot build the OTLP metric exporter: {e}"))?;
                let meter = SdkMeterProvider::builder()
                    .with_periodic_exporter(metrics)
                    .with_resource(resource)
                    .build();
                opentelemetry::global::set_meter_provider(meter.clone());
                Some(meter)
            }
        };

        let sdk_tracer = tracer.tracer("vizier");
        Ok(Some((Self { tracer, meter }, sdk_tracer)))
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        let _ = self.tracer.shutdown();
        if let Some(meter) = &self.meter {
            let _ = meter.shutdown();
        }
    }
}

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
