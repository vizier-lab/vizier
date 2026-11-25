# Entity Links

A conversation and a memory can point straight at the agent's work with **entity wiki links**: `[[kind:name]]`, written as plain text by the agent or by you, rendered as clickable links in the WebUI.

## Syntax

| Form | Meaning |
|------|---------|
| `[[artifact:name]]` | One of the agent's artifacts, by name |
| `[[function:name]]` | One of its functions (custom tools), by name |
| `[[skill:slug]]` | One of its skills, by slug (agent or global scope) |
| `[[task:slug]]` | One of its scheduled tasks, by slug |
| `[[kind:name\|label]]` | The same, shown as `label` |
| `[[bundle/slug]]`, `[[bundle]]` | Memory links — a different thing entirely; see [Memory](/docs/configuration/memory) |

Rules the renderers agree on:

- A link is addressed **within the conversation's agent** — names are agent-scoped, never absolute.
- The kind must be one of the four reserved words and the target must be non-empty; anything else (`[[flavour:vanilla]]`, an unclosed `[[`) renders as the text it is.
- Text inside code blocks and inline code is never linkified.
- A target that no longer exists renders **broken** — visibly struck through, going nowhere — and never breaks the text around it.
- A link is a reference only: nothing is attached to the message, nothing runs when it is sent, and following it enforces the usual permissions (a link grants nothing).

## Where links come from

- **Agents** are taught the syntax in their operating doctrine and cite their work with it — an artifact they just wrote, the function they called, the task they scheduled. No tool call is involved; it is plain text in the reply, so it works on every channel (only the WebUI renders it).
- **You** type `[[` in the chat composer and pick from the suggestion list: the agent's artifacts, functions, skills, tasks, and memory bundles/documents, filtered as you type. Typing a kind prefix (`[[skill:`) narrows the list to that kind. Escape closes the list; picking inserts the complete link.

## How rendering resolves

Links render from the stored text, derived at display time — nothing is rewritten in history. The WebUI keeps a small per-agent **name index** (`GET /api/v1/agents/{id}/link-targets`): names only, with artifacts capped to the latest 200 and memory documents to the latest 500 so the payload stays small however much the agent accumulates. A link the index cannot settle is confirmed by one batched exact-existence check (`POST …/link-targets/resolve`) before it is drawn broken. The composer falls back to a server-side search (`?kind=&q=`) when the local index was cut short or comes up empty.

Because resolution is derived, links survive restarts with no migration, and deleting or renaming an entity turns its old references into the broken state rather than dead ends.

## Permissions

The index, search and resolve endpoints use the same agent view permission as every other agent read route, and a caller without artifact view permission sees no artifact names at all. The pages behind the links are the ordinary artifacts, functions, skills and tasks pages with their usual rules.
