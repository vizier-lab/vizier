# Questions and Sensitive Input

Sometimes an agent needs something from you rather than from a tool: a decision, a detail, or a credential. It has two ways to ask.

## Questions

An agent can ask up to ten questions at once: free text, choose one, choose several or yes/no. You answer them together in one submission.

- In the **WebUI** they appear as a card in the conversation. The card survives a reload, another tab or a server restart until you answer it.
- On **Discord** they are buttons and a select menu; on **Telegram** an inline keyboard per question. A set too large for a platform's controls falls back to a reply form that still answers everything in one go.
- Your answers reach the agent as your next message. Optional questions you leave blank are reported as skipped.
- A question set never expires.

## Sensitive input

When an agent needs a token or a password, it asks through a masked dialog.

- The value goes **only to the script that asked**. The agent does not see it, and it is not written to the conversation, the history or the logs: wherever it would appear, it shows as `[redacted]`.
- You can answer, decline, or dismiss the dialog (Escape, a press outside, or its close control). The agent is told which.
- Values must be 8 to 65,536 bytes. A shorter value is accepted but cannot be hidden by redaction, and the dialog warns you before you submit.
- It is available in the **WebUI and project chats only**. Discord and Telegram keep what they carry and show it to their own administrators, so there the agent asks in plain text instead (don't paste secrets there). It is not offered in scheduled runs, dreams or other unattended work.

Redaction replaces the exact text. It does not follow a value the agent transforms (encoded, hashed or split) and does not protect copies the agent writes itself, such as a file or a memory. To let an agent keep a credential for later, use [Agent Secrets](./secrets.md).

## What is kept

A question set and its answers are part of the conversation: they are deleted with it and visible to whoever can read it. For sensitive input only the fact that one was requested, and what came of it, is recorded, never the value.
