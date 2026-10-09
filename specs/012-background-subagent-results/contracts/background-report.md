# Contract: Background Report

**Feature**: `012-background-subagent-results`

A report is the request that wakes the originating conversation. It is stored in history as `SessionHistoryContent::Request` with `content: {"background_report": BackgroundReport}`.

## What the model sees

The output of `VizierRequest::to_prompt()` for a report:

````text
---
sender: background
job: b-7f3a9c
job_kind: batch
metadata: {}
---

# Background report: batch b-7f3a9c

This is not a message from a person. It reports the outcome of background work you started
earlier in this conversation. Act on it: tell the person if they are waiting on it, continue
the work, or do nothing if no follow-up is needed.

## 1. answered — Research topic A
<answer text>

## 2. answered — Research topic B
<answer text, cut at 4000 characters> … [truncated]

## 3. timed out — Research topic C
No answer within 600s.
````

The rendering rules:

- **Headings**: one `##` section per piece, in the original task order. The heading reads `<n>. <state> — <prompt, first line, max 80 characters>`, where `n` is 1-based.
- **States**: `answered`, `failed` or `timed out`. A delivered report never contains `running` or `interrupted`.
- **Truncation**: the body is cut at 4000 characters and ends with ` … [truncated]` when it was cut.
- **Delegations**: the title is `# Background report: delegation b-91c2e0 to <agent_id>`, and the single section's heading uses the delegated prompt.

## Frontmatter

| Key | Value |
|---|---|
| `sender` | `background`, never a person's name |
| `job` | the job id |
| `job_kind` | `batch` \| `delegation` |
| `metadata` | `{}` |

## Turn behaviour

- A report runs through the same path as a `Chat` turn. It has session history and auto-context, and `ThinkingStart` is sent.
- It is queued behind a running turn in its session like any other request (`session_queues`, `process.rs:454`).
- It carries `background_depth = job.depth + 1`.
