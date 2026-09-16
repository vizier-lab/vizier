# Specification Quality Checklist: Python Sandbox & Code Mode (Programmatic Tool Calling)

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-16
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

- **Iteration 1 (2026-09-16)**: One `[NEEDS CLARIFICATION]` marker (FR-007 — additive vs. exclusive tool exposure). User answered: **exclusive, with documentation tool(s)**. Encoded as FR-002 (exclusive tool list), FR-007/007a/007b (documentation tools), new User Story 2 (tool discovery), SC-009/SC-010, plus matching edge cases and assumptions.
- **Iteration 2 (2026-09-16)**: All items pass.
- **Iteration 3 (2026-09-16)**: User refined scope into **two switches**: (1) *sandbox* — pure-Python execution, additive to the tool list; (2) *code mode* — programmatic tool calling, exclusive, requires (1). Spec restructured: new P1 story for pure sandbox, stories re-prioritised (P1–P5), FR-001..FR-007 rewritten around the two switches and their dependency, FR-019 added (tool calls fail when code mode off), SC-001/SC-007 added, Key Entities and Assumptions updated. Re-validated: all items pass. Ready for `/speckit-plan`.
- **Iteration 4 (2026-09-17, during planning)**: User decided **no per-script memory ceiling in v1** after research showed Monty enforces cumulative memory only via a process-wide global allocator. Spec updated: FR-005/FR-021 drop the memory ceiling, FR-021a added (memory released per run — verified empirically), SC-006 narrowed, SC-013 added, US1-S4/US4-S6 and the runaway-script edge case adjusted, Assumptions record the decision and the residual risk. All items still pass.
- **Iteration 5 (2026-09-17, during planning)**: User removed every new limit setting — no `timeout` (the agent's existing per-tool timeout already bounds `execute_python` like any tool call), no `max_tool_calls`, no `max_output_bytes`. `PythonSandboxConfig` is now just `{enabled, code_mode}`. Spec updated: FR-005 rewritten (no settings beyond the switches), FR-013/FR-021 reworded, FR-026 and US2-S7/US5-S3 removed, US4 retitled, edge cases for many tool calls / large output added, Assumptions record the deferral. All items still pass.
- **Iteration 6 (2026-09-17, during planning)**: Added a mode-specific `SANDBOX.md` system-prompt briefing (folded into FR-008) so the model is told why its tool list changed and how to discover tools, rather than inferring it from a tool description. Plan: `system_prompt/sandbox.rs`; research Decision 12; text in `contracts/tool-definitions.md`.
- "Python" appears in the spec as the user-facing language of the feature (what the agent writes), not as an implementation choice; the candidate engine (Monty) is confined to the Assumptions section for the plan to validate.
- Items marked incomplete require spec updates before `/speckit-clarify` or `/speckit-plan`
