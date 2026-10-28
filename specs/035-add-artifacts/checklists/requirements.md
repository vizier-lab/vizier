# Specification Quality Checklist: Permanent Artifacts

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-10-29
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

- **Automatic collection is deliberately out of scope.** There is no retention period, background collector or size-based cleanup in this release. Session files are removed only by an explicit Clear session files action, `/clear-files`, conversation deletion, or agent deletion. This is recorded in the Context, FR-033, SC-014 and the Assumptions.
- **Artifacts have their own storage.** Artifact bytes live under `<workspace>/artifacts/<agent_id>/<artifact_id>/`, never under `uploads/`. Session files keep `uploads/`. This is a storage contract from the user's request and is recorded in the Context, FR-011, FR-025, FR-037, FR-040, FR-041, FR-013, FR-027, SC-017 and the Assumptions.
- **Promotion copies, it does not link.** FR-011 requires the artifact to own an independent copy and forbids marking, moving or linking the source. FR-043 and the Key Entities state that no promotion marker or reference count is needed to protect artifacts from clearing.
- **Clearing is explicit and conversation-scoped.** FR-034 to FR-043 cover the side panel, the channel command (canonical `/clear-files`; Telegram registers `/clear_files`), refusal while a turn is active, the count, the warning about legacy references, and the guarantee that artifacts and other conversations are untouched.
- **Session deletion is defined.** Deleting a conversation removes its session-file records and the actual stored file blobs, refuses while a turn is active, leaves artifacts intact because promotion copies, and makes legacy memory references to the deleted files show an unavailable placeholder. A blob still referenced by another conversation's session file is kept until its last reference is removed. Deleting an agent removes its session files, blobs and artifacts together. This is in the Context, US3 acceptance scenarios 9 to 14, FR-037, FR-040, FR-041, FR-054, SC-016 and the Assumptions.
- **Memory attachments use dedicated tools.** `memory_write` no longer takes attachments and never changes them. The agent links one artifact with `attach_artifact_to_memory` and removes one attachment with `detach_attachment_from_memory`; the memory API exposes matching attach/detach operations. Existing session-file memory attachments are legacy, readable while the file exists, detachable by name, and not migrated automatically. This is in the Context, US4 acceptance scenarios 5 to 11, FR-046 to FR-050, FR-054, SC-018 and the Assumptions.
- **Legacy session-file attachment paths are tracked in #82.** The pre-artifact message attachment fallback and the legacy memory attachment path are backward-compatibility code, not the permanent model. Before 1.0.0 they must be migrated to artifact references or intentionally dropped; #82 is the removal inventory. See the Assumptions section.
- The issue's open questions are resolved in the Assumptions section: agent-owned scope, overwrite-in-place versioning, agent-initiated promotion, text and binary support, references following the latest version, agent/permission rules, and no quotas in this release.
- Out of scope beyond automatic collection: retroactive promotion of existing session files, restoring earlier artifact versions, and artifact media in generative UI. Existing session-file memory attachments are also not migrated. Each is stated in the Context or Assumptions.
- Validation passed on the fourth revision. No clarifications were needed because the issue and the user's follow-ups supply the remaining constraints, and the remaining choices have reasonable product defaults.
