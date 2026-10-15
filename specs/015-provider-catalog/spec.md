# Feature Specification: Catalogue-Driven Providers and Models (and Removing rig)

**Feature Branch**: `015-provider-catalog`

**Created**: 2026-10-15

**Status**: Draft

**Input**: User description: "while removing rig. let's overhaul how we handle providers. instead mainly of hardcoded list, can we utilise this https://models.dev/ to get list of providers and models, i believe this is also utilised by opencode. by using that, we could get more accurate model list and additional data such as context windows and capabilities of the models"

**Related**: Issue #69 (remove `rig-core`). Draft technical notes are in [rig-removal-notes.md](rig-removal-notes.md).

## Background

Today vizier knows about 28 chat providers. Each one is hand-wired in four places:

- a fixed list of provider names;
- per-provider connection code borrowed from a third-party library (rig), which we must remove (#69);
- a credentials form per provider;
- a short, hand-typed list of model names in the WebUI.

That hand-typed list goes stale quickly, and it carries no facts about the models. Vizier has to guess a model's context window from its name, and it knows nothing about whether a model can call tools (which every agent needs, since agents always work in code mode), read images, reason, or how much it costs.

models.dev is an open, community-maintained catalogue of AI providers and their models, and opencode uses it too. For each provider it records:

- an id and display name;
- the environment variable that holds its key;
- the base URL of its API;
- which wire protocol it speaks.

For each model it records:

- context and output limits;
- what it can take as input and give as output (text, image, PDF, audio);
- whether it supports tool calling, reasoning and attachments;
- its price per million tokens;
- its knowledge cut-off, release date and status (for example deprecated).

At the time of writing it lists about 230 providers and 8,900 models. About 190 of those providers speak the OpenAI-compatible protocol, and most of the rest speak the Anthropic, OpenAI, Google or similar protocols.

This feature makes that catalogue the source of truth for which providers and models an operator can choose. The connection layer is rebuilt in vizier around a few wire protocols, rather than one hand-written client per provider, and that is what lets us drop rig. Agents get accurate limits and capabilities without anyone typing them in.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Existing deployments keep working after upgrade (Priority: P1)

An operator upgrades vizier. Their agents, provider credentials, scheduled tasks, dream settings and conversation history were all set up under the old provider names. After the upgrade every agent starts and answers exactly as before. Where an old provider name has a different id in the catalogue (for example `gemini` → `google`, `together` → `togetherai`, `moonshot` → `moonshotai`, `mimo` → `xiaomi`, `copilot` → `github-copilot`), the agent and its saved credentials are moved over automatically.

**Why this priority**: Removing rig and re-plumbing providers touches every agent turn. If upgrading breaks working agents, nothing else in this feature matters.

**Independent Test**: Start the new build on a workspace created by the previous release. It holds agents on at least `dummyplug`, `ollama`, `openai`, `anthropic` and `gemini`, with saved provider credentials. Every agent must come up healthy. The dummyplug agent must pass the existing 008/014 quickstart scripts unchanged. A provider whose name changed must show its saved key under its new catalogue entry.

**Acceptance Scenarios**:

1. **Given** a workspace from the previous release, **When** the new build starts, **Then** every agent that was healthy before is healthy after, with the same provider (under its catalogue id), model and credentials.
2. **Given** a saved agent whose provider name has a different catalogue id, **When** vizier starts, **Then** the agent's provider, dream provider and saved credentials are migrated to that id once, and the migration is logged.
3. **Given** a saved agent whose provider has no catalogue entry and is not a built-in provider (see FR-014), **When** vizier starts, **Then** the agent is not silently dropped. It shows as failed, with a message naming the provider and telling the operator to pick a replacement.
4. **Given** any agent turn after the upgrade, **When** the model replies, calls tools and receives their results, **Then** the conversation history and what the person sees are the same as before the upgrade.

---

### User Story 2 - Operator picks a provider and model from an accurate catalogue (Priority: P1)

An operator creating or editing an agent opens the provider picker. They see every provider vizier can talk to, searchable by name, not a fixed list of 28. After choosing one, the model picker lists that provider's actual models. Each model shows its context window, maximum output, whether it supports tools, images and reasoning, and its price. Deprecated models are marked. The operator can still type a model id that isn't listed (for example a fine-tune or a model newer than the catalogue).

**Why this priority**: This is the headline user-facing change, and it replaces the stale hand-typed lists.

**Independent Test**: With no network access, open the agent form. The provider list must contain at least the providers vizier supported before, plus the catalogue's other supported providers. Selecting `anthropic` must list its current models with context window and capabilities. Save an agent with a typed-in unlisted model id: it must be accepted.

**Acceptance Scenarios**:

1. **Given** the agent form, **When** the operator opens the provider picker, **Then** it lists every catalogue provider whose protocol vizier supports, plus the built-in providers. It can be searched by name or id, and each entry shows whether credentials are already configured.
2. **Given** a selected provider, **When** the operator opens the model picker, **Then** it lists that provider's models with name, context window, maximum output, capability badges (tools, image input, reasoning) and price. Deprecated models are marked or hidden.
3. **Given** a model id not in the catalogue, **When** the operator types it and saves, **Then** the agent is saved and runs. Values the catalogue would have supplied fall back to the agent's explicit settings or safe defaults.
4. **Given** a catalogue provider that speaks a protocol vizier does not support, **When** the operator browses providers, **Then** it is not offered for selection.
5. **Given** the dream model, read-image (vision) model and handover model pickers, **When** the operator opens them, **Then** they use the same catalogue-backed provider and model pickers.

---

### User Story 3 - Agents use accurate model limits and capabilities (Priority: P1)

When an agent runs, vizier knows its model's real context window and maximum output from the catalogue, and no longer guesses from the model's name. That matters for handovers, checkpoints and context-size reporting. Vizier also knows what the model can do:

- If the model cannot call tools, the operator is warned when saving, because an agent cannot work without tool calls.
- If the model cannot read images, attachments are handled without sending something the provider will reject.

**Why this priority**: Wrong context windows cause premature or late handovers and failed requests. This is the concrete payoff the user asked for.

**Independent Test**:
- Create an agent on a catalogue model whose context window differs from what name-guessing gives. Check that agent health and the usage view report the catalogue's value.
- Set an explicit context window on the agent and check that it wins.
- Save an agent on a model the catalogue marks as having no tool calling, and check that a warning is shown.

**Acceptance Scenarios**:

1. **Given** an agent on a catalogue model with no explicit context window, **When** it runs, **Then** its context window is the catalogue's value.
2. **Given** an agent with an explicit context window or max-tokens setting, **When** it runs, **Then** the explicit setting wins over the catalogue.
3. **Given** a model the catalogue says cannot call tools, **When** the operator saves an agent on it, **Then** they see a warning that the agent needs tool calling to work. Saving is still allowed, because the catalogue can be wrong.
4. **Given** a model the catalogue says cannot take image input, **When** a person sends that agent an image, **Then** the turn does not fail. The agent is told an image was attached that it cannot view, and the person's text still goes through.
5. **Given** a model with no catalogue data, **When** it runs, **Then** vizier falls back to the current behaviour: name-based detection, then the provider's own model listing where available.

---

### User Story 4 - Catalogue stays current without breaking offline installs (Priority: P2)

A release ships with a snapshot of the catalogue, so vizier works fully offline and on first start. When the network is available, vizier periodically refreshes the catalogue from models.dev, so new models show up without upgrading vizier. An operator can see when the catalogue was last refreshed, trigger a refresh, or turn refreshing off entirely.

**Why this priority**: A catalogue that only updates with releases recreates the staleness problem more slowly. But the bundled snapshot alone already delivers Stories 2–3, so this comes second.

**Independent Test**:
- Start vizier with no network: the provider and model pickers work from the bundled snapshot.
- Start it with network and refresh enabled: after a refresh, the reported "catalogue as of" time advances and a model newer than the snapshot appears.
- Start it with refresh disabled: no request is made to models.dev.

**Acceptance Scenarios**:

1. **Given** no network access, **When** vizier starts, **Then** the catalogue comes from the most recent refreshed copy if one was saved, or otherwise from the bundled snapshot. Nothing fails.
2. **Given** network access and refresh enabled, **When** the refresh interval elapses or an admin triggers a refresh, **Then** the catalogue is updated and saved, and its "as of" time is shown in settings.
3. **Given** a refresh that fails or returns invalid data, **When** it completes, **Then** the previous catalogue stays in use and the failure is logged and shown in settings.
4. **Given** refresh disabled in config, **When** vizier runs, **Then** it never contacts models.dev.
5. **Given** a refresh that removes a model an agent uses, **When** the agent next runs, **Then** it keeps working with its last known limits, and the model is shown as "no longer in catalogue".

---

### User Story 5 - Connecting a new OpenAI-compatible provider needs no code change (Priority: P2)

An operator wants to use a provider vizier has never had first-class support for, for example Cerebras, Fireworks, DeepInfra or Nebius. It appears in the provider picker because the catalogue lists it. The operator enters its API key in the providers settings and creates an agent on it, without waiting for a vizier release that adds that provider.

**Why this priority**: This is the main structural win of replacing hand-wired providers, but the providers people already use (Story 1) and accurate model data (Stories 2–3) matter more.

**Independent Test**: Pick a catalogue provider that vizier didn't support before and that speaks the OpenAI-compatible protocol. Enter a key, create an agent on one of its models, and complete a turn with a tool call. This needs a key, so mark it as live-provider only.

**Acceptance Scenarios**:

1. **Given** a catalogue provider vizier did not support before, using a supported protocol, **When** the operator saves its API key, **Then** agents can be created on it and complete turns with tool calls.
2. **Given** a provider's credentials, **When** they are looked up, **Then** a key saved in settings wins, and otherwise the environment variables the catalogue names for that provider are tried in order.
3. **Given** the providers settings page, **When** the operator opens it, **Then** each provider asks for the fields it needs: an API key for most, plus extra fields where the provider needs them (for example Azure's endpoint, or the base URL for local providers).

---

### User Story 6 - Model prices feed the usage view (Priority: P3)

The usage view already counts tokens per agent. With prices from the catalogue, it can also show an estimated cost per agent and per period, including cached-token discounts.

**Why this priority**: Useful and nearly free once the catalogue is in place, but it is not what the user asked for first.

**Independent Test**: Run a few turns on a priced catalogue model (or replay recorded usage). The usage view's estimated cost must equal tokens × catalogue price, with cache reads at the cache price.

**Acceptance Scenarios**:

1. **Given** usage on a priced catalogue model, **When** the operator opens the usage view, **Then** an estimated cost is shown next to token counts, labelled as an estimate.
2. **Given** usage on a model with no price data (local models, unlisted models), **When** the operator opens the usage view, **Then** no cost is shown for it, and it is not shown as zero.

### Edge Cases

- **Same model id at several providers**: the same model can appear under several providers with different limits and prices, for example a Claude model at Anthropic, OpenRouter and Copilot. Data is always taken from the chosen provider's entry, never from another provider's.
- **Model listed but rejected by the provider** (retired early, or missing from the operator's account): the turn fails with the provider's error, as it does today. The catalogue never blocks a request.
- **Provider without a base URL in the catalogue**: some major providers (OpenAI, Anthropic, Google, Groq, Mistral, xAI, Cohere, Together, Perplexity, Azure) have no base URL in the catalogue. Vizier supplies a known default for each provider it supports.
- **Local providers** (Ollama, llama.cpp, LM Studio): their model lists depend on what is installed, so the picker uses the running server's own model list when it is reachable, alongside any catalogue data.
- **Very large catalogue** (OpenRouter alone lists about 380 models): the pickers must stay usable through search and filtering, and must not lock up the page.
- **Two vizier processes sharing one workspace**: refreshing the catalogue must not corrupt the saved copy.
- **Refresh in the middle of a turn**: an agent that is running keeps the limits it started the turn with. The refreshed data applies from its next build.
- **Catalogue data that is obviously wrong** (missing limits, context window 0): it is treated as unknown, and the fallback detection is used.
- **An old client** (a WebUI build or script) still sends an old provider name such as `gemini` through the API: it is accepted and mapped to the catalogue id.

## Requirements *(mandatory)*

### Functional Requirements

**Catalogue**

- **FR-001**: The system MUST hold a catalogue of providers and models, built from models.dev data. For each provider it records:
  - id and display name;
  - which environment variables hold its credentials;
  - base URL;
  - wire protocol.

  For each model it records:
  - id and name;
  - context and output limits;
  - input and output kinds;
  - support for tool calling, reasoning and attachments;
  - prices per million tokens (input, output, cache read, cache write);
  - status;
  - release date.
- **FR-002**: Each release MUST bundle a snapshot of the catalogue, so provider and model selection works with no network access and on first start.
- **FR-003**: The system MUST be able to refresh the catalogue from models.dev periodically and on an admin's request. It MUST save the refreshed copy so later restarts use it, and MUST keep using the previous copy when a refresh fails or returns invalid data.
- **FR-004**: Catalogue refreshing MUST be possible to turn off in config, and the refresh interval MUST be configurable. When it is off, the system MUST make no request to models.dev.
- **FR-005**: The system MUST expose the catalogue to the WebUI and the HTTP API:
  - list providers;
  - list a provider's models;
  - get one model's details;
  - show when the catalogue was last refreshed and from where (bundled or refreshed).

**Provider selection and credentials**

- **FR-006**: Agents MUST identify their provider by catalogue provider id (or built-in provider id, FR-014) and their model by model id. Any provider whose protocol vizier supports MUST be selectable, not only a hard-coded list.
- **FR-007**: The system MUST support enough wire protocols to cover every provider supported before this change, plus every catalogue provider that uses those protocols. At minimum this means OpenAI-compatible chat, OpenAI, Anthropic, Google Gemini and Ollama.
- **FR-008**: Catalogue providers whose protocol is not supported MUST NOT be offered for selection.
- **FR-009**: Credentials MUST be resolved in this order: a value saved in provider settings first, then each environment variable the catalogue names for that provider, in order. The environment variable names vizier accepted before this change MUST keep working for the providers that had them.
- **FR-010**: The providers settings MUST let an admin save credentials for any selectable provider, asking only for the fields that provider needs.
- **FR-011**: The model picker MUST accept a typed-in model id that isn't in the catalogue.

**Using catalogue data at runtime**

- **FR-012**: An agent's effective context window MUST be decided in this order:
  1. its explicit setting;
  2. the catalogue value for its provider and model;
  3. the provider's own model listing (where available);
  4. detection from the model name.

  Maximum output tokens MUST be decided the same way. The catalogue value is used only where a provider requires a maximum and the agent has none set.
- **FR-013**: The system MUST record each model's catalogue capabilities, and act on them:
  - **No tool calling**: warn when an agent is saved on such a model.
  - **No image input**: when an image is sent to an agent on such a model, replace the image with a short note that an image was attached and cannot be viewed, so the turn does not fail.
  - **Unknown capabilities**: assume the capability is present, which is today's behaviour.

**Built-in providers and migration**

- **FR-014**: Providers vizier supports today that are not in the catalogue MUST remain available as built-in providers with the same behaviour:
  - local Ollama;
  - llama.cpp;
  - the generic custom OpenAI-compatible endpoint;
  - ChatGPT subscription access;
  - the offline `dummyplug` test provider.

  Hyperbolic, Galadriel and Mira are **dropped as chat providers**, because none of them is in the catalogue. A saved agent on one of them is handled as in Story 1, scenario 3: it shows as failed, with a message naming the provider and asking the operator to pick a replacement. This is a breaking change. Hyperbolic stays available as a text-to-speech and image-generation provider.
- **FR-015**: On first start after upgrading, the system MUST migrate saved agents (provider and dream provider) and saved provider credentials from old provider names to catalogue ids, exactly once, and log each change. The HTTP API MUST keep accepting old provider names and map them to catalogue ids.
- **FR-016**: Speech-to-text, text-to-speech, image generation and embedding provider choices MUST keep working with their current providers and models. In this feature their pickers **stay curated** and are not driven by the catalogue. They move onto vizier's own provider layer (FR-017) without changing what they offer. The catalogue drives only chat-model selection: the main, dream, read-image and handover models.

**rig removal**

- **FR-017**: The system MUST no longer depend on the rig library. Every provider connection, model listing, embedding, transcription, speech and image-generation call it handled MUST be served by vizier's own provider layer.
- **FR-018**: For the same configuration and conversation, what an agent does after this change MUST match what it did before:
  - the messages, tool calls and tool results exchanged;
  - the conversation history stored;
  - the retry behaviour on rate limits and outages;
  - prompt caching for Anthropic.
- **FR-019**: Provider errors MUST keep the provider's HTTP status and its retry-after hint, so rate limits and outages are recognised reliably. A provider's `Retry-After` MUST be honoured, within the turn's time limit.
- **FR-020**: Existing embedding indexes MUST stay valid: the same embedding provider and model MUST produce the same vectors as before, so stored memory search keeps matching.

**Cost (P3)**

- **FR-021**: The usage view SHOULD show an estimated cost per agent and period, computed from token counts and the catalogue's prices for the agent's provider and model. Cost MUST be omitted, never shown as zero, where no price is known.

### Key Entities

- **Catalogue**: the set of known providers and models, with an "as of" time and a source (bundled snapshot or refreshed copy). One per vizier process, shared by all agents.
- **Catalogue Provider**: id, display name, wire protocol, base URL, credential environment variables, documentation link, and whether vizier supports its protocol.
- **Catalogue Model**: belongs to one catalogue provider. Holds id, name, family, context and output limits, input and output kinds, capability flags (tool calling, reasoning, attachments, temperature), prices, status (for example deprecated), release date and knowledge cut-off.
- **Built-in Provider**: a provider vizier supports that the catalogue does not list (local Ollama, llama.cpp, custom endpoint, ChatGPT subscription, dummyplug). It has the same shape as a catalogue provider, but vizier maintains it.
- **Provider Credentials** (existing, re-keyed): saved per provider id, holding whatever fields that provider needs (API key; endpoint or base URL where required).
- **Agent model choice** (existing): the provider id and model id on an agent, its dream settings, and its read-image settings. It may carry explicit context-window and max-token overrides.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of agents that are healthy on the previous release are healthy after upgrading, with no manual step, on a test workspace that covers every provider name vizier supported before. The only exceptions are the dropped Hyperbolic, Galadriel and Mira chat providers, which must report a clear failure instead.
- **SC-002**: An operator can select at least 5× as many providers as before (28 today), with no code change per provider.
- **SC-003**: For every chat provider vizier supported before, the model picker offers that provider's current models, with context window and capability data shown, for at least 90% of the listed models.
- **SC-004**: The context window an agent uses matches the catalogue value for 100% of catalogue models, unless the agent explicitly overrides it.
- **SC-005**: With no network access, a fresh install can create an agent on any supported catalogue provider, choosing from a model list. Nothing in provider or model selection depends on reaching models.dev.
- **SC-006**: A model added to models.dev appears in the model picker within one refresh interval (default 24 hours), with no vizier upgrade.
- **SC-007**: The rig library is fully removed from the build, and the existing agent-loop end-to-end checks (the 008/014 dummyplug quickstarts) pass unchanged.
- **SC-008**: In the agent form, an operator can find a model by search among 380 or more models (OpenRouter) within 2 seconds of typing.

## Assumptions

- **models.dev stays open** and keeps its current data shape, which is the one opencode depends on. Vizier pins to that shape and treats a refresh it can't read as a failed refresh, never as an empty catalogue.
- **The protocol hint decides compatibility.** The catalogue's protocol hint (the SDK package each provider is used through) is a reliable enough guide to the wire protocol. Providers that need special handling (Azure deployments, Copilot's token exchange, ChatGPT's subscription backend, Anthropic caching) get it in vizier, keyed by provider id.
- **Refresh defaults**: refresh is on, every 24 hours, and an admin can trigger one by hand. Deployments that must stay air-gapped turn it off in config. This keeps the constitution's rule that network use beyond the configured model provider is opt-out and never required for startup.
- **The bundled snapshot is refreshed at release time.** It is a build input, so there is no network access during `cargo build`.
- **Only admins** (the existing provider-management permission) can trigger a refresh or change refresh settings. Anyone who can edit agents can browse the catalogue.
- **The catalogue is advisory.** It never blocks a request the operator configured explicitly.
- **Streaming of model output** stays out of scope. Completions remain single request and response, as today.
- **No new external service**: the catalogue is a file kept in the workspace and storage, not a dependency vizier needs at runtime.
- **Breaking change**: dropping three chat providers (FR-014) is breaking. Commit it as `feat!:` with `[**breaking**]`.
- **Media stays curated**: the speech-to-text, text-to-speech, image-generation and embedding pickers keep their curated lists in this feature (FR-016). Driving them from the catalogue is a possible follow-up.
- **Delivery order**: removing rig (FR-017–FR-020) and the catalogue (FR-001–FR-016) ship as one feature. Rebuilding the provider layer around wire protocols is what makes the catalogue's providers usable without per-provider code. They may still land in stages behind each other.
