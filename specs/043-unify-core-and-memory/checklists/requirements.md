# Specification Quality Checklist: Unify Core and Memory

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-11-01
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

- The decisions taken during specification, recorded in Assumptions rather than left as open questions:
  1. `core` is reserved and accepts only the `soul` document (FR-011 to FR-013).
  2. The soul is removable and always falls back to a well-formed default identity (FR-008, FR-009).
  3. Limits are 8 KiB / 4 KiB for memories and 4 KiB / 1.5 KiB for the soul, measured rather than chosen for roundness (FR-016 to FR-023).
  4. The injected soul carries its own address, and the doctrine names that address, because the dedicated identity operation is gone (FR-009, FR-010).
  5. Content carried by the upgrade is exempt from the ceiling, so the limit cannot block or split an upgrade (FR-025). This deliberately overrides the ceiling for exactly one write.
  6. Every compatibility mechanism for carrying old identity forward is tracked for removal before the first stable release (FR-026); the tracking entry for this feature is on the pre-1.0 removal issue.
  7. Every surface referring to the old arrangement is in scope, including tool-name lists, progress labels, the identity-only navigation entry and pages, templates, documentation and website copy (FR-030).
  8. **The dedicated identity area is removed, not redirected** (US4, FR-027, FR-029, SC-005). Once identity is a memory, a page built solely to show one document is a second place to look for something already reachable from the first. Its behaviour — finding it, revision history, restore, stale-save refusal — must survive in the memory surfaces; the entry, the pages and their addresses must not.
  9. **Core is the only product-facing term** (Assumption 1). The path segment the agent addresses is an implementation detail the product never names, so no visible label, route anchor or template name changes; what changes is where identity lives.
  10. **Identity changes are classified by bundle membership, not by document path** (FR-031 to FR-033). An agent may add and remove documents in the reserved bundle, so what the bundle holds is the agent's model of itself; a change to any of it is an identity change, and the identity document needs no special case. These changes are reported once, as identity, and never also as memories (FR-032) — a change from today, where a period that moved identity reported only that something moved.
- SC-007 is the one criterion that cannot be verified before shipping; it is a post-release measurement and is stated that way.
- **Sequencing of FR-030:** the functional surfaces (agent doctrine and tool guidance, tool-name lists, progress labels, templates, management-interface labels) must land with the feature, because an agent told to call something that no longer exists is a correctness bug. The descriptive surfaces (documentation pages, website and blog copy) are in FR-030's scope but deliberately run as a separate change on the same branch, since they are content work rather than behaviour.
- Revision history for the amended decisions: FR-003 originally required preserving the old identity record as live history; it now requires a byte-for-byte copy with the old record kept read-only and retired before 1.0.0, and SC-002 moved from "byte-identical" to "cacheable and stable" to admit the address wrapper.
