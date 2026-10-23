# Specification Quality Checklist: WebUI Rewrite

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-10-23
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

- The repository layout (`legacy/`, the `webui` and `design` submodules, the `vizier-lab/webui` repository) appears in the requirements on purpose: the user asked for exactly that structure, so it is the requirement, not an implementation choice. The stack is mentioned only as an assumption for the plan to confirm.
- No clarification markers were needed. Defaults chosen in their place are under Assumptions: `legacy/` at the repository root; the "proposed" chat additions are in scope; the switch to the new WebUI happens once P1 and P2 pass.
- Server additions (FR-018 to FR-023) come from comparing the design's screens with the current HTTP API. API keys, roles, sharing, dream trigger and journal, revisions and integration events already exist and only need screens.
- Revision 2026-10-23: added the Home "Last 24 hours" dot grid (FR-018a/b) and the Dreams "Last 30 nights" timeline with per-cycle change attribution (FR-018c/d). Revisions currently record only that they came from a dream, not which cycle, so FR-018c is a server change. Revisions saved before it must not be miscounted.
- Revision 2026-10-23: added Chat turn internals: the Recalled memory, Background jobs and Session files panels, the trail summary and reply footer, and the context ring (FR-008a–c, US1 scenarios 7–9). Server gaps: auto-context passages are injected but never saved (FR-018e). Session files are stored, but only the agent can list them (FR-018f). Context use per conversation is needed for the ring (FR-018g).
