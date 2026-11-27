# Feature Specification: Catalogue Sources

**Feature Branch**: `068-catalogue-sources`

**Created**: 2026-11-27

**Status**: Draft

**Input**: User description: "Currently we rely on models.dev as our catalogue pricing, leaving other built-in providers and models that are not listed not calculated. We need a layer of abstraction for the catalogue, and models.dev is only one of our data points, so all providers and models will be tracked correctly."

## Context

Today vizier's catalogue *is* models.dev: the provider list, model facts (limits, capabilities)
and text prices all come from that one community catalogue. Anything it does not list, or lists
without a price, is reported as **unpriced** in the usage report. That includes:

- models running on the person's own machine (Ollama, llama.cpp, the offline test provider), which
  cost nothing;
- the ChatGPT subscription provider, whose usage is covered by a flat subscription;
- every model of an operator's custom provider;
- catalogue models published without a price (449 of the 10,022 models in the current seed);
- media models models.dev does not track: Venice's image models, for example, where vizier prices
  only one of them;
- any model at any provider that models.dev does not list at all.

Pricing for media and for AWS inference profiles already uses other data, but each follows its own
rules, and models.dev stays the base underneath them.

This feature turns the catalogue into an **aggregate of peer sources**. models.dev becomes one
source among several, with no special position. Providers that publish their own model lists with
prices (OpenRouter and Venice first) become sources in their own right. Every fact in the catalogue
records which source it came from.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A provider's own price list prices its models (Priority: P1)

An operator generates images with Venice and speech with OpenRouter. These models are not in
models.dev. The usage report prices them from the price list each provider publishes, and marks
them as priced by that provider.

**Why this priority**: It is the gap a single catalogue can never close. It also proves the
catalogue is an aggregate: a model is priced with models.dev playing no part.

**Independent Test**: Use a Venice image model and an OpenRouter speech model that models.dev does
not list, then open the usage report. Both are priced and labelled with their provider's list.

**Acceptance Scenarios**:

1. **Given** a Venice image model that only Venice's own list prices, **When** the report is read,
   **Then** its usage is priced from that list and labelled "Venice list".
2. **Given** a model that both its provider's own list and models.dev price, **When** the report is
   read, **Then** the provider's own list wins.
3. **Given** a provider list that has never been refreshed, **When** the report is read, **Then**
   the copy shipped with vizier answers.

---

### User Story 2 - Local and subscription usage is accounted for (Priority: P1)

An operator runs agents on Ollama and on the ChatGPT subscription. Those replies are reported at $0,
labelled "local" or "subscription", rather than counted as unpriced.

**Why this priority**: Every local install hits this gap, and fixing it needs no data at all,
only a rule.

**Independent Test**: Run an agent on a local provider and one on the subscription provider, then
open the usage report. Every reply is priced at $0 with the right label, and none is counted as
unpriced.

**Acceptance Scenarios**:

1. **Given** an agent on a local provider, **When** it answers, **Then** the report prices those
   replies at $0, labelled "local", and does not count their tokens as unpriced.
2. **Given** an agent on the ChatGPT subscription provider, **When** it answers, **Then** the report
   prices those replies at $0, labelled "subscription".
3. **Given** usage on a local provider recorded before this feature, **When** the report is opened,
   **Then** that past usage is priced the same way.

---

### User Story 3 - Every fact says where it came from (Priority: P1)

An operator looks at a model's row in the usage report and sees which source priced it. Where two
sources disagree on a price, the operator can see that they disagree and what the other one says.

**Why this priority**: Once several peers can answer, a figure without its source cannot be
trusted, and a disagreement hidden by precedence would hide a wrong price.

**Independent Test**: Produce usage priced by each source, including one model two sources price
differently, then open the report. Every model row names one source, and the disagreeing model
shows the other figure.

**Acceptance Scenarios**:

1. **Given** any priced model, **When** the report is read, **Then** its row names exactly one
   source.
2. **Given** two sources that price the same model differently, **When** the report is read,
   **Then** the winning source's price is used, and the row shows that another source disagrees,
   naming it and its price.
3. **Given** a model no source prices, **When** the report is read, **Then** it stays unpriced,
   listed in the unpriced note, and is never shown as $0.

---

### User Story 4 - Custom provider models get an estimate (Priority: P2)

An operator connects a custom provider that serves a model sold elsewhere (for example a proxy in
front of a well-known model). With no source pricing the custom provider itself, its usage is
priced by borrowing the price of the same model at another provider, marked as an estimate.

**Why this priority**: Custom providers are a smaller share of installs, but today none of their
usage is priced at all.

**Independent Test**: Create a custom provider with one model that matches a priced model elsewhere
and one that matches nothing. Use both, then open the report. The first is priced as "borrowed",
the second is unpriced.

**Acceptance Scenarios**:

1. **Given** a custom provider model that matches a priced model at another provider, **When** the
   report is read, **Then** it is priced at that price, marked "borrowed (estimate)", naming the
   provider it came from.
2. **Given** a custom provider model with no match, **When** the report is read, **Then** it is
   unpriced.

---

### User Story 5 - Model facts and providers come from every source (Priority: P3)

The same aggregate answers a model's other facts (context window, output limit, tool calling, image
input) and which models a provider offers in the pickers. A model that models.dev does not list,
but another source does, appears in its provider's model list, has known limits, and is not
flagged "isn't in the catalogue" on the agent page.

**Why this priority**: The value is smaller than pricing, but it is what makes the catalogue an
aggregate rather than a price lookup.

**Independent Test**: Pick a model listed only by a provider's own list, find it in the model
picker, set it on an agent, and check that no "unknown model" warning is shown and its limits are.

**Acceptance Scenarios**:

1. **Given** a model that only a provider's own list describes, **When** that provider's models are
   listed, **Then** the model is included.
2. **Given** such a model chosen for an agent, **When** the agent is saved, **Then** no "unknown
   model" warning is shown.
3. **Given** two sources that describe the same model, **When** its facts are read, **Then** each
   fact comes from the most authoritative source that has a value for it.

### Edge Cases

- **Different names for the same thing**: each source names providers and models in its own way
  (OpenRouter writes `anthropic/claude-sonnet-4.5`, models.dev writes `anthropic` /
  `claude-sonnet-4-5`). Every source's names are mapped onto vizier's provider and model ids before
  facts are combined, so one model is never counted as two, and two models are never merged into
  one.
- **A reseller's list**: OpenRouter's list prices models *at OpenRouter*. It is authoritative for
  the `openrouter` provider only, never for the original provider of a model it resells.
- A source has a model but not a given fact: the next source is asked for that fact. A source that
  has some facts does not hide the facts it lacks.
- A source fails to load or refresh (a corrupt shipped copy, a network failure): it keeps its last
  good copy, or is skipped with a logged warning. The other sources still answer, and no report or
  picker fails because of one source.
- A borrowed price for a local or subscription provider: never. Those rules come first.
- Two providers list the same model id at different prices when borrowing: the model's maker wins
  (OpenAI for an OpenAI model, never a reseller), then a provider that speaks the same protocol,
  then a fixed, documented order, so the same model always borrows the same price.
- A provider alias (the SSO identity of Amazon Bedrock) resolves through the same sources as the
  provider it stands for, so it gets one price, not two.
- A local model that really costs something (for example one rented by the hour): it is still
  reported at $0, since there is no way to set a price in this feature.
- Prices change over time: a report prices all usage at each source's current price, as today.

## Requirements *(mandatory)*

### Functional Requirements

**The aggregate**

- **FR-001**: The catalogue MUST be an aggregate of peer sources. Every query about a provider or a
  model (its models, price, limits, capabilities, whether it is listed) MUST go through the one
  aggregate. No source, models.dev included, MAY be read directly by the code that asks these
  questions.
- **FR-002**: How vizier connects to a provider (its protocol, address and credential fields) MUST be
  kept apart from the facts about its models, so a model can be described and priced by any source
  whatever source describes its provider's connection.
- **FR-003**: Each source MUST map its own provider and model names onto vizier's ids before its
  facts are combined. Names a source cannot map MUST be left out of the aggregate and counted in a
  logged summary, never guessed.
- **FR-004**: Each fact MUST come from the most authoritative source that has a value for it, in
  this order:
  1. the built-in rules (local providers at $0, subscription providers at $0);
  2. AWS inference-profile entries, for the profile they describe;
  3. the provider's own published list, for that provider only;
  4. vizier's curated table;
  5. general catalogues, models.dev being the first;
  6. a price borrowed from the same model at another provider.
  The order MUST be written down in one place. Custom provider rows MUST supply limits and
  capabilities for their own models ahead of every source but the built-in rules.
- **FR-005**: A source without a value for one fact MUST NOT stop less authoritative sources from
  answering it.
- **FR-006**: When two sources give different prices for the same model, the aggregate MUST keep the
  losing price and its source, so the disagreement can be shown.

**Sources in this feature**

- **FR-007**: models.dev MUST remain a source, with its current shipped copy, cache and refresh.
- **FR-008**: OpenRouter's and Venice's published model lists MUST be sources, covering their text
  and media models and the prices they publish. Each MUST ship with vizier, so a fresh or offline
  install has them.
- **FR-009**: vizier's curated table MUST be a source, holding today's media price figures and open
  to text prices. Every figure in it MUST name the page it was copied from and the date.
- **FR-010**: Adding a new source MUST need only the source itself (its data and its name mapping)
  and its place in the order. No code that reads prices or model facts MAY change.
- **FR-011**: A source's data MUST be refreshed only when a person reads the catalogue, or on the
  manual catalogue refresh, under the same rules and settings as models.dev today. Nothing in this
  feature MAY make a request at startup or on a schedule, and agents, the scheduler and dreams MUST
  never trigger one.
- **FR-012**: Each source MUST report when its data was last refreshed and whether its last refresh
  failed, as models.dev does today.

**Pricing**

- **FR-013**: Every price in the usage report MUST carry the source that produced it: the source's
  name, or local, subscription, borrowed or none.
- **FR-014**: Usage on a provider running on the person's own machine MUST be priced at $0, labelled
  local. This covers text replies, not only media as today.
- **FR-015**: Usage on a subscription provider MUST be priced at $0, labelled subscription.
- **FR-016**: A borrowed price MUST be labelled as an estimate, MUST name the provider it came from,
  and MUST be chosen the same way every time for the same model.
- **FR-017**: Usage that no source prices MUST stay unpriced, listed in the report's unpriced note,
  and MUST never be shown as $0.
- **FR-018**: Prices MUST be applied when a report is read, so a new source or a refreshed price also
  re-prices past usage, with no rewrite of stored usage.
- **FR-019**: Text and media MUST be priced by the same aggregate. A media model MUST be priced in
  its own units (per image, per character, per audio minute) when the winning source prices it that
  way.
- **FR-020**: The existing AWS inference-profile entries MUST keep their current pricing: the price
  saved on the entry, then the paired model, then unpriced.
- **FR-021**: Every figure vizier prices today MUST stay the same, unless a more authoritative source
  now answers. Each such change MUST be explainable by the source that now wins.

**Model facts**

- **FR-022**: A provider's model list in the pickers MUST include every model any source lists for
  that provider, once each.
- **FR-023**: A model described by any source MUST NOT trigger the agent page's "unknown model"
  warning.

### Key Entities

- **Source**: one publisher of facts about providers and models. It has a name (the label shown in
  reports), a kind (provider list, curated table, general catalogue, rule), the providers it is
  authoritative for, a shipped copy, and its refresh state.
- **Name mapping**: how one source's provider and model names translate into vizier's ids.
- **Model facts**: the combined answer for one provider and model. Each fact carries the source it
  came from, and a price carries any disagreeing prices.
- **Price**: rates per million input, output, cache-read and cache-write tokens, or per-unit media
  rates, plus its source and, for a borrowed price, the provider it came from.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Usage on local and subscription providers is never reported as unpriced: 0% of their
  tokens appear in the unpriced note.
- **SC-002**: Every default media model of OpenRouter and Venice is priced without an entry in
  vizier's curated table.
- **SC-003**: For an install using only local, subscription, models.dev-priced, OpenRouter and
  Venice models, the unpriced note is empty.
- **SC-004**: 100% of model rows in the usage report name exactly one price source.
- **SC-005**: Removing models.dev as a source leaves every model that another source covers fully
  priced and described. This is the test that models.dev is a peer, not the base.
- **SC-006**: Adding a source touches only that source and the one place the order is written down.

## Assumptions

- **No price can be entered by hand in this feature**: no overrides, no price field on custom
  provider model rows, no new settings screens. The source order leaves room for overrides as the
  most authoritative source later.
- **The first provider lists are OpenRouter and Venice.** Other providers that publish prices in
  their model lists can be added later as sources, one at a time. Which of them do is confirmed
  during planning; providers whose model lists carry no prices (OpenAI, Anthropic and others) stay
  with the other sources.
- **LiteLLM's price file is not a source in this feature.** It can be added later as another general
  catalogue.
- **Subscription usage is reported at $0, labelled "subscription".** An estimate of the pay-per-use
  cost is out of scope.
- **Borrowed prices count in the totals**, marked as estimates.
- "Local" means a built-in provider that runs on the person's machine (Ollama, llama.cpp, the
  offline test provider), or a provider a source marks local. A remote Ollama server is still
  treated as local.
- The existing OpenRouter media lists (spec 028) become the OpenRouter source, now keeping prices
  rather than dropping them.
- The only UI changes are showing the new source labels and price disagreements in the usage
  report. The disagreement display is drawn before it is built (FigCraft, or an HTML or ASCII
  mockup when FigCraft is unreachable).
- Storing the price at the time of use (price history) is out of scope.
