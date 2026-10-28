# Contract: Vizier Integration

## Provider identity and configuration

- Existing agents retain `provider: "dummyplug"` and their model string. No agent-row migration is required.
- The builtin dummyplug provider uses Vizier's ordinary `OpenAiChat` dialect. Its default base URL is `http://127.0.0.1:9998/v1`; `DUMMYPLUG_URL` overrides it through the ordinary provider field/env resolver. A configured `base_url` remains editable through the provider API and WebUI.
- `GET /api/v1/providers/dummyplug` and `PUT /api/v1/providers/dummyplug` follow the same read/write behavior as other configurable builtins. The providers listing includes dummyplug when configured. The old 400/404/no-list contract in `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md` must be revised in the implementation PR.
- Catalogue discovery can call `/v1/models`, but retains a static `dummyplug` model option when the optional service is offline. Agent model strings are not restricted to that listing.

## Runtime

Vizier does not construct `DummyplugModel`, start the server, or fall back to an internal model. A dummyplug turn reaches the same request encoding, HTTP transport and response decoding used by an OpenAI Chat provider. A missing server fails that turn visibly and does not stop Vizier startup or other agents. The current dialect uses whole JSON responses; server SSE serves SDK clients and is not a prerequisite for Vizier's ordinary turn.

## Development and release

- `just run` starts Vizier alone. `just dummyplug` starts the pinned server in a separate terminal. The e2e harness starts and stops it for tests.
- Compose gives Vizier `DUMMYPLUG_URL=http://dummyplug:9998/v1` and includes an explicitly started dummyplug service.
- A standalone dummyplug PR lands first. A Vizier PR pins its reviewed commit as `dummyplug/` and updates code, docs and submodule automation. Contributors run the documented local checks against the pinned version; this feature adds no GitHub Actions workflow.
- `cargo build --release`, ordinary `cargo test`, and non-dummyplug runtime do not build or require the server.
