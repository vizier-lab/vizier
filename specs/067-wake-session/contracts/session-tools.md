# Contract: the session tools

The two tools of spec 067, their arguments, their results, the addresses they accept, the reasons
they refuse, and what a woken turn looks like from the inside.

The tools live in their own category, **`sessions`**, appended to the tool-category list (FR-025).

---

## `list_sessions`

Lists the conversations the calling turn may reach.

### Arguments

| Name | Type | Required | Meaning |
|---|---|---|---|
| `agent_id` | string | no | Whose conversations to list. Omit for the calling agent |
| `channel` | string | no | Only conversations on this channel slug |
| `query` | string | no | Only conversations whose title contains this, case-insensitively |

Both filters may be given together. An empty or absent filter means no narrowing.

### Result

An array of rows, one per conversation:

| Field | Type | Meaning |
|---|---|---|
| `agent_id` | string | the agent that owns the conversation |
| `channel` | string | the channel slug — what `wake_session` takes as its `target` |
| `topic_id` | string \| null | the topic, for the channels that have topics |
| `title` | string | the conversation's title |
| `is_thinking` | boolean | whether a turn is running in it |

### Rules

1. The calling turn's own conversation is **always** in the list (FR-005). It is wakeable under
   the name `"current"` when it is a wakeable kind; in a conversation the machinery owns — a
   scheduled run's own conversation — it is listed and refused.
2. Only conversations on a **wakeable** channel are listed: a WebUI conversation, a project chat, a
   Discord channel or a Telegram chat (FR-014, FR-015). A scheduled run's, a worker's, a
   consultation's or a dream cycle's conversation is not in the list, and cannot be woken.
3. Another agent's conversations are in the list only when the person this turn is working for may
   use that agent — the same rule the consultation list uses (FR-016, FR-017).
4. A WebUI or project conversation of **another person** is not in the list (FR-018). One agent may
   serve several people, and every one of that agent's conversations is in the stored rows, so
   without this filter the listing would hand one person the titles of another person's
   conversations.
5. A title that is empty matches nothing under `query`; a conversation with no title is listed
   without one.

### What the listing is, and is not

A conversation is in the listing when it has a session record, and a session record is created the
first time a turn runs in it (or when the agent creates a conversation empty). So the listing holds
**the conversations the agent has been in** — not every channel a bot is a member of. A Discord
channel the bot has never replied in is not listed, and is still wakeable by its slug.

The listing is therefore a **convenience, not the authority**: the wake's own access check decides
what may be woken, and it does not require the conversation to have been listed.

---

## `wake_session`

Starts a turn in a named conversation.

### Arguments

| Name | Type | Required | Meaning |
|---|---|---|---|
| `target` | string | yes | `"current"`, or a channel slug |
| `agent_id` | string | no | The agent that owns the target conversation. Omit for the calling agent |
| `topic_id` | string | no | The topic within the target channel, for the channels that have topics |
| `instruction` | string | yes | What the woken agent should do |
| `wait` | boolean | no | Wait for the woken turn's answer and return it. Default `false` |

### Result

| Field | Type | Meaning |
|---|---|---|
| `agent_id` | string | the agent that was woken |
| `channel` | string | the channel it was woken in |
| `topic_id` | string \| null | the topic it was woken in |
| `reply` | string \| null | the woken turn's answer, when `wait` was true |

### The addresses

```
current                        the calling turn's own conversation
discord__<channel id>          a Discord channel          (no topic)
telegram__<chat id>            a Telegram chat            (no topic)
http__<user>__<channel>        a WebUI conversation       (topic required)
project__<user>__<project id>  a project chat             (topic required)
```

Anything else is refused. The address is the **channel slug** — the same string the conversation
picker lists and the WebUI's route carries — so a value taken from `list_sessions` is accepted
unchanged.

`topic_id` is required for the two channels that have topics. For the two that do not, it is
ignored.

### Rules

1. The call returns **at once**, with the target's address and no answer, unless `wait` is true
   (FR-012).
2. A wake that asks to wait returns the woken turn's **answer** in the same call (FR-013).
3. The woken turn runs as the **target conversation's agent** — its identity, memory, tools,
   integrations, provider and configured prompt — in that conversation, with that conversation's
   earlier turns in view (FR-008).
4. The woken turn's reply is saved into the target conversation and shown there, and the request
   that woke it appears in that conversation's trail attributed to the waking agent (FR-010).
5. A woken turn **waits behind** a turn already running in the target conversation, as a person's
   message does (FR-011).
6. A woken turn is **not interactive**: the question tools and the sensitive-input tool are not
   offered in it (FR-023).
7. A refusal is returned as **data**, never as a failure of the call (FR-026).

### The refusals

```json
{ "status": "not_wakeable", "message": "that is a scheduled run's own conversation; it belongs to the scheduler" }
```

| Status | Raised when |
|---|---|
| `unknown_channel` | the target is not an address the system knows |
| `not_wakeable` | the target names a conversation the system's own machinery owns (FR-015) |
| `unknown_agent` | the agent does not exist, **or** the person may not use it — one answer for both (FR-017) |
| `no_owner` | the turn has no person whose access could decide (FR-019) |
| `other_person` | the target's conversation belongs to somebody else (FR-018) |
| `in_chain` | the target's agent is already above this turn (FR-022) |
| `too_deep` | the hop limit is reached (FR-021) |
| `bad_request` | the instruction is empty, or the address is malformed |

A refusal never says more than its status: an agent that may not be reached is answered exactly as
an agent that does not exist (FR-017).

---

## What the woken turn sees

### Its request, in the conversation

The woken turn's request is stored in the target conversation's history like any other request,
and rendered in the WebUI as a message that is **not** the person's — the way a worker's task is
rendered, not the way a person's message is.

### Its frontmatter

```yaml
sender: <the waking agent's id>
via: wake
sent_at: 2026-11-26T09:14:03
metadata: {}
```

The `sender` is the **agent** that asked, never a person's name. `via: wake` tells the reader the
message is not from the person in the conversation.

### Its turn

- The conversation's history is in view (FR-008).
- The turn is **not** interactive: no question tool, no sensitive-input tool (FR-023).
- The turn's reply notifies the person's inbox, once, in a WebUI or project conversation (FR-010).

### The agent's own instruction

The `instruction` is the work. The woken agent is told, by the framing that accompanies the
request, that the request came from another agent, that it is not a person's message, and that
nobody is waiting on the answer in that conversation.

---

## The WebUI's side of the contract

The WebUI keeps its own copy of which built-in tool is in which category
(`webui/app/lib/toolCategories.ts`), and `scripts/trail-tool-categories.test.mjs` fails when the
copy and the engine disagree. Adding the category therefore means:

1. `BuiltinCategory` gains `'sessions'`.
2. `BUILTIN_TOOLS` gains `list_sessions: 'sessions'` and `wake_session: 'sessions'`.
3. `webui/app/lib/toolIcons.ts` gains the category's label, icon and phrase, so the activity trail
   says what the agent did ("worked in another conversation").
4. The request-content union in `webui/app/api/frames.ts` gains the woken kind, so the trail and
   the message list render it as an agent's message rather than the person's.
5. `webui/openapi.json` and `webui/app/api/schema.d.ts` are regenerated.

There is **no new screen, no new panel and no new layout**, so the design-first rule is not
triggered: the feature adds a tool the trail names and a message the chat renders.
