# Consultations

An agent can **consult** another agent: send it a request, carry on, and get the answer back in the
same step or later. The consulted agent is a real, separate agent. It answers as itself, with its
own identity, memory, tools, integrations and provider, and its work is charged to it.

Consultations replace Workspaces (removed in this release). A consultation works like a
[subagent worker](./tools-embedding.md): it gets a handle at once, can be waited on, stopped and
asked questions by its consultee, outlives the turn that started it, and its outcome reaches the
asking agent exactly once. The difference is who does the work: a worker is a temporary copy of
the asking agent; a consultation goes to another agent.

## The tools

They form the `consultations` category, reached from `execute_python` like every other tool.

| Tool | What it does |
|---|---|
| `consult_agent` | `consultations: [{agent, request}]` (1 to 8) and an optional `timeout_secs` (1 to 3600, default 600). Returns one handle (`c_…`) per consultation at once. |
| `wait_consultations` | Waits until `any` or `all` (the default) of the given consultations has settled: answered, failed, timed out, stopped, or asking. |
| `reply_consultation` | Answers a consultation that is asking, or sends a **follow-up** to one that answered. The conversation continues with everything said before. |
| `stop_consultation` | Stops one consultation; the others carry on. |
| `list_consultations`, `get_consultation` | This conversation's consultations; one with its whole request and answer. |
| `list_consultable_agents` | The agents this conversation may consult. |
| `ask_consulting_agent` | Offered only to the consulted agent: asks the agent consulting it one question, and ends its step. |

An answer nobody waited for arrives as a message starting `[Consultation update]`, at the asking
agent's next step or as a new turn if it is idle. Every refusal is a `status` result the script can
read: `unknown_agent`, `self`, `in_chain`, `no_owner`, `unavailable`, `unknown_consultation`,
`not_waiting`, `invalid_input`.

## Who may be consulted

A consultation uses the consulted agent's tools and budget and shows it what the conversation is
about, so it follows the **person the asking agent is working for**:

- In a WebUI or project chat, that is the person in the chat. An agent may consult any agent that
  person may chat with (their view right on it).
- In a scheduled run, it is the asking agent's **owner**. An agent with no owner cannot consult.
- An agent the person may not use is answered exactly as one that does not exist.
- Access is checked again on every reply and follow-up.
- **Discord and Telegram:** not offered yet. Those identities are self-declared until verified
  channel identities land ([#116](https://github.com/vizier-lab/vizier/issues/116)), so nothing
  proves who is asking.

## Limits

```yaml
vizier:
  consultations:
    max_depth: 1   # default: a consulted agent cannot consult further
```

With the default, a consulted agent is offered only `ask_consulting_agent`. Raising `max_depth`
lets consultations chain; an agent already in the chain is still refused, so a loop cannot form.
Subagent workers cannot consult, and dreams never do. A consulted agent may hire its own workers.

## What the consulted agent sees

The consultation runs in a conversation of the consulted agent's own, one per consultation. Its
prompt is its usual one, with a short `# CONSULTATION` section at the end naming the asking agent.
Each request's frontmatter says `via: consultation` (then `consultation_reply` or
`consultation_followup`), `sender` is the asking agent, and `on_behalf_of` names the person, or
`none` in a scheduled run.

## In the WebUI

- **The asking chat** has a **Consultations** item in the status strip, beside Subagents: the
  number of open consultations, with an amber dot while one is asking. Its panel lists them by the
  call that started them, each led by the consulted agent; expanding one shows its steps and its
  answer. It is read-only.
- **The consulted agent's conversations** list "⇄ Asked by <agent>" to people who may **edit**
  that agent. It opens read-only: only the asking agent writes in it.
- Usage shows a consultation's model calls under the consulted agent, as **Consultations**.

## Deleting

Deleting the asking conversation, or the asking agent, stops its consultations and deletes the
consulted agent's copy of each. Deleting the consulted agent stops its open consultations, and
the asking agent is told.

## Upgrading from Workspaces

On the first start after upgrading, every Workspace is removed once (marker
`migration.drop_workspaces_v1`): each member agent's record of each topic, the Workspace
notifications, the `workspaces:create` and `all_workspaces:manage` permissions, and the tables.
Agents' memory, their other conversations and recorded usage stay; old usage reads "Workspace
(removed)". The old tool names (`mention_agent`, `send_message_to_workspace`, `list_workspaces`,
`read_workspace_messages`, `delegate_agent`) answer with a hint to consult instead.
