# Specification Quality Checklist: Waking an agent in any conversation

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-11-26
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

- **No clarifications were needed.** Every open point had a reasonable default, so the spec carries
  zero `[NEEDS CLARIFICATION]` markers. The decisions the user made while shaping the feature are
  recorded in the spec's Clarifications section instead, which is this repository's convention.
- **Tool names are part of the contract, not an implementation detail.** `list_sessions` and
  `wake_session` appear in the requirements because the agent calls them by name. No language,
  framework, crate, module or API appears anywhere in the spec.
- **The audience is an agent, not a browser.** "User" in this spec means the agent that makes the
  call, and "the person" is the human the conversation belongs to. This follows the repository's
  existing specs (064-consult-agents and others), which are written the same way.
- **Out of scope, stated in Assumptions**: a handle/stop/panel for wakes, waking several
  conversations in one call, and changing how a scheduled run is set up. The agent in a scheduled
  run uses the same two tools a person-facing agent has.
- **Deferred, not blocking**: cross-person ownership rules beyond the calling agent's own person,
  rate limiting, and an audit trail of who woke what. The spec states the v1 boundary
  (FR-016 to FR-020) and leaves the rest to a later feature.
