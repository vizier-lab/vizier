# Contract: `vizier-core` public API (stable tier)

What a third-party host may rely on. Pre-1.0: a minor version may break it, but every break
shows up in `cargo semver-checks` and the changelog. Items marked `#[doc(hidden)]` (the
adapter surface, research R3) are **not** part of this contract.

## Starting and stopping

```rust
use vizier_core::prelude::*;

#[tokio::main]
async fn main() -> vizier_core::Result<()> {
    let config = VizierConfig::with_workspace("/path/I/choose".into());
    let mut runtime = VizierRuntime::start(RuntimeOptions { config }).await?;

    let mut events = runtime.events();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            println!("{event:?}");
        }
    });

    match runtime.wait().await {
        RuntimeExit::Requested => {}
        RuntimeExit::SubsystemStopped { name, outcome } => {
            eprintln!("{name} stopped: {outcome}"); // the host decides what to do
        }
    }
    runtime.shutdown().await
}
```

Guarantees:

- `start` uses `config.workspace` exactly; it reads no env var or home directory to decide
  where data lives. It errors (never panics) when the workspace is in use by another process,
  when storage cannot be opened, or when the encryption key is missing or wrong (today's
  rules).
- `start` must be called inside a tokio multi-thread runtime; core spawns onto it and never
  builds its own.
- Nothing in `vizier-core` calls `std::process::exit` or installs a global logger, tracing
  subscriber, TLS provider or signal handler. The host does those.
- A subsystem stop is reported through `wait`, `events` and `health`; the host keeps
  running.

## Talking to agents

The first version of the stable tier sends requests through the transport the runtime already
uses (`runtime.transport().send_request(VizierRequest { .. })`), receiving
`VizierResponse` frames on the returned channel, and subscribes to `runtime.session_events()`.
`VizierRequest`, `VizierRequestContent::Chat`, `VizierResponse`, `VizierChannelId`,
`VizierSession` and `SessionEvent` are re-exported from the prelude. Agent creation goes
through `AgentCommand::Create` on the same transport. Phase 5 replaces these with application
services; until then this is the documented minimum for "send a message, read the reply".

## Errors

All fallible stable-tier functions return `vizier_core::Result<T>` =
`Result<T, VizierError>`. `anyhow` never appears in a stable signature.

## Features

`default = []`. Optional: `discord`, `telegram`, `media`, `docker-shell`, `documents`,
`utoipa`, `telemetry`; `full` = all. With none enabled the engine builds and runs an agent
turn on any provider that needs no optional group (including `dummyplug`). A config that
names a capability that isn't compiled in fails that agent's build (visible in `health`) or is
treated as an unknown integration kind; it never panics.
