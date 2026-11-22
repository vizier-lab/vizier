# Consultations

An agent can **consult** another agent: send it a request, carry on, and get the answer back in the same step or later. The consulted agent is a real, separate agent. It answers as itself, with its own identity, memory, tools, integrations and provider, and its work is charged to it.

A consultation works like a [subagent worker](./agents.md#subagents): the asking agent can carry on meanwhile, the consulted agent can ask a question back, the work outlives the turn that started it, and the outcome reaches the asking agent exactly once. The difference is who does the work: a worker is a temporary copy of the asking agent, while a consultation goes to **another agent**. Consultations replace the older Workspaces feature, which was removed.

An agent can send up to eight consultations at once, optionally with a time limit (up to an hour, ten minutes by default), wait for any or all of them, answer a question the consulted agent asks back, send a follow-up after an answer, or stop one.

## Who may be consulted

A consultation uses the consulted agent's tools and budget and shows it what the conversation is about, so it follows the **person the asking agent is working for**:

- In a WebUI or project chat, that is the person in the chat. An agent may consult any agent that person may chat with.
- In a scheduled run, it is the asking agent's **owner**. An agent with no owner cannot consult.
- An agent the person may not use is answered exactly as one that does not exist.
- Access is checked again on every reply and follow-up.
- **Discord and Telegram:** not offered yet. Those identities are self-declared until verified channel identities land, so nothing proves who is asking.

## Limits

Consultations do not chain: a consulted agent cannot consult a third agent (fixed, not a setting). Subagent workers cannot consult, and dreams never do. A consulted agent may hire its own workers.

## What the consulted agent sees

The consultation runs in a conversation of the consulted agent's own, one per consultation. Its prompt is its usual one, with a short section naming the asking agent and the person it acts for.

## In the WebUI

- **The asking chat** has a **Consultations** item in the status strip, beside Subagents: the number of open consultations, with an amber dot while one is asking. Its panel lists them, each led by the consulted agent; expanding one shows its steps and its answer. It is read-only.
- **The consulted agent's conversations** list "⇄ Asked by <agent>" to people who may **edit** that agent. It opens read-only.
- **Usage** shows a consultation's model calls under the consulted agent, as **Consultations**.

## Deleting

Deleting the asking conversation, or the asking agent, stops its consultations and deletes the consulted agent's copy of each. Deleting the consulted agent stops its open consultations, and the asking agent is told.

## Upgrading from Workspaces

On the first start after upgrading, every Workspace is removed once: each member agent's record of the topics, the Workspace notifications, the Workspace permissions and the tables. Agents' memory, their other conversations and recorded usage stay; old usage reads "Workspaces (removed)".
