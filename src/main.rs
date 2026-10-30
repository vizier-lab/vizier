//! The `vizier` binary: process setup (env file, logging, telemetry export, TLS provider)
//! around `vizier::cli::start`, and the exit code.

use std::process;

use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use vizier::{Result, cli, cli::telemetry_init::Telemetry};

fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let filter = match std::env::var("RUST_LOG") {
        Ok(_) => EnvFilter::from_default_env(),
        Err(_) => default_filter(),
    };
    let (telemetry, telemetry_error) = match Telemetry::init() {
        Ok(t) => (t, None),
        Err(e) => (None, Some(e)),
    };
    let (telemetry, otel_layer) = match telemetry {
        Some((guard, tracer)) => (
            Some(guard),
            Some(tracing_opentelemetry::layer().with_tracer(tracer)),
        ),
        None => (None, None),
    };
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().compact())
        .with(otel_layer)
        .init();
    if let Some(e) = telemetry_error {
        tracing::warn!("{e}; telemetry is off");
    }

    // Both `ring` (ureq, bollard) and `aws-lc-rs` (reqwest) are compiled into
    // rustls, so it cannot pick a process default on its own; twilight builds
    // its TLS config from that default and would panic without one.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let exit_code = match cli::start() {
        Ok(()) => 0,
        Err(err) => {
            tracing::error!("{}", err);
            1
        }
    };
    // `process::exit` skips destructors: flush the exporters first.
    drop(telemetry);
    process::exit(exit_code);
}

fn default_filter() -> EnvFilter {
    EnvFilter::new("vizier=debug")
        .add_directive("twilight_gateway=error".parse().unwrap())
        .add_directive("twilight_http=error".parse().unwrap())
        .add_directive("sqlx=error".parse().unwrap())
        .add_directive("reqwest=error".parse().unwrap())
        .add_directive("hyper=error".parse().unwrap())
        .add_directive("tungstenite=error".parse().unwrap())
        .add_directive("h2=error".parse().unwrap())
        .add_directive("tracing=off".parse().unwrap())
        .add_directive("rustls=off".parse().unwrap())
        .add_directive("sqlite=off".parse().unwrap())
        .add_directive("ort=off".parse().unwrap())
        .add_directive("ureq=off".parse().unwrap())
        .add_directive("bollard=off".parse().unwrap())
        .add_directive("rmcp=off".parse().unwrap())
}
