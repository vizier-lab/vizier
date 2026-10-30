//! OpenTelemetry export for the server process: traces and metrics over OTLP/HTTP, switched
//! on only when
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
