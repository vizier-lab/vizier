# vizier-core

The [vizier](https://github.com/vizier-lab/vizier) agent engine as a library: agents with their
own tools, memory and model provider, a scheduler, and the storage underneath, with no web
framework, command-line parser or terminal UI. The `vizier` server is this crate plus
[`vizier-http`](https://crates.io/crates/vizier-http) and a command-line binary.

Pre-1.0: a minor version may change the API.

```sh
cargo add vizier-core
```

## Quick start

```rust,no_run
use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeExit, RuntimeOptions, VizierRuntime},
};

#[tokio::main]
async fn main() -> vizier_core::Result<()> {
    // You choose where the data lives; the engine reads no environment variable to decide.
    let config = VizierConfig::with_workspace("/path/I/choose".into());
    let mut runtime = VizierRuntime::start(RuntimeOptions { config }).await?;

    match runtime.wait().await {
        RuntimeExit::Requested => {}
        RuntimeExit::SubsystemStopped { name, outcome } => {
            eprintln!("{name} stopped: {outcome}"); // the host decides what to do
        }
    }
    runtime.shutdown().await
}
```

`examples/headless.rs` goes further: it creates an agent on the offline `dummyplug` test model,
sends it a message and prints the reply and the session events.

- `VizierRuntime::start` must run inside a tokio multi-thread runtime; it spawns onto it and never
  builds its own.
- The engine never exits the process, installs a logger or a TLS provider. A subsystem that stops
  is reported through `wait`, `events` and `health`.
- One engine per workspace: a second `start` on a workspace in use fails.

## Stable API and the rest

Documented and checked for breaking changes: `runtime` (`VizierRuntime` and its options, events and
health), `config`, `schema` (the request and response types), `auth`, `error`, `transport` and
`access`. The other public modules are `#[doc(hidden)]`: they are public only because
[`vizier-http`](https://crates.io/crates/vizier-http) and the `vizier` binary are built on them, they
are not part of the API, and application services will replace them.

## Features

`default = []`. With none enabled the engine builds and runs an agent turn on any provider that
needs no optional group.

| Feature | Adds |
|---|---|
| `discord` | the Discord channel reader and tools |
| `telegram` | the Telegram channel reader and tools |
| `media` | local speech recognition (whisper) and synthesis (kokoro), MP3 decoding |
| `docker-shell` | the Docker shell backend |
| `documents` | PDF text extraction and the Office addon (Word and Excel) |
| `utoipa` | OpenAPI schema derives on the engine's types |
| `telemetry` | OpenTelemetry spans and metrics for model calls |
| `full` | all of the above |

A configuration that names a capability that is not compiled in fails that agent's build (visible
in `health`) or is treated as an unknown integration kind; it never panics.
