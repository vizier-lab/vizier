# Feature Specification: Catalogue Sources

**Feature Branch**: `068-catalogue-sources`

**Created**: 2026-11-27

**Status**: Draft

**Input**: User description: "Currently we rely on models.dev as our catalogue pricing, leaving other built-in providers and models that are not listed not calculated. We need a layer of abstraction for the catalogue, and models.dev is only one of our data points, so all providers and models will be tracked correctly."

## Context

Today the usage report prices a model's text replies from one place only: the public model
catalogue vizier ships and refreshes (models.dev). Anything that catalogue does not list, or lists
without a price, shows up as **unpriced**. That includes:

- models running on the person's own machine (Ollama, llama.cpp, the offline test provider), which
  cost nothing but are reported as unknown;
- the ChatGPT subscription provider, whose usage is covered by a flat subscription;
- every model of an operator's custom provider;
- catalogue models published without a price (449 of the 10,022 models in the current seed);
- models the catalogue does not list at all.

Media usage (images, speech, transcription, embeddings) already has a second source, vizier's own
price table, and AWS inference profiles have a third, the price saved on the profile entry. These
three each follow their own rules. This feature makes the catalogue a single, ordered combination
of **sources**. models.dev becomes one of them, and every price in a report says which source it
came from.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Local and built-in usage is accounted for (Priority: P1)

An operator runs agents on Ollama and on the ChatGPT subscription. When they open the usage report,
those replies show a cost instead of sitting in "unpriced": local models cost $0 and are marked
local, and subscription usage costs $0 and is marked as covered by the subscription. The totals and
the "unpriced" note now cover only what really has no price.

**Why this priority**: It is the most common gap (every local install hits it) and needs no input
from the operator. It also proves that a source other than models.dev can answer.

**Independent Test**: Run an agent on a local provider and one on the subscription provider, send a
few messages, and open the usage report. Every reply is priced, at $0, with the right label, and
none is counted as unpriced.

**Acceptance Scenarios**:

1. **Given** an agent on a local provider, **When** it answers, **Then** the report prices those
   replies at $0, labels them "local", and does not count their tokens as unpriced.
2. **Given** an agent on the ChatGPT subscription provider, **When** it answers, **Then** the report
   prices those replies at $0 and labels them "subscription".
3. **Given** usage recorded before this feature on a local provider, **When** the report is opened,
   **Then** that past usage is priced the same way. Pricing happens when the report is read, so
   stored records need no rewrite.

---

### User Story 2 - Every price says where it came from (Priority: P1)

An operator looks at a model's row in the usage report and sees which source priced it: an AWS
inference-profile entry, vizier's built-in facts, the public catalogue, vizier's media price table,
an estimate borrowed from the same model at another provider, local, subscription, or unpriced.
They can tell a published price from an estimate at a glance.

**Why this priority**: Once more than one source can answer, a figure without its source cannot be
trusted. This story is what makes the other stories honest.

**Independent Test**: Produce usage that each source prices, then open the report and check that
every model row names one source and that the same model always names the same one.

**Acceptance Scenarios**:

1. **Given** a model priced by the public catalogue, **When** the report is read, **Then** its row
   names the catalogue as its source.
2. **Given** a model that two sources could price, **When** the report is read, **Then** the
   higher-priority source wins and its row names that source only.
3. **Given** a model no source prices, **When** the report is read, **Then** it stays unpriced,
   listed in the unpriced note, and is never shown as $0.

---

### User Story 3 - Custom provider models and unlisted models get an estimate (Priority: P2)

An operator connects a custom provider whose models are also sold elsewhere (for example a proxy in
front of a well-known model). Its usage is priced by borrowing the price of the same model at
another provider, marked as an estimate.

**Why this priority**: Custom providers are a smaller share of installs than local ones, but today
none of their usage is priced at all.

**Independent Test**: Create a custom provider with one model whose id matches a priced catalogue
model and one that matches nothing. Use both, and open the report: the first is priced as
"borrowed", the second is unpriced.

**Acceptance Scenarios**:

1. **Given** a custom provider model whose id matches a priced model at another provider, **When**
   the report is read, **Then** it is priced at that model's price and marked "borrowed
   (estimate)", naming the provider it was borrowed from.
2. **Given** a custom provider model with no match, **When** the report is read, **Then** it is
   unpriced.

---

### User Story 4 - Model facts beyond price come from every source (Priority: P3)

The same order of sources answers a model's other facts: context window, output limit, whether it
can call tools or see images. A model the public catalogue does not list but another source does is
no longer flagged "isn't in the catalogue" on the agent page, and its limits are known.

**Why this priority**: The value is smaller than pricing, but it uses the same mechanism.

**Independent Test**: Pick a model listed only by a non-catalogue source, set it on an agent, and
check that the "unknown model" warning is gone and its limits are shown.

**Acceptance Scenarios**:

1. **Given** a model that only a non-catalogue source describes, **When** it is chosen for an
   agent, **Then** no "unknown model" warning is shown.
2. **Given** two sources that describe the same model, **When** its facts are read, **Then** each
   fact comes from the highest-priority source that has a value for it.

### Edge Cases

- A source has a model but no price: the next source is asked for the price. A source that has some
  facts does not hide the facts it lacks.
- Two providers list the same model id at different prices when borrowing: the provider that speaks
  the same protocol wins, then a fixed, documented order, so the same model always borrows the
  same price.
- A borrowed price for a local or subscription provider: never. Those rules come before borrowing.
- A source fails to load (a corrupt shipped file, an unreadable stored row): it is skipped with a
  logged warning, and the other sources still answer. A report never fails because one source did.
- The public catalogue refresh is off or offline: the shipped copy still answers, as today.
- A provider alias (the SSO identity of Amazon Bedrock) resolves through the same sources as the
  provider it stands for, so it gets one price, not two.
- A local model that really costs something (for example one rented by the hour): it is still
  reported at $0, since there is no way to set a price in this feature.
- Prices change over time: a report prices all usage at the current price (as today). Storing the
  price at the time of use is out of scope.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The catalogue MUST combine an ordered list of sources, and every query about a
  provider or model (price, limits, capabilities, whether it is listed) MUST go through that one
  combination.
- **FR-002**: The sources, from highest to lowest priority, MUST be: custom provider rows (limits
  and capabilities), AWS inference-profile entries, vizier's built-in provider facts, the public
  catalogue (models.dev), vizier's media price table, and prices borrowed from the same model at
  another provider. The order MUST be written down in one place.
- **FR-003**: Each fact MUST be answered by the highest-priority source that has a value for that
  fact. A source without a value for one fact MUST NOT stop lower sources from answering it.
- **FR-004**: Every price in the usage report MUST carry the source that produced it. The labels
  are: inference profile, built-in, catalogue, media table, borrowed, local, subscription, or none.
- **FR-005**: Usage on a provider running on the person's own machine MUST be priced at $0 and
  labelled local. This covers text replies, not only media as today.
- **FR-006**: Usage on a subscription provider MUST be priced at $0 and labelled subscription.
- **FR-007**: A borrowed price MUST be labelled as an estimate, MUST name the provider it was
  borrowed from, and MUST be chosen the same way every time for the same model.
- **FR-008**: Usage that no source prices MUST stay unpriced, MUST be listed in the report's
  unpriced note, and MUST never be shown as $0.
- **FR-009**: Prices MUST be applied when a report is read, so a new source or a price change also
  re-prices past usage, with no rewrite of stored usage.
- **FR-010**: The existing AWS inference-profile entries MUST keep their current pricing behaviour:
  the price saved on the entry, then the paired model, then unpriced.
- **FR-011**: The existing media pricing order (local, then catalogue token price, then media table
  token rates, then media table unit price) MUST give the same figures for every model it already
  prices.
- **FR-012**: A model described by any source MUST NOT trigger the agent page's "unknown model"
  warning.
- **FR-013**: Adding a new source MUST NOT require changes to the code that reads prices or model
  facts (the report, the model pickers, the agent warnings).
- **FR-014**: A source that fails to load MUST be skipped with a logged warning, without failing the
  report or the other sources.
- **FR-015**: Nothing in this feature MAY make a network request that the catalogue does not make
  today. The public catalogue keeps its current refresh rules, which are triggered only by a
  person's read.

### Key Entities

- **Catalogue source**: one provider of facts about providers and models. It has a name (the label
  shown in reports), a fixed priority, and answers what it knows: price, limits, capabilities,
  whether a provider is local or a subscription.
- **Model facts**: the merged answer for one provider and model. Each fact carries the source it
  came from.
- **Price**: rates per million input, output, cache-read and cache-write tokens, or a per-unit
  media price, plus its source and, for a borrowed price, the provider it came from.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Usage on local and subscription providers is never reported as unpriced: 0% of their
  tokens appear in the unpriced note.
- **SC-002**: For a fresh install using only local, subscription and catalogue-priced models, the
  unpriced note is empty.
- **SC-003**: 100% of model rows in the usage report name exactly one price source.
- **SC-004**: Every figure vizier priced before this feature is unchanged after it, except where a
  higher-priority source now answers (a local or subscription model).
- **SC-005**: Adding a source touches only the new source and the one place the order is written
  down.

## Assumptions

- **No price can be entered by hand in this feature.** There is no override for any model and no
  price field on custom provider model rows, so there are no new settings screens. The existing AWS
  inference-profile entries keep the price they already carry. The source order leaves room to add
  overrides as the first source later.
- **Subscription usage is reported at $0, labelled "subscription".** An estimate of what the same
  usage would cost on the pay-per-use service is out of scope, but can be added later as a separate
  figure.
- **Borrowed prices count in the totals**, marked as estimates. Leaving them out would leave custom
  providers unpriced, which is the problem being fixed.
- "Local" means a built-in provider that runs on the person's machine (Ollama, llama.cpp, the
  offline test provider), or a public-catalogue provider marked local. A remote Ollama server is
  still treated as local.
- The only UI change is showing the new source labels in the usage report, which reuses the report's
  existing price-source display. No new screen or layout is needed.
- Live price lists published by providers themselves (such as OpenRouter's model list) are a
  possible later source. They are out of scope here because they would add network requests (see
  FR-015).
- Storing the price at the time of use (price history) is out of scope. Reports price all usage at
  the current price, as today.
