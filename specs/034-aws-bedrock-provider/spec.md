# Feature Specification: AWS Bedrock Provider

**Feature Branch**: `034-aws-bedrock-provider`

**Created**: 2026-10-28

**Status**: Draft

**Input**: User description: "i want to add new provier for aws bedrock" (revised: IAM access key pair only; chat, embeddings and media included)

## Context

Vizier's provider layer speaks five wire protocols (OpenAI Chat, OpenAI Responses, Anthropic, Gemini and Ollama) and can reach any provider in the model catalogue that uses one of them. AWS Bedrock is in the catalogue with its chat models, context windows, capabilities and prices, but it is deliberately hidden from every provider and model picker because Vizier cannot speak Bedrock's API: a Bedrock request is not any of those five protocols, and a Bedrock endpoint is regional and requires AWS request signing rather than a single API key.

Today an operator who runs agents on AWS has no way to use Bedrock from Vizier. They cannot select it in Settings › Providers, an agent cannot be pointed at a Bedrock model or use Bedrock for memory embeddings or image generation, and the only workaround — a custom OpenAI-compatible gateway in front of Bedrock — loses the catalogue's models, capabilities and prices and requires running another service, which Vizier is built not to need.

This feature makes AWS Bedrock a first-class provider for the capabilities Bedrock offers in batches: chat, memory embeddings and image generation. One connection is saved once in Settings › Providers, region included, then each capability chooses Bedrock and its model like any other provider. Bedrock's unified Converse interface carries the conversations Vizier already builds (system prompts, multi-turn history, tool calls and results, images), and its embedding and image models receive the same prompts and text the existing memory indexer and image tool produce. Model limits, capabilities and prices keep coming from the catalogue, so context checkpoints, attachment handling, tool warnings and cost estimates work as they do for every other provider.

Speech is the one media capability Bedrock does not offer as a batch API (its speech model is real-time bidirectional), so text-to-speech and speech-to-text keep their current providers. Audio transcription by a separate AWS service, image-to-image, video generation and the AWS credential chain beyond a static access key pair are not part of this feature and are recorded as assumptions. The value is one connection and one bill for an operator already on AWS: agents that talk, remember and draw through Bedrock, configured where every other provider is configured.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Connect AWS Bedrock once and talk to a Bedrock model (Priority: P1)

A person with an AWS account opens Settings › Providers, finds Amazon Bedrock, follows the setup guide shown on the form to create an access key and enable the model they want, enters their AWS access key ID, secret access key and region, and saves. They create an agent, choose Amazon Bedrock as its provider, pick a Bedrock model from the model list, and send it a message. The agent answers using that model. From then on the connection is shared by every agent that selects Bedrock.

**Why this priority**: This is the core of the feature: without a saved connection and a reply, nothing else matters. It exercises the credential model, the regional endpoint, the protocol and the catalogue's model data end to end.

**Independent Test**: With only an AWS account and a Bedrock-enabled region, save the connection in Settings › Providers, create an agent with provider Amazon Bedrock and a listed model, and hold a multi-turn conversation. Repeat with the connection supplied entirely through environment variables, with no saved entry.

**Acceptance Scenarios**:

1. **Given** no Bedrock connection exists, **When** a person opens Settings › Providers and searches for Bedrock, **Then** Amazon Bedrock is listed with its logo and can be connected, instead of being hidden as unsupported.
2. **Given** a person opens the Bedrock connect form, **When** the form is shown, **Then** an inline "How to connect" guide lists the steps to create an access key, enable model access, choose a region and paste the values, with a copyable minimal IAM policy and links to the AWS pages it names and to the Vizier provider documentation.
3. **Given** the setup guide is open or a test has just failed, **When** the person needs to fix a permission or model-access problem, **Then** the guide explains the required `bedrock:InvokeModel` permission and the model-access step, and remains reachable without leaving the form.
4. **Given** a person enters a valid access key ID, secret access key and region, **When** they save and test the connection, **Then** the test reports success without starting an agent.
5. **Given** a saved Bedrock connection and an agent whose provider is Amazon Bedrock, **When** the agent is sent a message, **Then** it replies with a model response produced by Bedrock and the reply is stored in history like any other reply.
6. **Given** an agent on a Bedrock model, **When** the conversation continues across several turns, **Then** earlier turns, the agent's system prompt and its CORE document are all carried to the model, and the conversation stays coherent.
7. **Given** the connection comes from the environment rather than a saved entry, **When** the provider list is opened and an agent uses Bedrock, **Then** the provider is listed as coming from the environment variable that supplied it and the agent works without any saved credentials.
8. **Given** an access key ID but no secret access key, **When** the person tries to save or use the connection, **Then** the attempt is refused with a message naming the missing field, and no unauthenticated request is sent.
9. **Given** no region is set anywhere, **When** a Bedrock agent is started or tested, **Then** the failure names the region setting and the environment variables that can supply it, rather than guessing a region.
10. **Given** an agent configured with a cross-region inference profile id (`us.`, `eu.`, `apac.` or `global.` prefixed), **When** a turn runs, **Then** that id is sent unchanged to the configured region's endpoint.
11. **Given** a model id typed by hand that the picker does not list, **When** the agent runs, **Then** the id is sent unchanged and an invalid id produces the provider's error for that id rather than a silent substitution.

---

### User Story 2 - Bedrock models support the tools and images agents already use (Priority: P1)

An agent on a Bedrock model receives a request that needs tools — searching memory, running a script, calling another agent — and the model calls them. The results go back to the model and it finishes the turn. The same agent is sent a picture in the conversation and, because its model can see images, it answers about the picture. On a Bedrock model without vision, the picture is replaced by a short note and the agent still answers.

**Why this priority**: Tool calling is how Vizier agents do anything, and image input is part of a normal conversation on several channels. A Bedrock provider that could only exchange plain text would not be usable for real agents. It is independently testable and independently valuable.

**Independent Test**: Using a live AWS account, on an agent whose provider is Bedrock and whose model supports tool calling, ask a question that requires a tool (for example a memory search or a Python script), confirm the tool runs exactly once and the model's final reply uses its result. Then send an image to a vision-capable Bedrock model and to a text-only one and compare the two behaviours.

**Acceptance Scenarios**:

1. **Given** a Bedrock model that supports tool calling, **When** the model asks for a tool, **Then** the tool executes through the normal tool path with the agent's configuration, and its result is returned to the model in the same turn.
2. **Given** a model asks for several tools in one round, **When** the round runs, **Then** every call is executed and every result is returned together, and no call is executed twice.
3. **Given** an agent that restricts tool choice, **When** it runs on Bedrock, **Then** the restriction is honoured where the model supports it rather than being silently dropped.
4. **Given** a Bedrock model whose capabilities include image input, **When** the person sends an image, **Then** the model receives the image and the reply can refer to its contents.
5. **Given** a Bedrock model that cannot accept images, **When** a conversation containing images reaches it, **Then** each image is replaced by a text note in the request and the model still answers.
6. **Given** a tool call that fails or a tool result the model cannot use, **When** the round completes, **Then** the failure is reported to the model as a tool result and the turn ends with an explanation instead of crashing.
7. **Given** an agent on a Bedrock model with the catalogue's context window, **When** the conversation reaches the checkpoint threshold, **Then** the existing checkpoint and handover behaviour runs unchanged.

---

### User Story 3 - Memory search is embedded through Bedrock (Priority: P2)

A person opens an agent's embedding settings, chooses Amazon Bedrock and leaves the model empty, or picks a Bedrock embedding model explicitly. The agent saves memories as usual; each passage is embedded through Bedrock and stored in the memory index. A later `memory_search` finds the saved passage. The same saved Bedrock connection supplies the credentials and region, so no second setup is needed.

**Why this priority**: Memory is the feature that makes an agent continuous, and embeddings decide whether recall works at all. A Bedrock connection that could chat but not index memory would force the operator to keep a second vendor just for embeddings. It is independently testable and independent of image generation.

**Independent Test**: With only a Bedrock connection, set an agent's embedding provider to Amazon Bedrock with an empty model, save a memory containing a distinctive phrase, then search for that phrase and confirm the passage is returned. Repeat with an explicitly chosen Bedrock embedding model and confirm the same recall.

**Acceptance Scenarios**:

1. **Given** an agent whose embedding provider is Amazon Bedrock and whose model is empty, **When** the agent is built, **Then** it uses the documented default Bedrock embedding model and starts successfully.
2. **Given** a Bedrock embedding provider, **When** a memory document is saved, **Then** its passages are embedded through Bedrock and stored in the memory index without failing the save.
3. **Given** memories embedded through Bedrock, **When** `memory_search` runs, **Then** it returns the passages that match, with the same addresses, limits and behaviour as any other embedding provider.
4. **Given** an explicit Bedrock embedding model, **When** it is saved and the agent rebuilt, **Then** that model is used and its returned vectors are stored.
5. **Given** the Bedrock connection is missing credentials or a region, **When** an agent with Bedrock embeddings is built, **Then** the build fails with an actionable error naming Bedrock and the setting to fix, rather than starting with silent recall.
6. **Given** a model id that is not a supported Bedrock embedding model, **When** it is used, **Then** the failure names the model and the supported model families, and the agent does not present broken recall as working.
7. **Given** an agent using local embeddings or another provider's embeddings, **When** this feature ships, **Then** it keeps working exactly as before with no reindex and no configuration change.

---

### User Story 4 - Images are generated through Bedrock (Priority: P2)

A person enables image generation on an agent, chooses Amazon Bedrock, leaves the model empty or picks a Bedrock image model, and asks for a picture. The agent generates it through the same Bedrock connection and delivers it in the conversation, where it can be opened and downloaded like any other generated image.

**Why this priority**: Drawing is the other media capability Bedrock offers in batches, and it is independently valuable: an operator on AWS can stop paying for a separate image vendor. It is separate from chat and embeddings and can ship on its own.

**Independent Test**: With only a Bedrock connection, enable image generation with provider Amazon Bedrock and no model, ask the agent for an image, and confirm a valid image file appears in the conversation's files and is displayed. Repeat with an explicit model and a size.

**Acceptance Scenarios**:

1. **Given** an agent with image generation enabled for Amazon Bedrock and no model set, **When** the agent is asked to draw something, **Then** it uses the documented default Bedrock image model, saves a valid image in the conversation's files and delivers it as an attachment.
2. **Given** an explicit model and size are configured, **When** an image is generated, **Then** that model is used and the requested size is honoured where the model supports it.
3. **Given** no size is configured, **When** an image is generated, **Then** the documented default size is used and the returned image opens in a standard viewer.
4. **Given** Bedrock returns the image as PNG or JPEG, **When** it is saved, **Then** the file keeps the media type the provider actually returned, with a matching extension, and is not mislabeled.
5. **Given** the selected model is not a supported Bedrock image model, **When** generation is attempted, **Then** the tool call fails with a message naming the model and the supported families, and other tools in the turn are unaffected.
6. **Given** Bedrock blocks the prompt or returns no image, **When** generation is attempted, **Then** the tool call fails with the provider's explanation and no empty or broken file is saved.
7. **Given** the Bedrock connection is missing credentials or a region, **When** an agent with Bedrock image generation is built, **Then** it reports an actionable error naming Bedrock and the setting to fix.

---

### User Story 5 - Credentials, health and cost behave like every other provider (Priority: P2)

A person rotates their AWS keys, changes the region, removes the connection, or tries to use a region where their model is not enabled. Vizier tells them what is wrong, which agents are affected, and lets them recover by saving new credentials. Usage and estimated cost for Bedrock chat turns appear in the same usage views as every other provider, and one credential change applies to chat, embeddings and image generation together.

**Why this priority**: A connection that fails silently or hides its cost is not operable. Rotation and failure reporting are routine on AWS, and usage accounting is already promised for every answered completion. It does not block the first four slices but must ship with the feature.

**Independent Test**: Run a chat turn, then replace the saved keys with invalid ones and confirm the next turn fails with an actionable message naming Bedrock and the credential fields, the agent is shown as failed rather than silently idle, and fixing the credentials makes the next turn succeed. Check the usage view for the successful turns and confirm embeddings and image calls are treated like other non-chat media.

**Acceptance Scenarios**:

1. **Given** a working Bedrock agent, **When** the saved credentials are replaced with invalid ones, **Then** the next turn fails with an actionable error, agent health reports the failure, and the agent recovers once valid credentials are saved.
2. **Given** a Bedrock agent, **When** the saved region is changed, **Then** chat, embeddings and image generation all use the new region without affecting agents on other providers.
3. **Given** a Bedrock agent whose model is not enabled for the account or not offered in the configured region, **When** it tries to answer, **Then** the error names the model, the region and the likely cause, and the agent stays available for other work.
4. **Given** a Bedrock connection with an expired session token, **When** a turn runs, **Then** the provider's expiry is surfaced with the fix (a new session token or keys) rather than being reported as an unknown failure.
5. **Given** any answered Bedrock chat completion, **When** the usage report is opened, **Then** the turn appears with its input and output token counts and, when the catalogue prices the model, an estimated cost at the catalogue's rates.
6. **Given** a Bedrock embedding or image call, **When** usage is inspected, **Then** it is treated exactly like the existing embedding and image calls of other providers and no new cost is invented.
7. **Given** a person removes the Bedrock connection, **When** an agent still configured for it runs, **Then** the error says the provider is not connected and points to Settings › Providers.
8. **Given** a Bedrock credential is saved, **When** any provider, agent or settings response is inspected, **Then** the secret access key and session token are never returned in clear, while the access key ID and region are shown so the person can confirm what is connected.

---

### Edge Cases

- The saved access key pair is incomplete (id without secret, or the reverse): saving is refused naming the missing field; an already-stored incomplete pair fails with the same message rather than sending a request.
- Only a Bedrock API key (`AWS_BEARER_TOKEN_BEDROCK`) is supplied, with no access key pair: the failure says that Bedrock API keys are not supported yet and names the access-key variables, rather than reporting a generic missing key.
- The region is missing, empty, or not a region string: the failure names the setting and the environment variables, and no request is attempted.
- The region is valid but the model is not offered there: the provider's error is surfaced with the model id and region; agents on other models are unaffected.
- The account has not been granted access to the model: the failure explains that model access may need enabling in AWS, names the model, and does not look like a credential error.
- An IAM policy lacks `bedrock:InvokeModel`: the permission error is surfaced with the missing action named where AWS returns it.
- A session token expires mid-turn: the failure names expiry, the turn ends cleanly with a saved history entry, and the next turn succeeds after new credentials are saved.
- Throttling or a provider 5xx: the existing retry and backoff policy applies, an eventual failure is reported as the provider's error, and the agent stays healthy.
- The request is rejected as too long or over the model's output limit: the provider's message is shown; the turn is not silently truncated or retried forever.
- An image is too large or in a format Bedrock refuses: the tool or turn reports the provider's explanation, and already produced results in that turn are kept.
- A PDF or another document attachment is sent to a model that cannot read it: it is replaced by a note like any unsupported attachment, never sent as a broken image.
- The picker's catalogue lists a model retired upstream, or lags behind a new release: the listed or typed id is still attempted and the provider's answer decides.
- The embedding model returns vectors whose size differs from the agent's existing memory index: the incompatibility is reported clearly and no vectors of a different size are written into the index.
- The embedding model is a chat model id, or the image model is an embedding id: the failure names the model and the supported families instead of sending a malformed request.
- An embedding fails while a memory document is being saved: the save still succeeds, following the existing behaviour, and the failure is logged rather than silently losing the document.
- An image request returns more than one image: the first is saved; the rest are ignored, matching the existing single-result contract of the image tool.
- The prompt is blocked by Bedrock's content filter: the tool call reports the provider's explanation and saves no file.
- Two agents share the Bedrock connection, one using chat and one embeddings: one saved credential serves both; changing it restarts both and leaves agents on other providers untouched.
- Two Bedrock regions are needed at once: this is out of scope for the single connection; the limitation is documented rather than silently choosing one.
- The account uses AWS SSO, an instance profile or an assumed role with no static keys: the connection cannot be saved from those sources; the error names the supported credential form rather than pretending to support the chain.
- A turn is aborted or the agent restarts while a Bedrock request is in flight: no partial assistant message is saved, and the existing stop/resume rules apply.
- A model returns a response with blocks Vizier does not map (for example reasoning or citation blocks): the response is still usable; unsupported blocks never fail the turn.
- The Bedrock connection is removed while agents use it: each affected agent reports the missing provider, and no request is attempted with half-deleted credentials.

## Requirements *(mandatory)*

### Functional Requirements

**Connection and authentication**

- **FR-001**: `amazon-bedrock` MUST be a selectable provider for chat, embeddings and image generation in Settings › Providers, in the agent editor's provider and model pickers, and through the HTTP API, shown with its catalogue name and logo instead of being hidden as unsupported.
- **FR-002**: A Bedrock connection MUST accept an AWS access key ID, secret access key, optional session token and region. A Bedrock API key (bearer token) MUST NOT be offered in this iteration.
- **FR-003**: The region MUST be required for every Bedrock connection, taken from the saved connection, then `AWS_REGION`, then `AWS_DEFAULT_REGION`; when none is present the failure MUST name the setting and the environment variables that supply it, and MUST NOT guess a region.
- **FR-004**: Bedrock MUST use the saved connection when present and otherwise credentials found in the environment (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`), following the existing provider precedence and reporting the source in the provider list.
- **FR-005**: An access key pair with one half missing MUST be refused with a message naming the missing field, and no Bedrock request MUST ever be sent without a complete pair.
- **FR-006**: Every Bedrock request MUST be authentically signed with the configured access key pair (including the session token when one is present), for the configured region, so that credentials never travel in a header or query parameter.
- **FR-007**: Bedrock requests MUST go to the regional Bedrock endpoint derived from the configured region, and a Bedrock connection MUST NOT accept a `base_url` override; an operator who needs another address uses a custom provider.
- **FR-008**: The one saved connection MUST supply chat, embeddings and image generation together; changing its credentials or region MUST affect all three, and no capability may require a separate Bedrock credential.
- **FR-009**: Saving, changing or removing Bedrock credentials or the region MUST restart exactly the agents that use Bedrock, through the existing provider-change path, and MUST NOT affect agents on other providers; the next turn succeeds without restarting Vizier.
- **FR-010**: Secret access keys and session tokens MUST be encrypted at rest, MUST never be returned by any API, screen, log line, error message or telemetry attribute, and MUST be write-only after saving; the access key ID and region MAY be shown so the person can identify the connection.
- **FR-011**: The existing Test action for a provider MUST verify a Bedrock connection without starting an agent, report success or the provider's failure with a readable reason, and keep the saved credentials either way. The test MUST verify the region and credentials without requiring access to a specific chat model.
- **FR-011a**: The Bedrock connect and edit forms MUST include an in-form setup guide with the steps to obtain the connection: create an IAM access key, attach a minimal IAM policy that can be copied as-is (`bedrock:InvokeModel` plus the read permission the connection test uses), enable access to the chosen model in the Bedrock console, choose a region where Bedrock and the model are available, and state which value goes in each field — noting that the secret access key is shown only once and the session token is only for temporary credentials. The guide MUST link to the AWS pages it names and to the Vizier provider documentation, MUST stay available while the form is open (including after a failed test), and MUST never display or prefill a saved secret.
- **FR-011b**: The setup guide MUST be supplied as part of the provider's listing and rendered by the existing generic connect form, so no provider-specific branch is added to the form; its full text MUST be the Amazon Bedrock section of the provider documentation, reachable through a stable link.

**Chat**

- **FR-012**: Chat MUST use Bedrock's unified conversation interface (Converse), with Vizier's system prompt, full multi-turn history, tool definitions, tool results and assistant text translated to its request shape and its response translated back into Vizier's message parts.
- **FR-013**: Bedrock chat models MUST be listed from the existing model catalogue with their identifiers, readable names, context windows, output limits, capabilities and prices, and choosing one MUST send that identifier unchanged, including cross-region inference profile prefixes.
- **FR-014**: A chat model identifier that is absent from the picker MUST be accepted when typed or supplied through the API, stored exactly as entered and sent unchanged.
- **FR-015**: Tool calling MUST work end to end: tool definitions offered to the model, every requested call executed once through the normal tool path, results returned to the model, several calls per round supported, and the agent's tool-choice restriction honoured where the model supports it.
- **FR-016**: Image input MUST be sent to chat models whose catalogue capabilities include it, in a form Bedrock accepts; for models that cannot accept images, each image MUST be replaced by the existing text note rather than failing the request.
- **FR-017**: The context window, output limit and capabilities MUST come from the same resolution order as other providers (agent override, then catalogue), and the source of the effective context window MUST appear in agent health.
- **FR-018**: Every answered Bedrock chat completion MUST record its input and output token counts in usage accounting, priced at the catalogue's rates when the model is priced, exactly as other providers' completions are.

**Embeddings**

- **FR-019**: Amazon Bedrock MUST be selectable as an agent's embedding provider through the HTTP API and the WebUI, reusing the saved Bedrock connection and region.
- **FR-020**: An empty embedding model MUST resolve to a documented default Bedrock embedding model that needs no other setting, and an explicitly typed model id MUST be accepted and stored unchanged.
- **FR-021**: Bedrock's Amazon Titan Text Embeddings models and Cohere Embed models offered by Bedrock MUST be supported; a model id outside the supported families MUST fail with a message naming the model and the supported families rather than sending a malformed request.
- **FR-022**: Text sent to Bedrock embeddings MUST be embedded in batches through the existing embedding interface, and the returned vectors MUST feed the existing memory indexer, so saving memories and `memory_search` behave as they do for every other embedding provider.
- **FR-023**: The vector size MUST be the model's own published default (no new dimension setting); a model whose vector size differs from the agent's existing memory index MUST produce an actionable incompatibility error and MUST NOT write vectors of the wrong size.
- **FR-024**: An unusable Bedrock embedding configuration MUST fail the agent's build with an actionable error naming Bedrock, the region or the credentials; an embedding failure during a memory save MUST keep the existing behaviour (the save succeeds and the failure is logged).
- **FR-025**: Agents using local embeddings or another provider's embeddings MUST be unaffected, with no reindex and no configuration change.

**Image generation**

- **FR-026**: Amazon Bedrock MUST be selectable as an agent's image-generation provider through the HTTP API and the WebUI, reusing the saved Bedrock connection and region.
- **FR-027**: An empty image model MUST resolve to a documented default Bedrock image model that needs no other setting, and an explicitly typed model id MUST be accepted and stored unchanged.
- **FR-028**: Bedrock's Amazon Nova Canvas and Amazon Titan Image Generator models MUST be supported; a model id outside the supported families MUST fail with a message naming the model and the supported families.
- **FR-029**: Image generation MUST accept the prompt and the agent's optional size, use the documented default size when none is given, and send the size in the form the chosen model accepts; an unsupported size MUST surface the provider's explanation.
- **FR-030**: A generated image MUST be saved in the conversation's session files with the media type the provider actually returned and a matching extension, delivered as a channel attachment, and follow the same retention and deletion rules as other generated media.
- **FR-031**: A blocked prompt, an empty result or a provider failure MUST fail the tool call with the provider's explanation, save no partial file, and leave the agent able to continue the turn and later turns.
- **FR-032**: Image and embedding calls MUST NOT be added to chat token accounting, matching the existing treatment of media and embedding calls.

**Cross-cutting**

- **FR-033**: Bedrock's failures MUST be surfaced as actionable errors that preserve the provider's error code and message: missing or incomplete credentials, missing region, unsupported Bedrock API key, model not available in the region, model access not enabled, missing IAM permission, expired session token, throttling, validation errors, unsupported model family, service unavailability and timeouts MUST each read differently enough to act on.
- **FR-034**: A failed Bedrock request MUST NOT crash the agent, lose results already produced in the turn, or leave a half-written assistant message; the agent MUST stay available and its health MUST report the failure until it recovers.
- **FR-035**: Retries MUST follow the existing provider policy: throttling and retryable server failures are retried with backoff (honouring the provider's retry hint where present), and an exhausted retry budget ends the turn with the provider's error.
- **FR-036**: Bedrock MUST work everywhere any other provider works without special cases: ordinary chat on every channel, the dream cycle, scheduled tasks, subagents, background jobs, inter-agent messages, handovers and checkpoints, conversation history that predates the provider switch, and memory indexing for every agent.
- **FR-037**: Startup, provider and agent builds, the scheduler and dreams MUST NOT make any AWS request; requests happen only for a person-triggered test, an agent turn, a memory indexing operation or an image-generation call, and the catalogue's refresh rules are unchanged.
- **FR-038**: Existing providers, agents and stored configurations MUST behave exactly as before; no migration is required, and an agent on another provider MUST NOT be restarted or changed by this feature.
- **FR-039**: The documentation MUST cover connecting Bedrock in an **Amazon Bedrock** section that repeats the in-form setup guide — creating an access key, the minimal IAM policy, enabling model access, and choosing a region — plus the credential and region fields and their environment variables, the supported IAM-only authentication and the unsupported credential sources, the required AWS permissions, the model-access prerequisite, the regional and cross-region model-id behaviour, the supported embedding and image model families with their documented defaults and sizes, the single-region limitation, and the fact that Bedrock does not provide text-to-speech or speech-to-text; the section MUST be the stable link the form's guide points to, and the encryption documentation MUST list the new protected fields.
- **FR-040**: Bedrock's text-to-speech and speech-to-text provider choices MUST NOT be added, because Bedrock exposes no batch API for them; the documentation MUST say so and name the AWS services that would provide them instead.

### Key Entities

- **Bedrock connection**: the one saved credential entry for the provider, holding an access key ID, secret access key, optional session token and region, plus where the values came from (saved or an environment variable). Secrets are write-only and encrypted; the key ID and region are identifiers.
- **Bedrock chat model entry**: a catalogue model of the Bedrock provider with its identifier (possibly a cross-region inference profile), readable name, context window, output limit, capabilities (tool calling, image input) and price; the picker's source and the agent's selected value.
- **Bedrock embedding selection**: the agent's embedding provider and optional model id, resolved to a supported Bedrock embedding family, its vector size and the shared connection; the memory indexer's embedding source.
- **Bedrock image selection**: the agent's image-generation provider, optional model id and optional size, resolved to a supported Bedrock image family and the shared connection.
- **Agent provider selection**: an agent's provider and model for chat, plus its embedding and image settings, each resolved at build time into the effective endpoint, credential and limits.
- **Bedrock conversation request/response**: the translated chat turn — system prompt, messages, tool definitions and results, images — and the model's answer parts, stop reason and token usage.
- **Generated media artifact**: an image file attached to a conversation's session files with the provider's media type, produced by Bedrock or any other image provider.
- **Usage record**: one row per answered chat completion with provider, model, input and output tokens, activity and session, priced later from the catalogue.
- **Provider health**: the state an affected agent reports when its Bedrock request fails, including the last error and, when healthy, the effective context window and its source.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person with an AWS account, a region and credentials can connect Bedrock and get a first chat reply in under 3 minutes, with no step outside Settings › Providers and the agent editor.
- **SC-002**: 100% of the verification turns in the chat matrix — first turn, multi-turn, system prompt and CORE, tool call, several tool calls, tool result, image to a vision model, image to a text-only model, cross-region model id, and a checkpointed long conversation — complete with the expected reply or the expected replacement behaviour.
- **SC-003**: In 100% of missing-credential, incomplete-pair, missing-region, wrong-region and unsupported-API-key trials, the person sees an actionable message naming the setting (or environment variable) to fix, and no unauthenticated request is sent.
- **SC-004**: In 100% of checks, no secret access key or session token appears in an API response, a WebUI screen, a log line, an error message or a telemetry attribute; the access key ID and region remain visible.
- **SC-005**: 100% of tool-using turns execute each requested call exactly once, return every result to the model, and end with the model's final answer rather than a provider error.
- **SC-006**: A vision-capable Bedrock chat model answers correctly about an image in 100% of runs, and a text-only Bedrock chat model answers with the image replaced by a note in 100% of runs.
- **SC-007**: With Bedrock embeddings selected and the model left empty, 100% of newly saved memories are embedded, and `memory_search` returns the saved passage for a distinctive phrase in 100% of runs; the same holds for an explicitly chosen Titan or Cohere model from the supported families.
- **SC-008**: Selecting a supported Bedrock embedding model whose vector size differs from the agent's existing index produces a clear incompatibility error in 100% of trials and writes no wrong-sized vectors; agents on local or other embeddings are unchanged.
- **SC-009**: With Bedrock image generation selected and the model left empty, 100% of prompts produce a valid image in the conversation's files that opens in a standard viewer; an explicit supported model and size are honoured or the provider's refusal is explained; unsupported model ids and blocked prompts produce actionable errors with no broken file.
- **SC-010**: 100% of answered Bedrock chat completions appear in the usage views with non-zero token counts where the provider reports them, and their estimated cost matches the catalogue's rate for the model; models with no catalogue price show no invented cost, and embedding and image calls are not counted as chat tokens.
- **SC-011**: After invalid credentials are replaced with valid ones, the next chat turn succeeds with no Vizier restart, and agents on other providers are never restarted by a Bedrock credential change.
- **SC-012**: 100% of the existing provider and agent regression checks still pass; no existing stored configuration changes behaviour and no migration runs.
- **SC-013**: Zero AWS requests are made at startup, during agent builds, by the scheduler or by the dream cycle; every request is attributable to a person-triggered test, an agent turn, a memory indexing operation or an image-generation call.
- **SC-014**: A person finds Amazon Bedrock in Settings › Providers and a Bedrock chat model in an agent's picker on the first attempt, and sets Bedrock embeddings or image generation without leaving the agent editor, in 100% of usability trials.
- **SC-015**: In 100% of injected provider failures (throttling, model not enabled, permission denied, expired token, invalid model id, unsupported model family, oversized request, blocked prompt), the failure is reported with the provider's explanation, the agent remains able to run a later turn, and no partial assistant message or broken artifact is stored.
- **SC-016**: A person who has never connected Bedrock can obtain the credential values and complete the connection using only the in-form setup guide and the AWS pages it links to, without consulting any other Vizier documentation, in 100% of first-time trials and within 5 minutes.

## Assumptions

- The provider's identifier is the catalogue's `amazon-bedrock`, displayed as **Amazon Bedrock**; chat models, limits, capabilities and prices continue to come from the existing catalogue (models.dev).
- Only IAM access keys are supported for authentication: an access key ID, secret access key and optional session token, with requests signed for the configured region. Bedrock API keys (bearer tokens), shared profiles, SSO, instance profiles, ECS task roles, assumed roles and credential-process plugins are out of scope for this iteration; `AWS_BEARER_TOKEN_BEDROCK` is recognised only to explain that it is unsupported, not to authenticate.
- One Bedrock region is configured per installation, shared by chat, embeddings and image generation. Multiple regions at once, per-agent regions and automatic failover between regions are out of scope; cross-region inference profile identifiers remain the way to reach multiple regions from one configured region.
- The protocol is Bedrock's unified Converse interface for chat, which covers text, system prompts, multi-turn messages, tool use and image input across the model families Bedrock offers. Embeddings and image generation use Bedrock's model-invocation interface with the request and response shape of their model family.
- Supported embedding families are Amazon Titan Text Embeddings V2 (the documented default, 1024 dimensions) and Titan Text Embeddings V1, plus Cohere Embed English and Multilingual v3 as offered by Bedrock. They all default to a 1024-dimension vector, so they remain compatible with each other in one index. A dimension setting, Nova multimodal embeddings, asynchronous embedding jobs and custom output dimensions are out of scope.
- Supported image-generation families are Amazon Nova Canvas (the documented default, 1024×1024 when no size is set) and Amazon Titan Image Generator V2. Stability AI models on Bedrock, image-to-image, negative prompts, seeds, quality controls, multiple images per request and video generation (Nova Reel) are out of scope.
- Bedrock exposes no batch text-to-speech or speech-to-text API (its speech model is real-time bidirectional), so TTS and STT keep their current providers; Amazon Polly and Amazon Transcribe are separate services and are not wired in here.
- Model availability and model access are the operator's responsibility on AWS: a chat model may be listed in the catalogue but not enabled for the account or offered in the chosen region, and the provider's error is authoritative. Vizier does not silently substitute another model. Embedding and image model ids are not in the chat catalogue, so they are chosen from the documented supported families or typed.
- Streaming responses are not used: chat turns receive the complete response, as with every provider except the one that requires streaming. The `InvokeModelWithResponseStream` action is not required.
- Extended thinking, reasoning-content display, prompt caching, structured outputs, Guardrails, provisioned throughput, Batch inference, Bedrock Agents and Bedrock Knowledge Bases are out of scope. Chat responses containing blocks Vizier does not map must still produce a usable turn.
- The connection is opt-in: a fresh install makes no AWS call and needs no AWS configuration, preserving config-less startup. Only a person's test, an agent turn, a memory indexing operation or an image-generation call reaches AWS.
- The required AWS permissions are `bedrock:InvokeModel` for chat, embeddings and image generation, plus `bedrock:ListFoundationModels` for the connection test, which verifies the credentials and region without requiring model access. The exact policy is documented in the same change.
- Prices come from the catalogue and may be missing for a chat model or become stale; usage without a price is still recorded and shown without an invented cost estimate. Embedding and image calls stay outside chat token accounting, as existing media calls do.
- The existing retry policy, timeouts and error classification are reused rather than reinvented for Bedrock; only Bedrock-specific error translation is added.
- The WebUI connect form is generic over the provider's declared fields, so the new credential fields appear without redesigning Settings › Providers. The Bedrock setup guide is data on the provider listing rendered by that generic form, and its full text is the Amazon Bedrock section of the provider documentation. The existing embedding and image model fields remain free text, and the existing chat model picker lists catalogue models once the provider becomes selectable.
- Verification is split by access. The project's automated suite covers the wire translation, SigV4, credentials, encryption, media bodies and the UI with fixtures and does not contact AWS; the live behaviour (chat, tools, embeddings, images, model access and usage) is verified by a volunteer with Bedrock access on a special release, because the maintainers have no Bedrock account. The quickstart's live sections are that volunteer checklist, and a failure found there is fixed before the provider is declared stable.
