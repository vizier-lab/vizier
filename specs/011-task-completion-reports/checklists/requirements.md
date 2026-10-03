# Specification Quality Checklist: Task Results and Requester

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-10-03
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

- All items pass. Ready for `/speckit-plan`.
- Iteration 2 (user direction): run list is paginated (the relevant requirements), and the
  retention bound from iteration 1 was deleted rather than kept — session
  history already grows unbounded for every other channel, so trimming task
  runs alone would be both inconsistent and scope the user did not ask for.
  the relevant requirements now states retention positively so a later reader does not reintroduce
  a trim.
- the relevant requirements was reworded after checking the code: the existing chat/topics/history
  routes hardcode `VizierChannelId::HTTP(user, channel_id)` and cannot address a
  task run's session, so "open the conversation" could not have meant a jump to
  the chat view. The exchange is read inside the task's own view instead, and
  the relevant requirements guards against exposing run results through a person-scoped route.
- Iteration 3 (user direction): requester tracking folded in as the relevant requirements.
  One field, not two — the system knows the call path at creation, so storing
  origin separately would be storage for something derivable (constitution
  Principle I). The agent declares *whether* a task is its own initiative; the
  system decides *who* the person is, since a model asserting human provenance
  is the same failure class 012 addresses in the request frontmatter.
- the relevant requirements is a deliberate breaking change to task creation (a caller-supplied
  requester is ignored). Flagged in Assumptions; the commit will need
  `[**breaking**]` per the repo's changelog convention.
- Iteration 4 (user direction): the account-existence check on a person
  requester was dropped. `Task.user` was never an account — Discord and Telegram
  already synthesise an identity string ("@name (DiscordId: …)"), and accounts
  exist for web sign-in only, so requiring one would refuse the commonest case:
  someone on a chat channel asking the agent to schedule something. the relevant requirements now
  states the opposite requirement. The enum distinction kept is own-initiative
  vs someone-asked, not verified vs unverified.
- Consequence recorded in Assumptions rather than designed now: delivering a
  notification to a requester would need a structured channel-qualified
  identity, not today's descriptive string. Left to the feature that needs it.
- Iteration 5 (user direction): an agent-facing way to read its own task
  reports, responses only (US4, the relevant requirements). Noted during this pass: the
  per-run session separation this feature introduces REMOVES continuity that
  recurring tasks get today by accident (all runs collide into one session and
  replay each other). That makes US4 a prerequisite of the separation rather
  than an optional extra, and is recorded in Assumptions so planning cannot
  drop one without the other.
- Iteration 6 (user direction): US4 split into list-then-fetch (the relevant requirements),
  mirroring the memory tools' search/read idiom, and excluded from the dream
  tool set. Correction worth carrying into planning: the dream cycle does NOT
  cover task sessions — `is_non_user_channel` (storage/sqlite/history.rs:63)
  filters `task__` out of `list_user_sessions_in_window`, so no reflection
  happens on task outcomes today. Excluding the tool from the dream set is still
  the right call, but not because dreaming already covers tasks. Widening that
  filter is a separate question and is out of scope here.
- Iteration 7 (user direction, reversing iteration 6): both tools ARE in the
  dream set (the relevant requirements). The filter finding is what justifies it — because the
  cycle never sees task conversations, these tools are the only route by which a
  task's outcome can reach memory. Known limitation recorded in Assumptions
  rather than fixed: the cycle only runs when there were conversations in the
  window (dream/mod.rs early-returns on an empty session list), so an agent that
  only runs tasks still never reflects. Pre-existing gating, untouched here.
- Iteration 9 (user direction, reversing an earlier decision): the unattended-run
  prompt framing, first held back as a separate spec 012, is merged in here as
  User Story 5 and its own requirement group. The argument for merging: shipping
  the display without the framing would put conversational filler on screen, so
  the two halves only deliver value together. No spec 012 exists.
- Scope was narrowed on user direction after a first draft: no status taxonomy
  (succeeded/failed/blocked/timed out), no push notifications, no deduplication
  of repeated failures, no attention filter. The agent's own last response is
  the report. Both earlier [NEEDS CLARIFICATION] markers dissolved with that
  narrowing — there is no blocker-detection mechanism to design and no delivery
  destination to choose.
- A Problem section precedes the template's first mandatory section, stating the
  current behaviour being corrected. It contains no implementation detail.
- the relevant requirements and the relevant requirements are the two requirements that carry real risk of being
  skipped as "obvious"; they are stated explicitly so planning has to account
  for the in-progress and no-response states.

- Iteration 8 (user direction): response previews removed from both the task
  list and the agent-facing listing; the run's state carries the signal in a
  listing, and the response is read by opening a run. Consequence recorded
  below rather than silently absorbed.
- Known consequence of removing previews: a task whose agent reported it could
  not do the work reaches state `answered`, identical to one that worked, so a
  listing no longer flags problems — only *ran / running / never ran / produced
  nothing*. Distinguishing good news from bad in a listing would require the
  status taxonomy this spec deliberately does not have. Accepted; the task view
  is where outcomes are read.
- FR cross-references in these notes were replaced with group names after the
  requirement list was renumbered twice; numbers in notes went stale faster than
  they were worth maintaining.

- Framing trap recorded in both the edge cases and the assumptions, because the
  obvious implementation is the wrong one: the reflection cycle issues its work
  as the SAME request content kind a scheduled task uses (it carries its own
  framing already), so framing selected on content kind would double up. It must
  be selected on the run being a scheduled task.
- User Story 5 is P1 alongside User Story 1, deliberately. They are co-equal
  rather than ordered: the display is what makes a report visible, the framing is
  what makes it a report. Either alone leaves the original complaint unfixed.
