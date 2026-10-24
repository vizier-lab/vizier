# Feature Specification: OpenRouter Media Provider

**Feature Branch**: `028-openrouter-media-provider`

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "let's add openrouter as tts, transcribe/stt, and image generation provider"

## Context

Vizier lets an operator connect each model provider once and then choose a provider per media capability on an agent: text-to-speech, speech-to-text, and image generation. Today OpenRouter is available for chat, embeddings and text-to-speech, but it is missing from the speech-to-text and image-generation provider choices. An operator who already pays for OpenRouter — and has its key saved once in Settings › Providers — cannot point an agent's listening or drawing at it, and must connect a second vendor for those two features.

This feature makes OpenRouter a first-class media provider across all three capabilities. The value is one credential and one bill for chat, hearing, speaking and drawing: selecting OpenRouter in any of the three media sections of an agent's configuration is enough, with a sensible default model so that no other field is mandatory. The existing per-capability settings (model, voice, speed, language, size) keep working unchanged and are passed through to OpenRouter.

OpenRouter's text-to-speech support already exists in Vizier; this feature treats it as part of the same promise — it must be selectable, documented and exercised alongside the two new capabilities, and any gap found in it is closed here rather than deferred (a gap already identified: a person had to know a model id from memory, because the media model fields are free text). The same feature therefore gives OpenRouter's media models the catalogue treatment chat models already have: the provider's current speech, transcription and image models are listed per capability, with readable names, supported voices for speech, and a documented default. A typed model id remains as a fallback, so a stale or unreachable list never blocks configuration. Only the three media capabilities are in scope. Chat, embeddings, read-image/vision, usage accounting for media calls, image-to-image reference images, per-capability credentials, and model discovery for media providers other than OpenRouter are not.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Voice messages are transcribed through OpenRouter (Priority: P1)

A person with an OpenRouter key configured in Settings › Providers creates or edits an agent, turns on speech-to-text, chooses OpenRouter and leaves the model empty. They send the agent a voice message from Discord. The agent receives the transcript as if the person had typed it, and can respond to what was said.

**Why this priority**: Listening is the capability OpenRouter support is most visibly missing today: without it, a person cannot use the vendor they already pay for to understand voice messages. It also exercises the most paths (chat attachment, channel voice note and the transcription tool).

**Independent Test**: With only an OpenRouter credential saved, enable speech-to-text on an agent with provider OpenRouter and no model, then transcribe a short spoken clip through the agent's transcription tool and through a voice message; both must yield readable text and the agent's reply must reflect the content. Repeat with an explicit model and language and confirm both are accepted.

**Acceptance Scenarios**:

1. **Given** an agent with speech-to-text enabled for OpenRouter and no model set, **When** a voice message arrives from a supported channel, **Then** the agent receives a transcript of that audio and answers it.
2. **Given** speech-to-text is enabled for OpenRouter, **When** a person asks the agent to transcribe an audio file in the conversation, **Then** the transcription tool returns the spoken text.
3. **Given** the OpenRouter settings include an explicit model and language, **When** audio is transcribed, **Then** the transcript is produced with that model and language rather than the defaults.
4. **Given** OpenRouter is selected for speech-to-text but no OpenRouter credential exists, **When** the agent is started, **Then** it reports an actionable error naming OpenRouter and where to add the key, and does not silently disable listening.
5. **Given** OpenRouter is selected for speech-to-text, **When** the provider rejects the audio (unsupported format, oversized clip, exhausted credits or an unknown model), **Then** the tool call fails with the provider's explanation and the agent stays available for other work.

---

### User Story 2 - Images are generated through OpenRouter (Priority: P1)

A person enables image generation on an agent, chooses OpenRouter and asks for a picture. The agent generates the image and sends it in the conversation, where it can be opened and downloaded like any other generated image.

**Why this priority**: Drawing is the other capability OpenRouter support is missing, and it is independently valuable: a person can stop paying for a separate image vendor. It is separate from listening and can ship on its own.

**Independent Test**: With only an OpenRouter credential saved, enable image generation with provider OpenRouter and no model, ask the agent for an image, and confirm a valid image file appears in the conversation's files and is displayed. Repeat with an explicit model and a size, and confirm the returned image still opens.

**Acceptance Scenarios**:

1. **Given** an agent with image generation enabled for OpenRouter and no model set, **When** the agent is asked to draw something, **Then** it produces a valid image, saves it in the conversation's files and delivers it as an attachment.
2. **Given** an explicit model and size are configured, **When** an image is generated, **Then** that model is used and the size request is honored where the chosen model supports it.
3. **Given** OpenRouter returns an image in a format other than PNG, **When** it is saved, **Then** the file keeps the correct format and extension and opens in a standard image viewer.
4. **Given** OpenRouter is selected for image generation but no OpenRouter credential exists, **When** the agent is started, **Then** it reports an actionable error naming OpenRouter and where to add the key.
5. **Given** the requested model does not support image generation or the account lacks credits, **When** generation is attempted, **Then** the tool call fails with the provider's explanation and the agent remains available.

---

### User Story 3 - Replies are spoken through OpenRouter (Priority: P2)

A person enables text-to-speech on an agent and chooses OpenRouter, keeping or changing the voice and speed. When the agent replies with audio, the voice message is delivered on the conversation's channel.

**Why this priority**: The code path already exists; the work is to confirm it end to end, make its defaults reliable, and document it so it is not a hidden, untested capability. It ranks below the two new capabilities because no new connection is being added.

**Independent Test**: With only an OpenRouter credential saved, enable text-to-speech for OpenRouter and generate speech from a short sentence through the speech tool; the audio must play. Repeat with an explicit voice, model and speed.

**Acceptance Scenarios**:

1. **Given** an agent with text-to-speech enabled for OpenRouter and no model or voice set, **When** it speaks a reply, **Then** a playable audio attachment is produced and delivered.
2. **Given** an explicit model, voice and speed are configured, **When** speech is generated, **Then** those settings are used and the result is still playable.
3. **Given** the provider rejects the voice, model or speed, **When** speech is attempted, **Then** the tool call fails with the provider's explanation and the agent remains available.
4. **Given** OpenRouter is selected for text-to-speech but no OpenRouter credential exists, **When** the agent is started, **Then** it reports an actionable error naming OpenRouter and where to add the key.

---

### User Story 4 - OpenRouter media models are discoverable and selectable (Priority: P2)

A person editing an agent chooses OpenRouter for speech-to-text and opens the model field. Instead of an empty box expecting a raw identifier, they see a searchable list of OpenRouter's current transcription models with readable names and the documented default marked. They pick one without leaving the form. For text-to-speech the list holds speech models, and choosing one offers the voices that model supports. For image generation the list holds image models. A model that is not listed yet can still be typed by its identifier.

**Why this priority**: A supported capability that cannot be found or configured is not usable, and OpenRouter's media model identifiers are long and change over time. This is independent of the provider mechanics and can be verified by inspection.

**Independent Test**: In the WebUI, open an agent's Tools section with OpenRouter selected for each capability and confirm a searchable list of that capability's models appears with the default marked. Choose a non-default model, save, reopen, and confirm it persisted. Type an id that is not in the list and confirm it is accepted. Then disconnect the network, reopen the editor, and confirm the last-known list still appears and an agent can still be saved and used.

**Acceptance Scenarios**:

1. **Given** a person chooses OpenRouter for a media capability, **When** the model field is opened, **Then** it lists models valid for that capability with their readable names and identifiers, and the documented default is marked.
2. **Given** the OpenRouter media list was fetched recently, **When** the picker is opened again within the refresh interval, **Then** it is served immediately from the stored list without waiting on the network.
3. **Given** the refresh interval has passed, **When** a person opens the picker or asks to refresh, **Then** the list is refreshed in the background while the current list stays usable.
4. **Given** the network is unreachable or OpenRouter refuses the model list, **When** the picker is opened, **Then** the last successful list (or the bundled seed on a fresh install) is shown with a non-blocking notice, and saving and using the agent still work.
5. **Given** a person selects a text-to-speech model, **When** the voice field is shown, **Then** voices the model publishes are offered, while a typed voice remains accepted; when the model publishes no voices, the previous free-text behavior remains.
6. **Given** a model is missing from the list, **When** the person types its identifier, **Then** it is accepted and stored exactly as typed, like a listed model.
7. **Given** a media capability is set to OpenRouter with no model chosen, **Then** the documented default is what the field shows as its effective value and what the agent uses.
8. **Given** a person opens Settings › Providers and filters to media-capable providers, **Then** OpenRouter is listed and can be connected with one key.

---

### Edge Cases

- The OpenRouter key is removed or rejected while an agent that uses it for media is running: the agent restarts and reports a failed health state with the credential error, rather than failing silently or serving a stale capability.
- The chosen model exists but is not an audio or image model: the provider's refusal is surfaced; the agent must not crash or lose the turn's other results.
- A requested image size is not supported by the chosen model: the request either succeeds at the provider's nearest supported size or fails with an explanation; the saved file always opens correctly.
- Audio is empty, corrupt, in an unsupported format, or too large for the provider's accepted upload path: transcription fails with a clear explanation and no partial transcript is presented as real.
- The transcript is empty (silence): the agent receives an empty or explicitly empty transcript rather than an error that hides the cause.
- OpenRouter returns an image as JPEG, WebP or SVG: the artifact is stored with a matching type and name, and sending it does not mislabel it as PNG.
- The provider is rate-limited or times out during a turn: the tool call reports the failure and the agent can continue or retry later.
- An agent uses OpenRouter for more than one media capability: one saved OpenRouter credential serves all of them, and changing the key takes effect for all of them together.
- An existing agent configured for another media provider keeps working untouched after upgrade.
- A listed model is retired upstream after the stored list was fetched: it keeps being offered until the next refresh, and attempting to use it surfaces the provider's error rather than a silent fallback to a different model.
- The stored list is empty, absent or unreadable and the network is down: the image, speech and transcription fields still accept a typed identifier, and the documented default still applies when the field is left empty.
- A refresh is running while a person edits the form: their current selection is never changed or cleared by the refreshed list, even if the selected model disappeared upstream.
- A model appears in more than one OpenRouter capability list: each picker offers only the models valid for its own capability.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: OpenRouter MUST be selectable as the provider for text-to-speech, speech-to-text and image generation in an agent's tools configuration, through both the HTTP API and the WebUI.
- **FR-002**: OpenRouter media features MUST use the OpenRouter credential saved in Settings › Providers (or the OpenRouter environment variable), the same credential chat and embeddings use. No separate media credential or account MUST be required.
- **FR-003**: Speech-to-text through OpenRouter MUST return the transcript text to every existing consumer: the agent's transcription tool, voice messages from channels, and audio sent in chat.
- **FR-004**: Speech-to-text MUST accept an optional model override and an optional language override and pass both to the provider; when omitted it MUST use a documented default that works without further configuration.
- **FR-005**: Image generation through OpenRouter MUST accept a prompt, an optional model override and an optional size, and MUST produce the image in the conversation's session files.
- **FR-006**: A generated image MUST be stored and delivered with the media type the provider actually returned (PNG, JPEG, WebP or SVG), including a matching default filename extension, so it opens correctly and channel renderers do not treat it as a broken PNG.
- **FR-007**: Text-to-speech through OpenRouter MUST accept an optional model, voice and speed, and MUST produce audio that plays in the WebUI and as a channel attachment.
- **FR-008**: Each OpenRouter media capability MUST have a documented default model, and text-to-speech a documented default voice, so enabling the capability and leaving every override empty still produces a valid artifact.
- **FR-009**: When the OpenRouter credential is missing, the failure MUST name OpenRouter, state that the credential is needed for the feature, and tell the person where to add it or which environment variable to set.
- **FR-010**: Provider-side failures (unknown model, unsupported parameter, rejected media, insufficient credits, rate limit, timeout) MUST be surfaced as tool errors that include the provider's explanation and MUST NOT crash the agent, abort unrelated tool work in the same turn, or lose already produced results.
- **FR-011**: OpenRouter MUST appear in the WebUI's media-capable provider list and in the provider drop-downs for all three media capabilities; each drop-down MUST display the default that applies when its override field is empty.
- **FR-012**: Saving or clearing the OpenRouter credential MUST take effect for media capabilities the same way it does for chat today (the agents that use the provider pick up the change), without a separate media-specific restart step.
- **FR-013**: Existing agents and requests that do not select OpenRouter for media MUST behave exactly as before, and existing stored agent configurations MUST remain valid without migration.
- **FR-014**: The documentation that lists media providers and their defaults MUST include OpenRouter for all three capabilities, with its default model/voice and the shared credential it uses.
- **FR-015**: Generated audio and images MUST follow the same conversation-file lifecycle as the existing generated media: visible in the conversation, downloadable, and subject to the same retention and deletion rules.
- **FR-016**: When OpenRouter is selected for a media capability, the model field MUST offer a list of that capability's models published by OpenRouter, each with its identifier and a human-readable name, and MUST mark the documented default.
- **FR-017**: The list MUST be specific to the capability: speech models for text-to-speech, transcription models for speech-to-text, and image models for image generation, with no model from another capability offered.
- **FR-018**: The list MUST be fetched from OpenRouter only when a person opens or explicitly refreshes it; startup, agent builds, turns, scheduled tasks and the dream cycle MUST NOT fetch it.
- **FR-019**: A successful fetch MUST be stored persistently and reused across restarts, and a person-triggered refresh MUST be available. Refreshing MUST follow the same interval, failure backoff and manual-refresh rules as the existing provider and model catalogue.
- **FR-020**: A bundled seed of known OpenRouter media models MUST ship with the build, so a fresh install or an offline install still offers choices. A failed refresh MUST keep the last successful list or the seed usable and MUST show a non-blocking notice with the failure reason.
- **FR-021**: The model picker MUST be searchable by name and identifier, MUST accept an identifier that is not in the list, and MUST store and use a typed identifier exactly as entered.
- **FR-022**: For text-to-speech, the voice field MUST offer the voices the selected model publishes where OpenRouter publishes them, while continuing to accept a typed voice; an empty voice field MUST fall back to the documented default.
- **FR-023**: Refreshing the list MUST NOT change, clear or invalidate the model, voice, language or size already selected in an open form, even when the selected model is absent from the refreshed list.
- **FR-024**: Refresh failures and stale-list state MUST NOT prevent saving an agent or making a media call with the last-known or typed model.
- **FR-025**: The lists and refresh status MUST be available through the same API surface the WebUI uses, and access to them MUST follow the existing permissions for viewing or editing the agent and for provider settings.

### Key Entities

- **Agent media settings**: per agent, one setting group per capability. Text-to-speech holds provider, optional model, voice and speed; speech-to-text holds provider, optional model and language; image generation holds provider, optional model and size. The provider value must accept OpenRouter in all three.
- **OpenRouter credentials**: one provider credential entry shared by chat, embeddings and all three media capabilities, resolved from the saved provider settings first and then from the provider's environment variable.
- **Generated media artifact**: an audio or image file attached to a conversation's session files, with a filename, media type and size, produced by the capability and delivered to the channel.
- **Provider default**: the model (and default voice for speech) used when the agent has not set an override, documented per capability.
- **Media model listing**: one entry in the discoverable list for a capability: model identifier, readable name, capability it belongs to, and published metadata relevant to that capability (voices for speech, supported sizing for images) plus whether it is the documented default.
- **Media model snapshot**: the last successfully fetched per-capability model lists, with when they were fetched and the last failure if a later check failed. It may be absent, in which case the bundled seed stands in.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With a valid OpenRouter key, a person can enable each of the three media capabilities by choosing only the OpenRouter provider and leaving every override empty; all three produce a usable artifact (image opens, audio plays, transcript is readable) in 100% of verification runs.
- **SC-002**: After the OpenRouter key is saved, configuring all three capabilities takes under 2 minutes and requires no credential entry beyond the one key.
- **SC-003**: The number of separate credentials needed to use OpenRouter for chat, embeddings, listening, speaking and drawing is exactly one.
- **SC-004**: In 100% of missing-credential trials, the error names OpenRouter and points to the place or variable that supplies the key; no trial results in silent non-function or an unexplained failure.
- **SC-005**: Every existing media capability and provider continues to pass its current checks after the change; no existing agent configuration changes behaviour.
- **SC-006**: 100% of generated images open in a standard image viewer and 100% of generated audio plays, including when the override fields are left empty.
- **SC-007**: In 100% of injected provider failures (unknown model, rejected parameter, exhausted credits, rate limit, timeout), the person or agent sees the provider's explanation and the agent remains healthy and able to run another turn.
- **SC-008**: A person can find and select OpenRouter in all three media sections and in the media filter of Settings › Providers on the first attempt, without consulting external documentation.
- **SC-009**: A person can choose a non-default model for each of the three capabilities entirely from the offered list, without typing an identifier, in under 1 minute per capability.
- **SC-010**: After a refresh, models OpenRouter has added or retired are reflected in the pickers without upgrading the binary; with the network unavailable, the last-known or bundled list remains usable in 100% of checks.
- **SC-011**: Zero automatic network requests are made for the media model list during startup, agent builds, turns, scheduled tasks or the dream cycle; every fetch is traceable to a person opening or refreshing the list.
- **SC-012**: In 100% of refresh-failure trials, saving an agent and producing media still succeed using the last-known, bundled or typed model.
- **SC-013**: For 100% of speech models whose voices OpenRouter publishes, the offered voice choices match the published set for the selected model.

## Assumptions

- OpenRouter text-to-speech support already exists in Vizier through the shared OpenAI-compatible speech path. This feature verifies it end to end, fixes any gaps found (defaults, error text, documentation, model discovery), and keeps it consistent with the two new capabilities rather than redesigning it.
- OpenRouter offers all three media capabilities under its public API, with its own accepted audio formats, size limits, output formats and error responses. The exact request and response shapes are implementation concerns for the planning phase.
- OpenRouter publishes per-capability model lists (speech, transcription and image) with model identifiers and names, and publishes supported voices for speech models. Those lists are readable without a credential; if that changes, the saved OpenRouter credential is used, and a missing credential still leaves the bundled seed usable.
- Defaults confirmed against OpenRouter's live model list during planning (2026-10-24) and recorded in the documentation: text-to-speech `hexgrad/kokoro-82m`, speech-to-text `openai/whisper-large-v3`, image generation `bytedance-seed/seedream-4.5`. The previous text-to-speech default `openai/gpt-4o-mini-tts-2025-12-15` is no longer in the live list, so it is replaced here. If any of these is retired or unavailable at implementation time, the nearest stable OpenRouter model of the same modality is substituted and the documentation updated in the same change.
- The bundled media model seed lists known OpenRouter models per capability at release time. It may be incomplete and may list a model that has since been retired; the live list and the provider's response are authoritative when available.
- The refresh rules mirror the existing provider catalogue: a lazy check only when a person reads the list, a configurable interval with the same default, a failure backoff, a manual refresh, and no startup, agent, scheduler or dream traffic.
- The default voice for OpenRouter text-to-speech is `af_alloy`, a published voice of `hexgrad/kokoro-82m`; the old `alloy` voice belongs to OpenAI and is not valid for the new default model. If the default model changes, the voice is re-checked and updated in the same change.
- Language overrides follow the existing convention (a short language code such as `en`); when the provider cannot auto-detect or the code is unsupported, the provider's error is surfaced rather than a guessed language.
- The OpenRouter account has credit available. Free or promotional models may appear, disappear or behave differently; selecting one is the operator's choice and its provider errors follow FR-010.
- Media calls remain excluded from Vizier's token usage accounting, exactly as the existing speech, transcription and image capabilities are. Adding cost tracking for media is a separate concern.
- Model prices and other commercial metadata may be displayed alongside a model when OpenRouter publishes them, but selection and billing stay with OpenRouter; this feature does not route between upstream providers.
- Image-to-image generation with reference images, streaming partial images, and multiple images per request are out of scope; the existing single-prompt, single-result contract of the image tool is preserved.
- Per-capability credentials, separate OpenRouter accounts, and provider-specific routing preferences (choosing upstream sub-providers) are out of scope.
- Model discovery is OpenRouter-only in this feature. The other media providers keep their current free-text model fields; generalizing discovery to them is a possible follow-up.
- The WebUI is the configuration surface under test; API clients use the same fields and receive the same accepted provider values, and the same discoverable model lists and refresh status.
