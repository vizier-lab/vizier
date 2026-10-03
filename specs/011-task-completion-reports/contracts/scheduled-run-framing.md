# Contract: Scheduled-Run Framing

**Feature**: `011-task-completion-reports` · User Story 5

What the agent is given when a task fires. Two parts: a system message that exists only for a
scheduled run, and a request frontmatter that stops implying a person sent it.

---

## Selection — on the channel, never on the content kind

```rust
session.is_scheduled_task()      // matches!(self.1, VizierChannelId::Task(..))
```

**This is the one place where the obvious implementation is wrong.** The dream cycle sends its
work as the same request content kind a scheduled task uses (`scheduler/dream/mod.rs:195`,
`:343`) while carrying its own framing in `EXTRACTION_PROMPT`. Selecting on the content kind would
therefore apply this framing to the dream cycle as well, which FR-031 forbids and US5 scenario 5
tests. A scheduled task is `VizierChannelId::Task`; a dream is `VizierChannelId::Dream`. The
channel distinguishes them; the content kind does not.

Two things keep this from being re-broken later. `VizierSession::is_scheduled_task()` gives the
question one home, so there is no second place to answer it from. And the content variant is
renamed `Task` → `Unattended` (research Decision 12), because two of its three construction sites
are the dream cycle — the old name invited precisely this mistake.

Interactive channels (`DiscordChanel`, `TelegramChannel`, `HTTP`) match nothing here and are
untouched (FR-032, US5 scenario 6).

---

## The system message

A `scheduled_run_md()` in `src/agents/agent/system_prompt/`, appended by
`prepare_system_prompts` — which gains the run kind as a parameter, since today it takes no
session at all (`agents/agent/mod.rs:282`).

```markdown
# SCHEDULED RUN

This turn is a scheduled task run, not a conversation. Nobody is reading as you
work, and nothing you write here reaches a person until they open the task later.

- **No questions.** Anything you ask goes unanswered and the run just ends. Where
  something is ambiguous, take the most reasonable reading, say which assumption
  you made, and continue.
- **Act, don't offer.** Use your tools to do the work now. "I can do X if you'd
  like" is a dead end here.
- **To reach someone, send to them.** If the task calls for telling or asking a
  person something, use a tool that delivers to them. Writing it in this turn
  does not.
- **Your last message is the report.** It is what a person sees when they open
  the task. Lead with the outcome, then what you did, and anything needing their
  attention — a failure, a blocker, a judgement call you had to make. No
  greeting, no sign-off, no offer of further help.
```

**Constant.** No task name, no requester, no timestamp — those are in the frontmatter. This is what
satisfies FR-033 and keeps the prefix reusable between runs.

**Placement**: appended last, after CORE and documents. Content, not ordering, is what keeps the
framing from touching the agent's character (FR-035) — but last is where situational context
belongs, and it leaves the boot/CORE prefix byte-identical to an interactive turn's.

**Scope**: it describes the situation. It says nothing about tone or persona; a warm agent writes a
warm report.

---

## The request frontmatter

`generate_frontmatter` (`schema/request.rs:239`) today emits the task's `user` as `sender`, so a
scheduled run arrives looking like a message a named person just sent. BOOT.md directive 4 then
tells the agent to *"Check channel metadata … to understand the interaction"*
(`system_prompt/boot.rs:16`) — and a scheduled run offers nothing to check, leaving that sender
name as the only social cue in the request. Models read it correctly and answer conversationally.

**Before**

```yaml
---
sender: alice
metadata:
  timestamp: 2026-10-04T09:00:00Z
---
Summarise merged PRs and post to #eng
```

**After**

```yaml
---
sender: scheduler
task: daily-report
requested_by: "@dani (DiscordId: 182…)"     # or: requested_by: self
metadata:
  timestamp: 2026-10-04T09:00:00Z
---
Summarise merged PRs and post to #eng
```

`requested_by` renders the requester this same feature introduces — the person's identity, or
`self` where the task was the agent's own initiative (FR-030). Who the work is for is kept,
because a report written for a specific person can be written for them; what is dropped is the
false claim that they sent it.

The instruction body is untouched. A person's task instruction stays exactly the text they wrote.
