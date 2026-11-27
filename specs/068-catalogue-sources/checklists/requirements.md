# Specification Quality Checklist: Catalogue Sources

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-11-27
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- Provider and product names (models.dev, Ollama, ChatGPT, AWS) name what is priced. They are not
  implementation choices.
- FR-015 and SC-006 state the extensibility goal the request asked for ("a layer of abstraction")
  as an outcome, not a design.
- The open decisions from the design discussion are settled in Assumptions: subscription = $0,
  borrowed prices count as estimates. Hand-entered prices (overrides and custom-provider model
  prices) were dropped at the user's request on 2026-11-27, so the spec has no new settings screen.
- 2026-11-27: rewritten as an aggregate of peer sources, with models.dev no longer the base, at the
  user's request. Provider lists (OpenRouter, Venice) are in scope, and LiteLLM's price file is
  left out. SC-005 (models.dev can be removed) is the test that it is a peer. The checklist was
  re-run after the rewrite and passes.
- Example names in the edge cases (`anthropic/claude-sonnet-4.5`) show the naming problem; they
  are data, not a design.
