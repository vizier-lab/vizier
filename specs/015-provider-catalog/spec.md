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
3. **Given** a saved `gemini` key used by an agent's Gemini speech-to-text and embeddings, **When** vizier starts after upgrading, **Then** those features keep working from the migrated `google` credentials, with no key entered again.
4. **Given** a saved agent whose provider has no catalogue entry and is not a built-in provider (see FR-014), **When** vizier starts, **Then** the agent is not silently dropped. It shows as failed, with a message naming the provider and telling the operator to pick a replacement.
5. **Given** any agent turn after the upgrade, **When** the model replies, calls tools and receives their results, **Then** the conversation history and what the person sees are the same as before the upgrade.

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
5. **Given** the dream model and read-image (vision) model pickers, **When** the operator opens them, **Then** they use the same catalogue-backed provider and model pickers.

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
4. **Given** a provider with no saved key but one of its catalogue environment variables set, **When** the operator opens the providers page, **Then** that provider is listed as connected, labelled with the name of the environment variable it is using. The variable's value is never shown.
5. **Given** a connected provider, **When** the operator views it, **Then** it shows which features use it and how many agents each (for example "chat (3 agents) · TTS (1) · embeddings (2)").
6. **Given** a provider that agents still use, **When** the operator deletes its saved credentials, **Then** they are first warned with the list of affected agents and must confirm.
7. **Given** a local provider (Ollama, llama.cpp) or a custom provider, **When** the operator views the providers page, **Then** each shows whether its server is reachable, and how many models it serves when that can be listed.

---

### User Story 6 - Operator adds their own custom providers (Priority: P2)

An operator runs models the catalogue doesn't know about: vLLM on a home server, a LiteLLM gateway at work, or an Anthropic-format proxy. In the providers settings they choose **Custom provider** and enter:

- a name (the id is derived from the name and can be edited);
- the protocol: OpenAI-compatible or Anthropic;
- the base URL;
- an optional API key;
- optional extra headers.

Then they list the provider's models, either fetched from the server's own model list or typed in by hand. A model entry can carry its context window, maximum output and capabilities. An operator can have as many custom providers as they need, and agents pick them exactly like catalogue providers.

**Why this priority**: Self-hosted and proxied models are a core use for a self-hosted agent framework, and today there is only one fixed `custom` slot, which speaks only OpenAI's chat format. This story extends Story 5 to providers no catalogue will ever list.

**Independent Test**:
- Run a local OpenAI-compatible stub server and create a custom provider for it. Fetch its models, create an agent on one, and complete a turn with a tool call. The stub can echo tool calls, so this needs no key.
- Create a second custom provider using the Anthropic protocol against a stub.
- Check that a workspace with the old single `custom` provider still works after the upgrade.

**Acceptance Scenarios**:

1. **Given** the providers settings, **When** the operator creates a custom provider with the OpenAI-compatible protocol and a base URL, **Then** it shows under connected providers with a "custom" label, and agents can be created on it.
2. **Given** a custom provider whose server publishes a model list, **When** the operator asks to fetch models, **Then** the list is filled from the server, and the operator can still edit, add or remove entries.
3. **Given** a custom model entry with no limits or capabilities entered, **When** an agent uses it, **Then** the values come from a catalogue model with the same id if there is one, and otherwise from name-based detection.
4. **Given** two custom providers, **When** agents use each, **Then** each agent talks to its own provider's base URL, with that provider's key and headers.
5. **Given** a workspace with the old single `custom` provider (saved in settings, or set through `CUSTOM_BASE_URL`/`CUSTOM_API_KEY`), **When** vizier starts after upgrading, **Then** it becomes a custom provider with id `custom`, and agents on it keep working.
6. **Given** a catalogue refresh, **When** it completes, **Then** custom providers and their model lists are unchanged.
7. **Given** the custom provider form, **When** the operator tests the connection, **Then** they see either that the server is reachable (with how many models it lists) or the error it returned, such as a 401 or a refused connection, before saving.
8. **Given** a model list the operator has edited, **When** they fetch models from the server again, **Then** new models are added, and the edited entries keep the operator's values.
9. **Given** a saved custom provider, **When** the operator edits it, **Then** its id cannot be changed, and its API key and header values are shown masked, with an option to replace them.

---

### User Story 7 - Agent setup is simpler because vizier knows the models (Priority: P2)

An operator creating an agent in the wizard, or editing one in settings, sees one form either way. They pick a provider and model from the same searchable picker everywhere: main, dream and read-image models. The picker:

- lists tool-capable, current models first;
- preselects the provider's newest suitable model;
- shows the chosen model's limits, capabilities and price.

Context window and max output are no longer fields to fill in. They come from the catalogue, and can be overridden under "Advanced". The checkpoint threshold shows how many tokens it means. If the main model can see images, the read-image setup disappears, because images go straight to the model. Only when the model can't see images is the operator asked to pick a vision model. If the chosen provider isn't connected yet, the operator enters its key right there, without leaving the wizard.

**Why this priority**: The provider and model fields have to change for Stories 2–3 anyway. Doing it once, in one form, turns accurate model data into a visibly easier setup. It ranks below the P1 stories, which already deliver correctness.

**Independent Test**: With the bundled catalogue and no network access:
- Create an agent in the wizard on a vision-capable, tool-capable catalogue model. The default model must be preselected, no context or max-token value has to be entered, and no read-image model is asked for.
- Switch the model to one without image input: the vision-model picker must appear, listing only image-capable models.
- Open the agent's settings: the same form and pickers are shown, plus the sharing and danger-zone sections.
- Pick a provider with no credentials in the wizard, enter a key inline, and finish creating the agent.

**Acceptance Scenarios**:

1. **Given** the agent wizard and the agent settings page, **When** the operator opens either, **Then** both show the same form, with the same fields, pickers and validation. Settings adds only sharing and the danger zone.
2. **Given** the model picker, **When** it opens, **Then** it lists the provider's tool-capable, non-deprecated models, and offers a "show all models" option for the rest.
3. **Given** a provider just selected while creating an agent, **When** the model field is still empty, **Then** the provider's newest non-deprecated, tool-capable model is preselected.
4. **Given** a selected model, **When** the operator looks at the form, **Then** it shows the model's context window, maximum output, capabilities and price. The context-window and max-output fields are empty, show the catalogue values as placeholders, and sit under an "Advanced" section.
5. **Given** a checkpoint threshold, **When** the operator changes it, **Then** the form shows the matching token count for the selected model, for example "80% ≈ 160,000 tokens".
6. **Given** a main model that accepts images, **When** the operator views the read-image settings, **Then** no vision model is asked for, and the form says images go to the main model directly.
7. **Given** a main model that does not accept images, **When** the operator views the read-image settings, **Then** they are warned and offered a vision-model picker limited to image-capable models.
8. **Given** a provider with no credentials, **When** the operator selects it in the agent form, **Then** they can enter and save its credentials inline and carry on, and the key is saved to the shared provider settings.
9. **Given** the Review step of the wizard, **When** the operator reaches it, **Then** it shows the chosen model's card, with limits, capabilities, price and any warning (no tool calling, no image input).

---

### User Story 8 - Model prices feed the usage view (Priority: P3)

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
- **A custom server that is down**: test connection and fetch report the error, and the provider can still be saved, for example when the server is not running yet. Its card then shows "unreachable". Agents on it fail their turns with the server's error, as with any provider outage.
- **A custom provider whose id clashes** with a catalogue, built-in or other custom id: it cannot be saved, and the form names the clash. A catalogue refresh that later adds a provider with the same id as an existing custom provider MUST NOT take it over. The custom provider keeps its id, and the catalogue entry is hidden until the conflict is resolved.
- **A logo that is missing or fails to load**: the generic icon or the provider's initials are shown instead, and layout never breaks or shifts.
- **One key, several features**: an operator who saved an OpenAI key for chat turns on OpenAI text-to-speech. It works with no second key entry, both before and after the upgrade migration (FR-016a).
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

- **FR-005a**: Providers and models MUST be shown with logos from models.dev wherever they are listed: the provider picker, the model picker, the providers settings and the agent list.
  - **Provider logos**: a catalogue provider shows its own logo.
  - **Model logos**: a model shows its lab's logo, taken from the lab named in the model's canonical id (so a Claude model hosted on OpenRouter shows the Anthropic logo). If that lab has no logo, the model shows its hosting provider's logo.
  - **No logo**: providers and labs without a logo, built-in providers and custom providers show a generic icon or the provider's initials.
  - **Bundled**: logos MUST ship with the bundled catalogue snapshot and refresh with it, so they display offline and viewing a page never contacts models.dev.
  - **Safe**: logos MUST be displayed in a way that cannot run scripts embedded in them.
  - **Themes**: logos MUST stay legible in both light and dark themes.
- **FR-006**: Agents MUST identify their provider by catalogue provider id (or built-in provider id, FR-014) and their model by model id. Any provider whose protocol vizier supports MUST be selectable, not only a hard-coded list.
- **FR-007**: The system MUST support enough wire protocols to cover every provider supported before this change, plus every catalogue provider that uses those protocols. At minimum this means OpenAI-compatible chat, OpenAI, Anthropic, Google Gemini and Ollama.
- **FR-008**: Catalogue providers whose protocol is not supported MUST NOT be offered for selection.
- **FR-009**: Credentials MUST be resolved in this order: a value saved in provider settings first, then each environment variable the catalogue names for that provider, in order. The environment variable names vizier accepted before this change MUST keep working for the providers that had them.
- **FR-010**: The providers settings MUST let an admin save credentials for any selectable provider, asking only for the fields that provider needs.
- **FR-010a**: The providers settings MUST list as connected any provider whose credentials come from an environment variable, naming the variable but never showing its value.
- **FR-010b**: Each connected provider MUST show which features use it and how many agents each: chat (including dream and read-image), speech-to-text, text-to-speech, image generation and embeddings. Deleting a provider's credentials while agents use it MUST warn first, listing the affected agents.
- **FR-010c**: Local providers (Ollama, llama.cpp) and custom providers MUST show whether their server is reachable, and how many models it serves where it can list them. Checking this MUST NOT delay the page from loading.
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
  - ChatGPT subscription access;
  - the offline `dummyplug` test provider.

  Hyperbolic, Galadriel and Mira are **dropped as chat providers**, because none of them is in the catalogue. A saved agent on one of them is handled as in Story 1, scenario 3: it shows as failed, with a message naming the provider and asking the operator to pick a replacement. This is a breaking change. Hyperbolic stays available as a text-to-speech and image-generation provider.
- **FR-015**: On first start after upgrading, the system MUST migrate saved agents (provider and dream provider) and saved provider credentials from old provider names to catalogue ids, exactly once, and log each change. The HTTP API MUST keep accepting old provider names and map them to catalogue ids.
- **FR-016**: Speech-to-text, text-to-speech, image generation and embedding provider choices MUST keep working with their current providers and models. In this feature their pickers **stay curated** and are not driven by the catalogue. They move onto vizier's own provider layer (FR-017) without changing what they offer. The catalogue drives only chat-model selection: the main, dream and read-image models.
- **FR-016a**: Credentials MUST be saved once per provider and shared by every feature that uses that provider. One saved OpenAI key (or `OPENAI_API_KEY`) MUST serve chat, speech-to-text, text-to-speech, image generation and embeddings alike, which is today's behaviour. Each curated media or embedding provider MUST refer to the same provider id as its chat counterpart (for example, Gemini speech-to-text and embeddings use the `google` credentials after migration, and Together embeddings use `togetherai`), so the FR-015 migration never makes an operator enter a key again. Providers used only for media or embeddings (ElevenLabs, Voyage AI, Hyperbolic) MUST still be able to save credentials in the providers settings, even though they are not chat providers.

**rig removal**

- **FR-017**: The system MUST no longer depend on the rig library. Every provider connection, model listing, embedding, transcription, speech and image-generation call it handled MUST be served by vizier's own provider layer.
- **FR-018**: For the same configuration and conversation, what an agent does after this change MUST match what it did before:
  - the messages, tool calls and tool results exchanged;
  - the conversation history stored;
  - the retry behaviour on rate limits and outages;
  - prompt caching for Anthropic.
- **FR-019**: Provider errors MUST keep the provider's HTTP status and its retry-after hint, so rate limits and outages are recognised reliably. A provider's `Retry-After` MUST be honoured, within the turn's time limit.
- **FR-020**: Existing embedding indexes MUST stay valid: the same embedding provider and model MUST produce the same vectors as before, so stored memory search keeps matching.

**Custom providers**

- **FR-022**: Operators MUST be able to create, edit and delete any number of custom providers. Each has:
  - a name;
  - a unique id that does not clash with catalogue or built-in ids;
  - a protocol, either **OpenAI-compatible** or **Anthropic**;
  - a base URL;
  - an optional API key;
  - optional extra request headers.

  A custom provider's id MUST be fixed once it is first saved, because agents refer to it. To "rename" one, the operator duplicates it and moves the agents. API keys and header values MUST be treated as secrets: shown masked after saving, and never returned by the API.
- **FR-022a**: Before saving, the custom provider form MUST let the operator test the connection, which reports either success (with the number of models listed, where available) or the error the server returned.
- **FR-023**: A custom provider's models MUST be possible to list by hand, or fetch from the server's own model listing where it offers one. Fetched lists stay editable. Each model entry MAY carry a context window, maximum output, and capability flags (tool calling, image input, reasoning). When models are fetched again, new ids MUST be added, and entries the operator edited MUST keep the operator's values. Fetched entries whose id matches a catalogue model MUST be pre-filled from that model, and shown as such.
- **FR-024**: For a custom model, the FR-012 order applies, with the operator-entered values in the "catalogue value" step. Values that were not entered MUST fall back to a catalogue model with the same id, if one exists, before name-based detection.
- **FR-025**: The catalogue refresh MUST NOT change or remove custom providers.
- **FR-026**: The existing single `custom` provider, whether saved in settings or set through `CUSTOM_BASE_URL`/`CUSTOM_API_KEY`, MUST be migrated to a custom provider with id `custom` and the OpenAI-compatible protocol, so agents on it keep working.
- **FR-027**: The base URL of a catalogue provider MUST NOT be operator-overridable. To route a catalogue provider through a proxy or gateway, operators create a custom provider instead.

**Configuration file**

- **FR-028**: The `.vizier.yaml` config file MUST no longer hold provider configuration. Provider credentials come only from the providers settings (WebUI/API) and from environment variables (FR-009).
  - **Leftover config**: a `providers` section left in an existing config file MUST NOT stop startup. It is ignored, and a warning tells the operator to move it to the providers settings or to environment variables.
  - **Upgrades**: deployments that already imported their YAML providers into settings keep them as they are.
  - **Onboarding**: the onboarding wizard MUST NOT write providers into the config file.

**Agent form**

- **FR-029**: The WebUI MUST use a single agent form for both creating and editing an agent. The settings page adds only its sharing and danger-zone sections, and the separate settings-page copy of the form is removed. This merge MUST land before the picker changes below, so that each change is made once. In edit mode, a single sticky "Save changes" bar MUST appear whenever there are unsaved edits, replacing the per-tab Save buttons.
- **FR-030**: Every chat-model choice in the agent form (main, dream, read-image) MUST use one shared provider-and-model picker. It shows logos (FR-005a), capability badges, connected providers first and custom providers as their own group, and can be searched. The hard-coded provider and model lists in the WebUI MUST be removed.
- **FR-031**: The model picker MUST list tool-capable, non-deprecated models by default, with an option to show all. When a provider is chosen while creating an agent, its newest non-deprecated, tool-capable model MUST be preselected.
- **FR-032**: Context window and maximum output MUST be presented as optional overrides under an "Advanced" section, with the effective catalogue value shown as the placeholder. Rarely changed settings (thinking depth, prompt timeout, silent-read chance) MUST move into the same section. The checkpoint threshold MUST show its equivalent token count for the selected model.
- **FR-033**: Read-image MUST configure itself from the main model's capabilities:
  - **Main model accepts images**: images go to it directly, and no vision model is asked for.
  - **Main model does not accept images**: the form warns, and offers a vision-model picker limited to image-capable models.
  - **Existing agents**: an agent saved with an explicit vision model keeps using it. The form shows that vision model as an optional override the operator can remove.
- **FR-034**: Selecting a provider with no credentials in the agent form MUST let the operator enter and save its credentials inline. They are saved to the shared provider settings (FR-016a), subject to the same permission as the providers settings. Without that permission, the operator is told to ask an admin.
- **FR-035**: The wizard's Review step MUST show the chosen model's card: limits, capabilities, price and any capability warnings.

**Cost (P3)**

- **FR-021**: The usage view SHOULD show an estimated cost per agent and period, computed from token counts and the catalogue's prices for the agent's provider and model. Cost MUST be omitted, never shown as zero, where no price is known.

### Key Entities

- **Catalogue**: the set of known providers and models, with an "as of" time and a source (bundled snapshot or refreshed copy). One per vizier process, shared by all agents.
- **Catalogue Provider**: id, display name, wire protocol, base URL, credential environment variables, documentation link, and whether vizier supports its protocol.
- **Logo**: an image for a catalogue provider or model lab, keyed by its id. It is bundled and refreshed with the catalogue.
- **Catalogue Model**: belongs to one catalogue provider. Holds id, name, family, context and output limits, input and output kinds, capability flags (tool calling, reasoning, attachments, temperature), prices, status (for example deprecated), release date and knowledge cut-off.
- **Built-in Provider**: a provider vizier supports that the catalogue does not list (local Ollama, llama.cpp, ChatGPT subscription, dummyplug). It has the same shape as a catalogue provider, but vizier maintains it.
- **Custom Provider**: a provider an operator defines. It holds a name, id, protocol (OpenAI-compatible or Anthropic), base URL, optional key, optional headers, and its own list of model entries with optional limits and capabilities. It has the same shape as a catalogue provider, and refreshes never touch it.
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
- **SC-009**: An operator can connect a self-hosted OpenAI-compatible server and run an agent on it in under 2 minutes, without editing any file.
- **SC-010**: Creating an agent on a catalogue model needs no typed-in number for context window or max output, and no vision-model choice when the model can see images. That is at least 3 fewer required decisions than today.
- **SC-011**: The agent form's code is reduced by at least 40%, because the create and edit copies are merged.

## Assumptions

- **models.dev stays open** and keeps its current data shape, which is the one opencode depends on. Vizier pins to that shape and treats a refresh it can't read as a failed refresh, never as an empty catalogue.
- **The protocol hint decides compatibility.** The catalogue's protocol hint (the SDK package each provider is used through) is a reliable enough guide to the wire protocol. Providers that need special handling (Azure deployments, Copilot's token exchange, ChatGPT's subscription backend, Anthropic caching) get it in vizier, keyed by provider id.
- **Refresh defaults**: refresh is on, every 24 hours, and an admin can trigger one by hand. Deployments that must stay air-gapped turn it off in config. This keeps the constitution's rule that network use beyond the configured model provider is opt-out and never required for startup.
- **The bundled snapshot is refreshed at release time.** It is a build input, so there is no network access during `cargo build`.
- **Only admins** (the existing provider-management permission) can trigger a refresh or change refresh settings. Anyone who can edit agents can browse the catalogue.
- **The catalogue is advisory.** It never blocks a request the operator configured explicitly.
- **Streaming of model output** stays out of scope. Completions remain single request and response, as today.
- **No new external service**: the catalogue is a file kept in the workspace and storage, not a dependency vizier needs at runtime.
- **Onboarding without providers in YAML**: the onboarding wizard either saves the chosen provider's credentials straight into the providers settings, or tells the operator which environment variable to set. Which one is decided in planning.
- **Audio input** (sending voice messages straight to models that accept audio, skipping speech-to-text) is out of scope. It is a possible follow-up, mirroring FR-033.
- **The form merge** (FR-029) is a WebUI refactor with no behaviour change. It is planned as its own early phase.
- **Logos are trademarks** of their owners. They are used only to identify each provider or lab, which is how models.dev and opencode use them.
- **Breaking change**: dropping three chat providers (FR-014) and removing `providers` from `.vizier.yaml` (FR-028) are both breaking. Commit it as `feat!:` with `[**breaking**]`.
- **Media stays curated**: the speech-to-text, text-to-speech, image-generation and embedding pickers keep their curated lists in this feature (FR-016). Driving them from the catalogue is a possible follow-up.
- **Delivery order**: removing rig (FR-017–FR-020) and the catalogue (FR-001–FR-016) ship as one feature. Rebuilding the provider layer around wire protocols is what makes the catalogue's providers usable without per-provider code. They may still land in stages behind each other.
