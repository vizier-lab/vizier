# Specification Quality Checklist: Asking the Person — Structured Questions and Sensitive Input

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

### Settled decisions

- **Two tools, one spec (2026-11-01).** `ask_question` (structured questions, **turn-ending**, answers arrive in the agent's next turn, universal across channels) and `ask_sensitive_input` (one sensitive value, **not** turn-ending, delivered to the running script only, WebUI-only dialog). Folded together on request; there is no separate sensitive-input spec.
- **Channel decision (option B).** Discord and Telegram present questions with their own interactive controls — buttons, select menus, inline keyboards, text-entry forms — with a reply-to text form only as the fallback for sets that exceed the platform's limits (FR-027). Feasibility checked in the tree: the Discord reader already receives interaction events (it handles slash commands today) and the workspace already depends on the builder crate its components need; the Telegram reader polls updates with an explicit allowed-update list, so callback queries must be added there. Neither needs a new dependency or extra bot permission (FR-031).
- **Names settled (2026-11-01).** The family is `ask_question` + `ask_sensitive_input`, verb-first and consistent, so the pair reads as one family in a tool listing. `ask_secret` rejected (`secret` means a *stored* secret here, and this tool stores nothing); `request_sensitive_input` rejected for the shared prefix. Checked against the tree: no existing or retired tool name starts with `ask`. Tool names are dispatch keys, so this rename would be breaking later.
- **Categories.** `questions` (its own category, per-turn, absent in unattended turns) for `ask_question`; the existing `secrets` category for `ask_sensitive_input`. Neither is part of the always-on toolset, so an agent that cannot ask never sees the category line.
- **Sensitive input is WebUI-only and shown as a dialog.** Reject action, and dismissing the dialog (Escape, outside press, close control) declines rather than leaving the ask open (FR-035, FR-036). The accidental-dismissal trade-off is accepted: the decline is branchable, so the agent can ask again but must not retry on its own in a loop.
- **The scrub mechanism is reused, and that is what forces the design.** Verified in the tree: `SecretRedactor` is per-turn, armed by `set_secret`/`get_secret`, and `execute_python` calls `redactor.report(&mut report)` before anything leaves the sandbox — the comment there is explicit that the log line, the returned report, its history entry and the model's copy are all scrubbed. A value armed at the moment the ask is answered inherits the whole guarantee with no new mechanism. A turn-ending ask would hand the value to the next turn, whose context is the copy that must stay clean — hence FR-033.
- **Two inherited limits stated, not hidden** (FR-043, FR-057, FR-058): redaction cannot arm a value shorter than `MIN_VALUE_BYTES` (8 bytes, deliberately, so short values do not corrupt ordinary report text) — a four-digit PIN is therefore not protectable and the ask must say so; and a value the script transforms or writes elsewhere is outside the guarantee.

### Carried into planning, not spec gaps

- How a pending question set is stored so channel controls resolve after a restart (FR-025), and how several questions are laid out when a platform allows only a few controls per message (FR-029).
- Whether the WebUI question card is drawn from the tool's saved call or from its own record (FR-011, FR-016).
- How the sensitive-input ask reaches the WebUI and returns an answer to a blocked script — the session event broadcast is one-way, so this needs a request/response path (FR-032 to FR-042).
- How a resumed turn seals an ask that was open when the agent restarted.

### Deliberately out of scope

Approval/permission gating for tool execution (issue #119), voice answers, editing an answer after submission, attachments in answers, questions that span conversations, and any secure input on channels that cannot provide one.
