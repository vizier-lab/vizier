# Feature Specification: OpenRouter Media Provider

**Feature Branch**: `028-openrouter-media-provider`

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "let's add openrouter as tts, transcribe/stt, and image generation provider"

## Context

Vizier lets an operator connect each model provider once and then choose a provider per media capability on an agent: text-to-speech, speech-to-text, and image generation. Today OpenRouter is available for chat, embeddings and text-to-speech, but it is missing from the speech-to-text and image-generation provider choices. An operator who already pays for OpenRouter — and has its key saved once in Settings › Providers — cannot point an agent's listening or drawing at it, and must connect a second vendor for those two features.

This feature makes OpenRouter a first-class media provider across all three capabilities. The value is one credential and one bill for chat, hearing, speaking and drawing: selecting OpenRouter in any of the three media sections of an agent's configuration is enough, with a sensible default model so that no other field is mandatory. The existing per-capability settings (model, voice, speed, language, size) keep working unchanged and are passed through to OpenRouter.

OpenRouter's text-to-speech support already exists in Vizier; this feature treats it as part of the same promise — it must be selectable, documented and exercised alongside the two new capabilities, and any gap found in it is closed here rather than deferred. Only the three media capabilities are in scope. Chat, embeddings, read-image/vision, usage accounting for media calls, image-to-image reference images, and per-capability credentials are not.

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

### User Story 4 - Choosing OpenRouter media features is obvious in the WebUI (Priority: P2)

A person editing an agent opens the Tools section and sees OpenRouter in the provider lists for text-to-speech, speech-to-text and image generation. Each list shows the default model or voice the agent will use when the field is left empty. Settings › Providers places OpenRouter among the media-capable providers so its key is easy to connect.

**Why this priority**: A supported capability that cannot be found or configured is not usable. This is independent of the provider mechanics and can be verified by inspection.

**Independent Test**: In the WebUI, open an agent's Tools section and confirm OpenRouter is selectable in all three media provider lists, that default placeholders appear when model or voice is empty, and that OpenRouter appears under the media filter in Settings › Providers.

**Acceptance Scenarios**:

1. **Given** a person is editing an agent, **When** they open the text-to-speech, speech-to-text or image-generation section, **Then** OpenRouter is offered as a provider in each.
2. **Given** OpenRouter is selected and the model field is empty, **When** the field is displayed, **Then** it shows the default model that will be used.
3. **Given** a person opens Settings › Providers and filters to media-capable providers, **Then** OpenRouter is listed and can be connected with one key.
4. **Given** an agent is saved with OpenRouter for a media capability, **When** the agent is reopened, **Then** the same provider and any overrides are shown.

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

### Key Entities

- **Agent media settings**: per agent, one setting group per capability. Text-to-speech holds provider, optional model, voice and speed; speech-to-text holds provider, optional model and language; image generation holds provider, optional model and size. The provider value must accept OpenRouter in all three.
- **OpenRouter credentials**: one provider credential entry shared by chat, embeddings and all three media capabilities, resolved from the saved provider settings first and then from the provider's environment variable.
- **Generated media artifact**: an audio or image file attached to a conversation's session files, with a filename, media type and size, produced by the capability and delivered to the channel.
- **Provider default**: the model (and default voice for speech) used when the agent has not set an override, documented per capability.

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

## Assumptions

- OpenRouter text-to-speech support already exists in Vizier through the shared OpenAI-compatible speech path. This feature verifies it end to end, fixes any gaps found (defaults, error text, documentation), and keeps it consistent with the two new capabilities rather than redesigning it.
- OpenRouter offers all three media capabilities under its public API, with its own accepted audio formats, size limits, output formats and error responses. The exact request and response shapes are implementation concerns for the planning phase.
- Reasonable defaults, to be confirmed against OpenRouter's live model list during planning and recorded in the documentation: text-to-speech `openai/gpt-4o-mini-tts-2025-12-15` (already the current default), speech-to-text `openai/whisper-large-v3`, image generation `bytedance-seed/seedream-4.5`. If any of these is retired or unavailable at implementation time, the nearest stable OpenRouter model of the same modality is substituted and the documentation updated in the same change.
- The default voice for OpenRouter text-to-speech stays the existing `alloy` unless the chosen default model requires another voice.
- Language overrides follow the existing convention (a short language code such as `en`); when the provider cannot auto-detect or the code is unsupported, the provider's error is surfaced rather than a guessed language.
- The OpenRouter account has credit available. Free or promotional models may appear, disappear or behave differently; selecting one is the operator's choice and its provider errors follow FR-010.
- Media calls remain excluded from Vizier's token usage accounting, exactly as the existing speech, transcription and image capabilities are. Adding cost tracking for media is a separate concern.
- Image-to-image generation with reference images, streaming partial images, and multiple images per request are out of scope; the existing single-prompt, single-result contract of the image tool is preserved.
- Per-capability credentials, separate OpenRouter accounts, and provider-specific routing preferences (choosing upstream sub-providers) are out of scope.
- The WebUI is the configuration surface under test; API clients use the same fields and receive the same accepted provider values.
