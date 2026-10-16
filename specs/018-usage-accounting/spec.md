# Feature Specification: Complete Usage Accounting and an Insightful Usage Page

**Feature Branch**: `018-usage-accounting`

**Created**: 2026-10-16

**Status**: Draft

**Input**: User description: "We have a usage page, but does it also calculate 'hidden' token usage made by the system? The page itself is not simple and not really insightful — improve simplicity and insight. Ignore audio, embedding, TTS etc. for now. Also track the model being used."

## Context

Today an agent's usage is counted only from the replies it successfully delivers in a conversation. Every other language-model call the system makes on the agent's behalf is invisible: the dream cycle (extraction and consolidation), checkpoint handovers that summarise a long conversation (automatic and manual), the rounds of a turn that later failed, timed out or was aborted, and the separate vision model used to read images. On an agent with dreams enabled and long-running chats, the page can therefore understate both tokens and cost by a large and unknown margin.

The page itself presents raw totals (tokens, input, output, requests, cost), two daily token charts with display toggles, and a collapsible list of internal channel identifiers. It answers "how many tokens" but not the questions an operator actually has: what did this cost, is it rising, where is it going, what was the single most expensive thing, which model is responsible, and is anything wasting money.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Every model call is counted (Priority: P1)

An operator opens an agent's usage page and trusts that the totals include everything the agent spent on language models in the period — not just the replies people saw, but also dreams, conversation checkpoints, failed turns and image reading.

**Why this priority**: Every other improvement is built on these numbers. A simpler page that still undercounts is a more convincing wrong answer.

**Independent Test**: On an agent using the offline test provider, run one chat turn, one manual checkpoint, one dream cycle, one turn that fails partway through a tool loop, and one image read. The usage totals for the period equal the sum of all those calls, and each one is attributed to its activity.

**Acceptance Scenarios**:

1. **Given** an agent with dreams enabled, **When** a dream cycle runs, **Then** its extraction and consolidation calls appear in the agent's usage under the "Dreams" activity, with tokens, model and estimated cost.
2. **Given** a conversation large enough to trigger an automatic checkpoint, **When** the checkpoint summary is generated, **Then** that call appears under "Compaction", separately from the chat turn that triggered it.
3. **Given** a person requests a manual checkpoint, **When** it completes, **Then** that call also appears under "Compaction".
4. **Given** a turn that completes two model rounds and then fails (provider error, timeout, or the agent being restarted), **When** the operator views usage, **Then** the tokens of the completed rounds are counted and marked as spent on a failed turn.
5. **Given** an agent with a separate vision model, **When** it reads an image, **Then** that call is counted under the vision model, not the agent's chat model.
6. **Given** a model call that was made without any known purpose, **When** usage is recorded, **Then** it is still counted, under an "Other" activity, rather than dropped.

---

### User Story 2 - Know which model was used (Priority: P1)

An operator wants to see, for every call, which provider and model it was requested from and which model version the provider actually served, and to break usage down by model.

**Why this priority**: Cost is a function of the model. Agents change models over time, one turn can involve two models (chat plus vision), and providers serve dated versions behind aliases. Without per-call model tracking, cost attribution is guesswork.

**Independent Test**: Switch an agent from one model to another mid-period, have it read an image with a separate vision model, then view the "By model" breakdown: each model appears with its own tokens, calls and cost, and days before and after the switch are attributed to the right model.

**Acceptance Scenarios**:

1. **Given** an agent whose model was changed on day 3 of a 7-day period, **When** the operator views usage by model, **Then** days 1–2 are attributed to the old model and days 3–7 to the new one.
2. **Given** a provider that answers an alias with a dated version, **When** the call is recorded, **Then** both the requested model and the served version are kept, and the served version is visible on the page.
3. **Given** calls on a model with no known price, **When** the operator views the page, **Then** the page states how many tokens are excluded from the cost estimate and on which models, instead of silently leaving them out.

---

### User Story 3 - Understand spend at a glance (Priority: P2)

An operator opens the usage page and, within a few seconds, can answer: how much did this agent cost in the period, is that more or less than the previous period, and what kind of activity is it going to.

**Why this priority**: This is what makes the page insightful rather than just a tally; it depends on Stories 1–2 for its numbers.

**Independent Test**: With seeded usage across two consecutive periods and several activities, the page shows one headline cost with its change against the previous period, a daily cost trend, and a single ranked breakdown that can be switched between activity, model and channel.

**Acceptance Scenarios**:

1. **Given** usage in the current and previous period, **When** the page loads, **Then** it shows the period's estimated cost, its percentage change from the previous period of equal length, and total tokens as a secondary figure.
2. **Given** usage across several activities, **When** the operator views the breakdown, **Then** it lists each activity ranked by cost with its share of the total, tokens and number of calls.
3. **Given** the breakdown, **When** the operator switches it to "By model" or "By channel", **Then** the same ranked layout shows that dimension instead, without changing the rest of the page.
4. **Given** the daily chart, **When** the operator looks at it, **Then** it shows cost per day stacked by activity, and it offers no display-mode or metric toggles.
5. **Given** the channel breakdown, **When** a channel has a human-readable name (a Discord channel, a Telegram chat, a WebUI topic, a scheduled task), **Then** that name is shown instead of an internal identifier.
6. **Given** usage across several activities, **When** the operator filters the page to one activity (e.g. Dreams), **Then** the headline, change figure, daily chart, breakdown and costliest list all show that activity only; the headline also shows its share of all spend; and the breakdown offers only Model and Channel.
7. **Given** a filter on one activity, **When** the operator chooses "All", **Then** the page returns to its unfiltered state, keeping the chosen period and breakdown dimension.

---

### User Story 4 - Find what is expensive or wasteful (Priority: P3)

An operator wants to find the single conversations or scheduled runs that cost the most, and to see whether money is being wasted on failed turns or poor prompt caching.

**Why this priority**: These are the most actionable insights (a runaway tool loop, a broken cache), but they are refinements on top of an accurate, readable page.

**Independent Test**: With seeded usage containing one unusually expensive conversation, some failed turns and a mix of cached and uncached input, the page lists that conversation first among the costliest, shows the tokens spent on failed turns, and shows the cache hit rate.

**Acceptance Scenarios**:

1. **Given** usage across many conversations and task runs, **When** the operator views "Costliest conversations", **Then** the ten most expensive are listed by cost, each with its activity and model.
2. **Given** a conversation in that list, **When** the operator asks for its details, **Then** a details view opens on the usage page itself, without leaving it, showing the conversation's cost, number of calls, share of input served from cache, cost spent on failed turns, and its model calls ordered by cost, each with time, activity and round, served model, input and output tokens, cost, and whether it was part of a failed turn.
3. **Given** the details view of a conversation that has a page in the WebUI (a WebUI topic, a scheduled run, a dream cycle, a background job's launching topic), **When** the operator follows its link, **Then** that page opens at that conversation or run.
4. **Given** the details view of a conversation with no WebUI page (Discord, Telegram) or one that was deleted, **When** it opens, **Then** it offers no link and says why, and its usage is still shown in full.
5. **Given** a conversation whose cost is mostly failed turns (a runaway tool loop, repeated rate limiting), **When** its details open, **Then** the view states in one or two plain sentences what happened and how much of the cost bought nothing.
6. **Given** some calls that belonged to failed turns, **When** the operator views the page, **Then** the tokens and cost spent on failed turns are shown as one figure.
7. **Given** calls with cached input, **When** the operator views the page, **Then** the share of input served from cache is shown, both overall and per model.

---

### Edge Cases

- **Empty period**: the page states that there was no usage and does not show zero-filled charts or a misleading "−100%" change.
- **No previous-period data** (new agent, or upgrade just happened): the change figure is omitted rather than shown as infinite or 0%.
- **Usage from before the upgrade**: replies already in history keep counting, as chat usage on the model they recorded; they cannot gain the activities that were never recorded (dreams, checkpoints, failed rounds), and the page says so for the affected range.
- **Deleted conversations**: usage remains counted after a conversation is deleted; it appears in the costliest list without a link.
- **Deleted agent**: its usage is removed with the agent.
- **A failed call that the provider never answered** (connection error, rejected request): it contributes no tokens, but it can be counted as a failed call.
- **Retried calls**: only attempts the provider answered with usage figures are counted; a retry does not double-count.
- **Custom providers with no prices**: their tokens are counted and appear in the "unpriced" note (Story 2, scenario 3).
- **Very long periods** (90 days, a busy agent): the page remains responsive.
- **Mixed models on one day**: daily figures add up across models; the by-model view splits them.
- **Filter with no usage**: filtering to an activity that has no usage in the period shows the empty state for that activity, not the unfiltered page.
- **Filter and partial accounting**: the partial-accounting notice (FR-019) shows under a filter only when that activity was among those not recorded before the upgrade (Dreams, Compaction, Vision, failed rounds).
- **Conversation spanning activities**: one conversation can include chat, compaction and vision calls; in the costliest list it is ranked by its whole cost, or by that activity's share when the page is filtered, and its details list every call.
- **Long conversations in details**: a conversation with hundreds of calls shows its most expensive calls first and the count of the rest, without loading them all at once.

## Requirements *(mandatory)*

### Functional Requirements

**Accounting**

- **FR-001**: System MUST record every language-model completion made on an agent's behalf as its own usage record, at the time the call completes, independently of whether a reply is later delivered or saved.
- **FR-002**: Each usage record MUST carry: the agent, the conversation it belongs to (when there is one), its activity, the provider, the requested model, the model version the provider reported serving (when reported), input tokens, cached input tokens, cache-write tokens, output tokens, duration, outcome (succeeded / part of a failed turn), and time.
- **FR-003**: System MUST classify each record into exactly one activity: **Chat** (a turn answering a person or channel), **Background** (subtasks and delegations run as background jobs, and agent-to-agent consults), **Scheduled** (scheduled task runs), **Dreams** (dream extraction and consolidation), **Compaction** (automatic and manual checkpoint summaries), **Vision** (reading an image with a vision model), or **Other** (any call with no known purpose).
- **FR-004**: When a turn fails or is aborted, System MUST still count the rounds of that turn that the provider had already answered, and mark them as part of a failed turn.
- **FR-005**: A retried request MUST be counted once per provider answer that reported usage, never once per attempt.
- **FR-006**: Recording usage MUST never delay, fail or alter the call it describes; a recording failure is logged and the call proceeds.
- **FR-007**: Existing per-reply usage from before this feature MUST continue to count, as **Chat** usage on its recorded provider and model, so that upgrading does not empty the page.
- **FR-008**: Usage records MUST be removed when their agent is deleted and MUST be kept when only their conversation is deleted.
- **FR-009**: Audio transcription, speech synthesis, image generation and embeddings are out of scope; they are not recorded by this feature.

**Cost**

- **FR-010**: System MUST estimate each record's cost from the catalogue price of its provider and requested model, pricing cached and cache-write input at their own rates where known, as the current estimate does.
- **FR-011**: When records in range have no known price, System MUST report how many tokens and which models were left out of the cost estimate.

**Page**

- **FR-012**: The usage page MUST show, for a chosen period (last 7, 14, 30, 90 days, or a custom range): estimated cost, change against the immediately preceding period of equal length, and total tokens.
- **FR-013**: The page MUST show one daily chart of estimated cost, stacked by activity. Tokens per day MUST be available on hover; no separate metric or display-mode toggles.
- **FR-014**: The page MUST show one ranked breakdown, switchable between **Activity**, **Model** and **Channel**, listing for each entry its cost, share of total cost, tokens and calls.
- **FR-015**: The **Model** breakdown MUST show requested model with provider, and the served version(s) where they differ, plus the cache hit rate and failed-turn tokens for each model.
- **FR-016**: The **Channel** breakdown and the costliest list MUST show human-readable names. Discord conversations read `‹server› / #‹channel› / ‹topic›` and Telegram ones `‹chat title› / ‹topic›`, with the names looked up live on the platform through the agent's own connection when the page loads (bounded and cached), and the topic part only once the topic has a title. Without a name the identifier is shown. *(Revised 2026-10-16: live lookup replaces "names from storage only".)*
- **FR-017**: The page MUST list the ten costliest conversations and scheduled runs in the period, each with its activity, model and cost, and a way to open its details (FR-022).
- **FR-021**: The page MUST offer an activity filter (All, or one of the activities in FR-003) that scopes every part of the page — headline, change figure, daily chart, breakdown, costliest list, failed-turn and cache figures — to that activity. While filtered, the headline MUST also show the activity's share of all spend, and the breakdown MUST offer only Model and Channel.
- **FR-022**: Opening a conversation's details MUST show, without leaving the usage page: its cost, calls, cache share and failed-turn cost; its model calls ordered by cost (time, activity and round, served model, input and output tokens, cost, failed or not), showing the most expensive first and a count of the rest; and, when most of its cost was spent on failed turns, a short plain-language statement of what happened.
- **FR-023**: The details MUST link to the conversation where the WebUI has a page for it — a WebUI topic opens in chat, a scheduled run in its task's run history, a dream cycle in the dream journal, a background job in the topic that launched it — and otherwise MUST say why there is no link (Discord and Telegram conversations have no WebUI page; deleted conversations are gone).
- **FR-018**: The page MUST show tokens and cost spent on failed turns, and the share of input tokens served from cache.
- **FR-019**: Days that predate complete accounting (FR-007) MUST be distinguishable in the daily chart (shown faded, with a note in the day's tooltip). *(Revised 2026-10-16: no page-level notice; it was judged noise.)*
- **FR-020**: Access to an agent's usage MUST follow the existing rules for viewing that agent; this feature does not change who can see usage.

### Key Entities

- **Usage record**: one language-model call made for an agent. Attributes listed in FR-002. Belongs to one agent; optionally refers to one conversation.
- **Activity**: the purpose of a call (FR-003). A fixed set, used for grouping and display.
- **Model identity**: the provider plus the requested model id (used for pricing), and the served model version as the provider reports it (used for display and audit).
- **Usage period summary**: what the page displays for a period, optionally narrowed to one activity: totals, comparison with the previous period, daily series by activity, ranked breakdowns, costliest conversations, failed-turn spend, cache hit rate, unpriced-token note.
- **Conversation usage detail**: one conversation's usage records in the period, with its totals, an explanation when failed turns dominate, and a link to its WebUI page where one exists.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For a controlled run covering chat, background, scheduled, dream, compaction, failed-turn and vision calls, 100% of the model calls made appear in the agent's usage, each under the correct activity and model.
- **SC-002**: An operator shown the usage page for the first time can state the period's cost, whether it rose or fell, and the largest activity driving it, within 10 seconds.
- **SC-003**: An operator can identify the single most expensive conversation in the period and see which of its calls cost the most, in no more than two interactions.
- **SC-008**: An operator can answer "how much did dreams cost this month, and which dream cycle cost most?" in no more than two interactions from the usage page.
- **SC-004**: The page has at most one chart and no display toggles beyond the period picker, the activity filter and the breakdown dimension switch (down from two charts and four toggles).
- **SC-005**: Recording usage adds no perceptible delay to agent replies (turn latency unchanged within measurement noise on the offline test provider).
- **SC-006**: The page for a 90-day period on an agent with 100,000 recorded calls loads in under 2 seconds.
- **SC-007**: After upgrading, an agent's usage for periods before the upgrade shows the same totals as before the upgrade.

## Assumptions

- The page remains per agent, as it is today. An install-wide usage overview across all agents is out of scope for this feature.
- "Hidden" usage means language-model completions only; audio, speech, image generation and embeddings are deferred (user instruction).
- Pricing continues to come from the provider catalogue; custom providers stay unpriced until they gain prices, and are reported as such.
- Usage records are kept for as long as the agent exists; no retention limit or pruning is introduced.
- Background jobs, agent-to-agent consults and delegations are attributed to the agent that ran the call (the target agent), as today. Showing which agent *requested* the work is out of scope.
- Usage before the upgrade cannot be reconstructed for dreams, checkpoints, failed rounds or vision; only the reply-level figures already stored are carried over.
- The provider connection test ("test this provider" in Settings) is not attributed to any agent and is not counted.
- Existing access rules for viewing an agent govern its usage page.
- The activity filter is single-choice (one activity or All); choosing several at once is out of scope.
- Discord and Telegram conversations get no new WebUI page in this feature; their details view is the only place their usage is broken down per call.
- The page's layout follows the mockup at https://claude.ai/artifact/SENVYMyyTnC9UBQbVFbdgN (sample data only).
