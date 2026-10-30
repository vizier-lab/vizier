# vizier-http

The HTTP, WebSocket and embedded WebUI adapter for the
[vizier](https://github.com/vizier-lab/vizier) agent engine ([`vizier-core`](https://crates.io/crates/vizier-core)):
the REST API under `/api/v1`, JWT and API-key authentication, the OpenAPI document and Swagger UI,
and the WebUI, built into the crate.

Pre-1.0: a minor version may change the API.

```rust,ignore
use vizier_core::runtime::{RuntimeOptions, VizierRuntime};

let runtime = VizierRuntime::start(RuntimeOptions { config: config.clone() }).await?;
// Serve on 0.0.0.0:{port}; the future resolves only if the server stops.
vizier_http::serve(&runtime, config.channels.http.clone().unwrap()).await?;
// Or take the axum router and mount it yourself.
let app = vizier_http::router(&runtime, &http_config);
```

## The embedded WebUI

`build.rs` stages the WebUI into `$OUT_DIR/webui`, which is what the crate embeds. In a checkout of
the vizier repository the source is `webui/build/client` (built first when `webui/node_modules`
exists). In a published crate there is no `webui/`: the release workflow copies the built files into
`webui-dist/` before packaging. If neither exists the build stops with a message saying so.
