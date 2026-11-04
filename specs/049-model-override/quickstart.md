# Quickstart: Model Override

## Preconditions

- Build/run the server with its embedded WebUI as described in `CLAUDE.md` (`just install`, then `just run` or an equivalent development server).
- Configure an agent on the offline `dummyplug` provider pointed at the standalone dummyplug service (`http://127.0.0.1:9998/v1` by default; see [dummyplug protocol](../../008-dummyplug-provider/contracts/dummyplug-protocol.md)). Keep the configured model ID distinct from the override ID so the response stats prove the selection.
- Create one test role with `model_override` and `all_agents:view`; create another with `all_agents:view` but without `model_override`. The built-in system role already receives available permissions.
- For UI verification, use the saved and user-approved FigCraft preview `C:\Users\ganes\OneDrive\Dokumen\Vizier Model Switcher Placement.figcraft` as the position reference.

## WebUI flow

1. Sign in as the permitted test user, open the agent conversation, and verify one model dropdown appears immediately to the right of the existing status-strip controls below Send. Verify the strip itself has not changed.
2. Confirm each choice shows provider ID plus full model ID, preserving model slashes, and an available provider icon where present. Verify no-icon options remain understandable by text.
3. Select a model distinct from the agent's configured model. Send `context` or another ordinary prompt. Verify the saved response stats report the selected provider/model and the existing topic URL/conversation remains unchanged.
4. Send a second prompt without changing selection; verify it uses the same selected model. Change the selection while the prior response is running; verify the prior response remains on its original model and the next prompt uses the new choice.
5. Reload/reopen the same conversation in the same browser profile; verify the preference returns. Open another topic and verify its preference is independent. Choose Agent default and verify subsequent prompts use configured model.
6. Remove the selected model from the provider's eligible list or make the provider unavailable, then submit. Verify the UI and server refuse the override and no dummyplug request arrives on the default model. Restore a valid choice to continue.
7. Sign in as the role without `model_override`; verify the selector is absent and a normal prompt still works. Attempt to submit an override directly over the authenticated topic request; verify `model_override_forbidden` and no model execution.
8. Repeat on the desktop WebUI topic path. Verify Discord and Telegram continue using their prior configured-model behavior and expose no selector.

## Protocol and service checks

- Send an authenticated topic request with an absent `model_override`: it follows the existing configured model path.
- Send an authorized request with `{ "provider": "dummyplug", "model": "<eligible-different-model>" }`: the resulting stats and usage event identify that exact pair.
- Send a request with an unauthorized user's same override: reject before creating a turn or invoking dummyplug.
- Send an authorized request with an unknown provider, unknown model, media-only provider, or stale choice: refuse without fallback. Send an override on a `client_event`, command, or non-WebUI topic channel: refuse as unsupported instead of ignoring it.
- Open a topic as a permitted user, revoke the permission, submit an override on that same socket: the next overridden request must be refused after server-side permission refresh.
- Queue a request with model A behind another turn, then resume/restart the queued/running turn in a test: the stored request still carries A. A newer message with model B must not cross-contaminate the first turn.

## Automated gates

Run the touched Rust tests first, then the CI-appropriate Rust tests and lint; run WebUI tests and typecheck:

```powershell
cargo test -p vizier-core --lib
cargo test -p vizier-http
cargo clippy -p vizier-core -p vizier-http
cd webui
npm test -- --run
npm run typecheck
```

Use the exact existing package test script if its CLI differs. Run the offline dummyplug interaction above against the real running binary: this feature changes agent-observable model dispatch, so unit tests alone do not satisfy the constitution's e2e gate. Record any pre-existing Windows test failures separately; do not report them as passing.
