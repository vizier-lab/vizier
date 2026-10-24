# Quickstart: OpenRouter Media Provider

**Feature**: `028-openrouter-media-provider` | **Plan**: [plan.md](./plan.md)

These steps verify the feature by hand, end to end. The dummyplug steps are the constitution's e2e gate and run **without** an OpenRouter account. Steps marked **LIVE** need a real `OPENROUTER_API_KEY` with credit and are not part of the offline gate.

## 0. Build and run

```sh
just install          # once: initialises submodules and npm packages
just run              # uses dev.vizier.yaml, WebUI on http://localhost:9999
```

For a clean workspace:

```sh
VIZIER_DATA_DIR=$(mktemp -d) just run
```

Use a single dummyplug agent for all steps: **Agents → New**, provider `dummyplug (testing)`, model `dummyplug`, save. The WebUI is the configuration surface; REST clients use the same fields.

## 1. Offline discovery from the bundled seed

This step must make **no** network request for model lists.

1. Stop vizier and restart with refresh disabled and a placeholder key so the agent can build:

   ```sh
   VIZIER_CATALOGUE_REFRESH=false OPENROUTER_API_KEY=sk-or-v1-placeholder just run
   ```

   The placeholder is only used to satisfy the provider factory; the media list is read from the embedded seed and no media call is made.

2. In the WebUI, edit the dummyplug agent → **Tools** tab.

3. **Speech-to-Text**: enable it, choose provider `openrouter`, open **Model**. Confirm:
   - a searchable list appears with readable names and ids;
   - the documented default `openai/whisper-large-v3` is marked **default**;
   - typing `openai/whisper-large-v3` filters to it.

4. **Text-to-Speech**: enable it, choose `openrouter`, open **Model**. Confirm `hexgrad/kokoro-82m` is marked default. Open **Voice** and confirm `af_alloy` is marked/available; the field also accepts a typed voice.

5. **Image Generation**: enable it, choose `openrouter`, open **Model**. Confirm `bytedance-seed/seedream-4.5` is marked default. The **Size** field stays free text (`1024x1024`).

6. Type an id that is not in the list (for example `someone/not-listed`) into the STT model search and choose **Use “someone/not-listed”**. Save the agent, reopen it, and confirm:
   - the typed id persisted exactly;
   - the other two selections persisted;
   - the agent is healthy (the picker does not require the key).

7. Select a non-default model for each capability from the list only (no typing). Save, reopen, and confirm persistence.

**Pass**: all three lists render from the seed, the defaults are marked, typed ids persist, and the agent saves/loads with refresh disabled.

## 2. API shape and permissions

While signed in, read a token from the browser (Application → Local Storage → `auth_token`) and run:

```sh
TOKEN=<token>
curl -s -H "Authorization: Bearer $TOKEN" \
  "http://localhost:9999/api/v1/catalogue/media-models?capability=speech" | python3 -m json.tool
```

Confirm:

- `data.capability == "speech"`;
- `data.source == "bundled"` (before any refresh);
- `data.refresh.enabled == false` in this run;
- exactly one model has `default: true` and it is `hexgrad/kokoro-82m`;
- `data.default_voice == "af_alloy"`;
- `voices` is a non-empty array on the default speech entry;
- the response contains no credential.

Invalid capability:

```sh
curl -s -o /dev/null -w '%{http_code}\n' -H "Authorization: Bearer $TOKEN" \
  "http://localhost:9999/api/v1/catalogue/media-models?capability=video"
# 400
```

Manual refresh in this run:

```sh
curl -s -o /dev/null -w '%{http_code}\n' -X POST -H "Authorization: Bearer $TOKEN" \
  "http://localhost:9999/api/v1/catalogue/media-models/speech/refresh"
# 409 (catalogue.refresh is false)
```

A user without `settings:providers` gets `403` from the refresh and can still read the list.

## 3. Refresh online (no OpenRouter credential needed)

1. Restart with refresh on and the placeholder key:

   ```sh
   OPENROUTER_API_KEY=sk-or-v1-placeholder just run
   ```

2. Open the speech model picker and click **Refresh** (or `POST .../speech/refresh`). Confirm:
   - the response/status becomes `source: "cached"` and `fetched_at` moves forward;
   - the count matches OpenRouter's live speech list (36 on 2026-10-24);
   - the default is still marked;
   - the agent's current selection did not change.

3. Repeat for `transcription` and `image`; the image entries expose `aspect_ratios`/`resolutions`.

4. Restart vizier. Reopen the picker **without** clicking refresh. Confirm `source: "cached"` and the same list (the snapshot survived).

## 4. Refresh failure keeps the last list

This step needs the network to be unavailable for `openrouter.ai` (disconnect, firewall, or a temporary hosts entry). Do not remove the local loopback.

1. With a cached list from §3, make `openrouter.ai` unreachable.
2. Reload the WebUI and reopen the agent editor: the last successful list must still appear from SQLite (the picker's status shows `cached`), with no network.
3. Click **Refresh** in the picker.
4. Confirm:
   - the list is still shown;
   - a non-blocking notice shows the failure reason;
   - saving the agent still works;
   - the last-known list remains usable. (Do not expect a successful media call in this run: the placeholder key is invalid, so a call would fail with 401 rather than with the list failure.)
5. Restore the network. The next refresh succeeds and clears the notice.

## 5. Missing credential is actionable

1. Stop vizier. Unset the key and make sure no `openrouter` provider is saved in Settings › Providers:

   ```sh
   unset OPENROUTER_API_KEY
   just run
   ```

2. Configure a **new** dummyplug agent with OpenRouter for speech-to-text (or TTS/image), no model, and save.
   The save is **refused** with `400` and a message naming `openrouter` and pointing to **Settings › Providers** (or `OPENROUTER_API_KEY`); nothing is persisted, and there is no silently unlistening agent.

3. To see the failed-health path, create the same agent while the key is present (a placeholder is enough), then remove the key and restart vizier. Confirm the agent reports `state: failed` with
   `speech-to-text is enabled but cannot start: no API key found for provider "openrouter": add one in Settings › Providers or set OPENROUTER_API_KEY`
   (the wording depends on which capability is enabled). It must **not** silently run without listening/speaking/drawing.

4. Save the key in Settings › Providers. Confirm the agent recovers without a special media restart step.

## 6. LIVE — real artifacts

**Requires a valid `OPENROUTER_API_KEY` with credit. Skip on CI.**

Use the same dummyplug agent and leave every override empty for the first pass.

### Speech-to-text

1. In the agent's WebUI topic, attach a short spoken clip (2–5 s, `.mp3` or `.wav`).
2. The model calls media tools from inside `execute_python`. Send the JSON to run the call:

   ```json
   {"tool":"execute_python","arguments":{"intent":"transcribe the attached clip","code":"result = stt_transcribe(filename=\"<the uploaded filename>\")\nresult"}}
   ```

3. Expect a transcript that matches the clip. Repeat with `language="en"` and an explicit model such as `openai/whisper-large-v3`.
4. Send a short voice message through a configured Discord/Telegram channel (or the WebUI's audio path) with a live chat model. Confirm the transcript reaches the agent and the reply reflects what was said; this is the channel path, not just the tool.

### Text-to-speech

1. Run the speech tool:

   ```json
   {"tool":"execute_python","arguments":{"intent":"speak a short sentence","code":"result = tts_generate(text=\"Vizier is speaking through OpenRouter.\")\nresult"}}
   ```

2. Expect a new `.wav` session file that plays in the WebUI; download it and play it in a standard player.
3. Repeat with `tts_settings.voice` set to another published Kokoro voice and `model` set to a non-default speech model that publishes voices; the result must still play. Then set `speed` to `1.25`: if the chosen model supports speed, the result still plays; if it does not, confirm the provider's explanation is surfaced and the agent stays healthy (FR-010), then repeat with speed empty or `1.0`.

### Image generation

1. Run the image tool:

   ```json
   {"tool":"execute_python","arguments":{"intent":"draw a red panda astronaut","code":"result = image_generate(prompt=\"a small red panda astronaut, studio lighting\")\nresult"}}
   ```

2. Expect an image session file. Confirm:
   - the file opens in a standard viewer;
   - its stored MIME and extension agree (PNG in the common case; use a Recraft vector model to see `image/svg+xml`);
   - `send_attachment` with the filename delivers it in the topic.
3. Repeat with `size="1792x1024"` and an explicit model; OpenRouter may return its nearest supported size, but the file must still open.

### Provider failure

1. Set the STT model to `someone/not-a-model` (credential present) and transcribe the clip.
2. Confirm the tool result includes OpenRouter's explanation, the agent remains healthy, and a normal text message still gets a dummyplug reply.

## 7. Regression

- An agent using OpenAI/Gemini/Whisper for the same capabilities still builds and behaves as before; the OpenAI TTS voice remains `alloy`.
- The Settings › Providers **media** filter still lists OpenRouter and it can be connected with the one key.
- The WebUI lists all three OpenRouter capabilities with their documented defaults.
- `GET /api/v1/catalogue/providers?kind=media` includes `openrouter`.

## 8. Gates

```sh
cargo clippy
cargo test
cd webui && npm test && npm run typecheck
```

Expected: all pass. The Rust tests cover capability/seed/list parsing, default/credential values, image path/body/media-type, speech speed omission, MIME→extension and storage round-trips; the WebUI tests cover the default maps and picker helpers.

## 9. Cleanup

```sh
# stop vizier
just shutdown
```

Remove the temporary `OPENROUTER_API_KEY` placeholder from the shell; do not commit a real key. If a temporary hosts entry was added for §4, remove it.

## 10. Validation record (2026-10-25, offline environment)

Run against `./target/debug/vizier` with temp workspaces; no live `OPENROUTER_API_KEY` (a
placeholder was used where a key had to resolve, and no key at all for §5).

| Step | Result |
|---|---|
| §1 offline seed | All three capabilities listed from the embedded seed (speech 36, transcription 28, image 61), defaults marked, `default_voice: "af_alloy"`, 54 Kokoro voices including `af_alloy`, 0 media-list network requests |
| §2 API shape | `source: "bundled"`, one `default: true` per capability, invalid capability → `400`, refresh with `catalogue.refresh: false` → `409`, unauthenticated refresh → `401` |
| §3 refresh online | `POST …/speech/refresh` → `200`, `source: "cached"`, new `fetched_at`, structure preserved; same for transcription (28) and image (61, with `resolutions`/`aspect_ratios`) |
| §3 persistence | Restart serves `source: "cached"` with the same `fetched_at`, no network |
| §4 refresh failure | With `HTTPS_PROXY` pointed at a closed port: `502` with the connection reason, the cached 61-model list stays, `refresh.last_error`/`failed_at` recorded, no automatic retry within the 1 h backoff, notice restored after restart, later success clears it |
| §5 missing credential | A new save is refused `400` with `no API key found for provider "openrouter": add one in Settings › Providers or set OPENROUTER_API_KEY`; an existing agent becomes `state: failed` with the same hint; saving the key recovers it to `running` |
| Dummyplug tools | `list_tool_functions` shows `image_generate` (media) and `stt_transcribe`/`tts_generate` (audio); each real call reached OpenRouter's endpoint and surfaced `401 … User not found.` as a tool error while the agent stayed `running` with 0 failures and answered the next turn |
| §7 regression | An OpenAI TTS + Whisper STT agent builds and runs; OpenAI TTS still defaults to `alloy`; `GET /catalogue/providers?kind=media` lists `openrouter` |
| Typed ids | `someone/not-listed` (STT), a non-default speech model with `voice_3150`, and a non-default image model with `size: 2K` all persisted exactly across save and reopen |
| SC-011 | Refresh disabled: 0 startup/build/turn media-list requests. Refresh enabled: 0 at startup with a cache; fetches happened only on the manual refresh |

Not run here (marked **LIVE** in §6, needs a funded key): a real transcript, playable audio, a
generated image, and their MIME/extension at the provider. The pure mappings behind them are
covered by `cargo test` (`the_speech_body_omits_the_neutral_speed`,
`a_base64_image_keeps_the_media_type_it_reports`,
`the_default_extension_follows_the_returned_media_type`) and the payload shapes were checked
against live OpenRouter responses during planning and seed generation.
