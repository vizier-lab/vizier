# Feature Specification: Usage and Spend of Images, Audio and Embeddings

**Feature Branch**: `065-media-usage-accounting`

**Created**: 2026-11-24

**Status**: Draft

**Input**: User description: "currently we don't track usage and money spend of image, audio, and embedding, can we track those too"

## Context

Spec 018 made an agent's usage page count every language-model completion made on its behalf, and deliberately left out audio transcription, speech synthesis, image generation and embeddings (018 FR-009). Those calls are real spend: an agent that draws pictures, answers live voice calls, reads voice messages, or keeps a large memory index can pay as much for them as for its chat, and today none of it shows anywhere.

These calls differ from completions in how they are measured and priced. A provider may bill an image per picture (by size and quality) or by tokens, speech by characters or tokens, transcription by audio minute or tokens, and embeddings by input tokens. Some calls run on a local model on the operator's own machine and cost nothing. The page must count them in their natural unit, price them when a price is known, and say plainly when it is not.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Every media call is counted (Priority: P1)

An operator opens an agent's usage page and trusts that it includes everything the agent spent in the period: not only chat completions, but also every image it generated, every piece of speech it produced, every recording it transcribed, and every embedding made for its memory and skills.

**Why this priority**: Without the records nothing else in this feature can exist, and an operator cannot find out what voice or images cost them.

**Independent Test**: On an agent configured with an image provider, a speech provider, a transcription provider and a remote embedding provider, generate one image, synthesize one reply, transcribe one voice message, hold one short live voice call, and write one memory. The agent's usage for the period lists each of those calls, each with its kind, provider, model, measured amount and activity.

**Acceptance Scenarios**:

1. **Given** an agent with image generation on, **When** it generates an image in a chat, **Then** one usage record appears with kind **Image**, the provider and model, the number of images and their size, and the activity of that chat.
2. **Given** an agent that answers with speech (a speech tool call, a spoken reply, or a live voice call), **When** speech is produced, **Then** a record appears with kind **Speech**, the number of characters synthesized and, where known, the length of the audio produced.
3. **Given** a person sends a voice message or speaks in a live voice call, **When** the agent transcribes it, **Then** a record appears with kind **Transcription** and the length of the audio transcribed.
4. **Given** an agent with a remote embedding provider, **When** a memory is written, searched, or automatically recalled for a turn, **Then** the embedding calls are recorded with kind **Embedding** and the number of input tokens.
5. **Given** an agent's memory is re-indexed at startup (a conversion or a change of embedding model), **When** the indexing runs, **Then** its embedding calls are recorded under the **Indexing** activity, with no conversation.
6. **Given** a provider that reports its own usage figures (tokens, images, seconds), **When** the call is recorded, **Then** the provider's figures are used; when it reports none, the figures are measured from the request and the result, and marked as measured rather than reported.
7. **Given** a media call that the provider refused or never answered, **When** usage is recorded, **Then** no amount is counted for it.

---

### User Story 2 - Know what media cost (Priority: P1)

An operator wants the media calls priced alongside chat, so the headline cost of the period is the agent's whole spend, and wants to see which media models cost how much.

**Why this priority**: The user asked for "money spend", not only counts. Counting without cost leaves the main question unanswered.

**Independent Test**: With seeded media usage on priced models, one unpriced model and one local model, the page's headline cost includes the priced media spend, the by-model breakdown lists each media model with its cost, the unpriced model appears in the "not in the estimate" note, and the local model shows as free.

**Acceptance Scenarios**:

1. **Given** media calls on models with a known price, **When** the operator views the page, **Then** their estimated cost is part of the headline, the daily chart, every breakdown and the costliest list.
2. **Given** media calls on a model with no known price, **When** the operator views the page, **Then** the page states the amount (images, characters, audio minutes or tokens) and the models left out of the estimate, as it does today for unpriced tokens.
3. **Given** media calls that ran on a local model (on-device transcription, local embeddings, a local server), **When** the operator views the page, **Then** they are counted with a cost of zero and labelled local, never as unpriced.
4. **Given** a model priced by tokens and a call reported in tokens, **When** it is priced, **Then** its input and output tokens are priced at the model's token rates; **given** a model priced per unit (per image, per character, per audio minute), **Then** the call is priced from its measured amount.
5. **Given** a price that changes later, **When** the operator views past usage, **Then** the estimate is computed with the current price, as it is for chat today.

6. **Given** a media model the provider catalogue does not price, **When** Vizier's shipped media price table lists it, **Then** the call is priced from that table; **given** a model in neither, **Then** it is unpriced (scenario 2).

---

### User Story 3 - See media on the usage page (Priority: P2)

An operator opens the usage page and can tell how much of the spend went to chat, images, speech, transcription and embeddings, and filter the page to one of them.

**Why this priority**: The numbers from Stories 1–2 are only useful if the page shows where they went; it builds on the page 018 delivered rather than adding a second one.

**Independent Test**: With seeded usage of every kind, the breakdown offers a **Kind** dimension listing Text, Image, Speech, Transcription and Embedding ranked by cost; filtering to Image scopes the headline, chart, breakdowns and costliest list to image calls only.

**Acceptance Scenarios**:

1. **Given** usage of several kinds, **When** the operator switches the breakdown to **Kind**, **Then** each kind is listed with its cost, share of total cost, calls and its amount in its own unit (tokens, images, characters, audio minutes).
2. **Given** the kind filter, **When** the operator chooses one kind, **Then** every part of the page is scoped to it, the headline also shows the kind's share of all spend, and the breakdown no longer offers Kind; the kind filter combines with the activity filter.
3. **Given** the **Model** breakdown, **When** media calls are in range, **Then** each media model appears beside the chat models with its kind, its amount in its own unit and its cost; token-only figures (cache hit rate) are shown only for models that have them.
4. **Given** the daily chart, **When** media calls are in range, **Then** their cost is included in each day's stack under the activity they belong to, and the day's tooltip lists the cost per kind.
5. **Given** a conversation's details (018 FR-022), **When** it includes media calls, **Then** they are listed among its calls with their kind, amount and cost.
6. **Given** a period with media usage only (no chat), **When** the page loads, **Then** it shows that usage rather than the empty state.

---

### User Story 4 - Media use is visible where it happens (Priority: P3)

A person in a chat wants to see what an image or a spoken reply cost, the same way a reply's tokens can be inspected today.

**Why this priority**: Useful for awareness and debugging, but the usage page already answers the operator's question.

**Independent Test**: Generate an image in a WebUI chat; the step that produced it shows the provider, model, amount and estimated cost of that call.

**Acceptance Scenarios**:

1. **Given** a tool step in a chat that generated an image, synthesized speech or transcribed audio, **When** a person who can view the agent's usage inspects the step, **Then** it shows the provider, model, amount and estimated cost of the media calls made by that step.
2. **Given** a person who cannot view the agent's usage, **When** they inspect the step, **Then** no cost is shown.

---

### Edge Cases

- **Usage from before the upgrade**: media calls made before this feature were never recorded and cannot be reconstructed; days before complete media accounting are marked in the chart as 018 FR-019 already marks days before complete accounting, and the page does not claim they had no media spend.
- **Live voice calls**: every utterance transcribed and every reply spoken is its own record in the call's conversation; a reply interrupted by barge-in counts what the provider produced, not what was played.
- **Retries**: a retried media request counts once per provider answer, never once per attempt.
- **A failed tool after a successful call**: an image the provider generated but the tool then failed to save is still counted; it was paid for.
- **Batched embeddings**: one request that embeds many passages is one record with the total input; a batch split by the provider's limits is one record per request sent.
- **Search embeddings**: the embedding of a search or recall query is counted under the conversation and activity of the turn that searched; one made outside any conversation is **Indexing**.
- **Skill index**: embeddings made for an agent's skill recommendations belong to that agent; embeddings made for no agent (install-wide work, provider connection tests) are not attributed to any agent and are not shown.
- **Dreams**: media and embedding calls made by a dream cycle are under **Dreams**.
- **Sub-agents and consultations**: media calls a worker or a consulted agent makes are attributed to the agent that ran them, as completions are today.
- **Mixed units on one model**: a model reported in tokens on one call and in units on another is priced per call by whatever that call reports.
- **Local models with no measurable amount**: counted as calls, with whatever amount can be measured, and a cost of zero.
- **Deleted agent / deleted conversation**: as 018 FR-008 — removed with the agent, kept when only the conversation is deleted.
- **Very busy indexing** (a large memory re-indexed at once): recording keeps up without slowing the indexing, and the page stays responsive (SC-005).

## Requirements *(mandatory)*

### Functional Requirements

**Accounting**

- **FR-001**: System MUST record every image generation, speech synthesis, audio transcription and embedding request made on an agent's behalf to a provider or local model, as its own usage record, at the time the request completes. This replaces 018 FR-009.
- **FR-002**: Each media record MUST carry, beside what 018 FR-002 lists where it applies: its **kind** (Image, Speech, Transcription or Embedding; completions are kind **Text**), its amount in the kind's unit — images (with size and quality when known), characters in, audio seconds in or out, input tokens — any tokens the provider reported, and whether the amount was reported by the provider or measured.
- **FR-003**: A media record's activity MUST follow the conversation it was made for, using 018's activities; calls made outside any conversation for keeping an agent's memory or skills indexed MUST use a new activity, **Indexing**.
- **FR-004**: When the provider reports its own usage figures, System MUST record them; otherwise System MUST measure the amount from the request and the result.
- **FR-005**: A retried request MUST be counted once per provider answer, never once per attempt; a request the provider refused or never answered MUST contribute no amount.
- **FR-006**: Calls to local models MUST be recorded like any other and marked local.
- **FR-007**: Recording MUST never delay, fail or alter the call it describes; a recording failure is logged and the call proceeds (as 018 FR-006).
- **FR-008**: Live voice calls MUST record every transcription and every synthesis they make, in the call's conversation.

**Cost**

- **FR-009**: System MUST estimate each media record's cost from the price of its provider and model: at token rates when the call is measured in tokens and the model has token prices, otherwise at the model's unit price for the call's unit.
- **FR-010**: Local calls MUST be priced at zero and MUST NOT appear as unpriced.
- **FR-011**: When media records in range have no known price, System MUST report the amount, in each kind's unit, and the models left out of the estimate (extending 018 FR-011).
- **FR-012**: Prices for media models MUST come from the provider catalogue first, then from a media price table shipped with Vizier that covers the built-in media providers' default and commonly used models, per token and/or per unit (image by size and quality, character, audio minute). The table is updated with releases; a model in neither source is unpriced. *(Clarified 2026-11-24: option B.)*
- **FR-020**: The page MUST be able to tell which source priced a figure, so a shipped-table estimate can be told apart from a catalogue price (for example in the Model breakdown's tooltip).

**Page**

- **FR-013**: Media cost MUST be included in the usage page's headline, change figure, daily chart, breakdowns and costliest list.
- **FR-014**: The breakdown MUST gain a **Kind** dimension beside Activity, Model and Channel, listing each kind's cost, share, calls and amount in its own unit.
- **FR-015**: The page MUST offer a kind filter (All, or one kind), combinable with the activity filter, that scopes every part of the page as the activity filter does (018 FR-021).
- **FR-016**: The Model breakdown and a conversation's details MUST show media calls with their kind and amount in its unit; token-only figures are shown only where they apply.
- **FR-017**: Days before media accounting began MUST be distinguishable in the daily chart in the same way as days before complete accounting (018 FR-019).
- **FR-018**: A chat step that made media calls MUST show their provider, model, amount and estimated cost to people allowed to view the agent's usage, and to no one else.
- **FR-019**: Access to media usage MUST follow the existing rules for viewing an agent's usage (018 FR-020).

### Key Entities

- **Usage record** (extended): one model call made for an agent, now of a **kind**. Media records carry their amount in the kind's unit, any reported tokens, whether the amount is reported or measured, and whether the model is local.
- **Kind**: Text, Image, Speech, Transcription, Embedding. A fixed set, used for grouping, filtering and display.
- **Unit**: the measure a kind is counted in — tokens, images, characters, audio seconds.
- **Media price**: the price of one provider's model, per token and/or per unit, from the catalogue or the shipped media price table (FR-012), with its source.
- **Activity** (extended): 018's activities plus **Indexing**.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For a controlled run covering image generation, a speech reply, a voice-message transcription, a live voice exchange, a memory write, a memory search and a startup re-index, 100% of the media calls made appear in the agent's usage under the correct kind, model, activity and amount.
- **SC-002**: For calls on priced models, the estimated cost of each call matches the provider's published price for its amount to within 1%.
- **SC-003**: An operator can answer "how much did images (or voice) cost this agent this month?" in no more than two interactions from the usage page.
- **SC-004**: Recording media usage adds no perceptible delay to image generation, speech, transcription, live voice responses or memory indexing (latency unchanged within measurement noise).
- **SC-005**: The usage page for a 90-day period on an agent with 100,000 completion records and 500,000 embedding records loads in under 2 seconds.
- **SC-006**: After upgrading, an agent's chat usage figures for any period are unchanged; only media spend is added.

## Assumptions

- This extends the per-agent usage page of spec 018; no install-wide overview is added.
- Vision (reading an image with a model) is already counted by 018 as a completion and is unchanged; "image" here means image generation.
- "Audio" means speech synthesis and transcription, including live voice calls. Music or other audio generation is not a capability today and is out of scope.
- Embeddings made by the agent's configured embedding model for its memory, recall and skill recommendations are in scope; work attributed to no agent, and provider connection tests, are not counted.
- Media prices change over time; like chat, estimates use the current price and are estimates, not invoices. The shipped price table can lag a provider's price change until the next release; operator-entered prices are out of scope.
- Usage is kept for as long as the agent exists, with no retention limit (as 018).
- Media calls made before the upgrade are not reconstructed.
- Embedding calls can be very frequent; the records may be aggregated per request batch, but never coarser than one record per request.
