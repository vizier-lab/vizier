# Sharing Conversations

A person with the **share conversations** permission can turn a conversation into a frozen snapshot, either a link on this instance or an offline `.vizier.html` file. The snapshot is read-only and does not change when the conversation, its files or its agent change later.

## Who can share

Two permissions, both in the "Sharing" group:

- `owned_agents:share_conversations`: conversations with agents the person owns.
- `all_agents:share_conversations`: conversations with any agent the person can see.

This permission is the **only** control over who can create shares of any audience, public links included. There is no separate instance-wide switch. If you do not want public links, give the permission only to people you trust.

## Audience

Chosen at creation, one per share, and never widened afterwards:

- **Specific people**: named users of the instance.
- **Anyone signed in** on this instance.
- **Anyone with the link**: no sign-in needed.

A link nobody may open, a revoked link and a link that never existed all look the same to someone not signed in (a sign-in request), and the same to someone signed in (one neutral page).

## What is included

- The people's messages and the agent's replies, in order.
- Images the conversation shows, carried inside the snapshot (an item over 1 MiB or missing becomes a labelled placeholder).
- Interactive pages the agent produced, in a form that only changes what the viewer sees. Actions that would message the agent, run a command, touch a file or use the network are inert.
- The agent's name and avatar. No user names or ids.
- The activity trail (tool calls and results), **only if** the person switches it on; it is off by default.

## What is never included

Regardless of the trail switch, removal happens in the person's browser **before** upload and is stored that way, so reading the raw data finds nothing:

- Recalled memory passages and internal reasoning.
- The values of stored secrets or sensitive inputs.
- Text shaped like a credential (API keys, tokens and similar), replaced with `[removed: <kind>]` in messages, tool arguments and tool results.
- Directory parts of absolute server paths.

The server never relies on this alone, and the viewer never trusts that it was done: it treats every snapshot as untrusted input.

## Reviewing before sharing

Before anything is stored or downloaded the dialog shows a preview: message count, images and interactive pages, whether the trail is included, total size, and how many removals were made, by kind. A snapshot over 5 MiB is refused with the size, the limit and the options that reduce it. Nothing is truncated silently. A conversation without messages cannot be shared.

Creation is limited to 10 per hour per person and 200 active shares.

## Revoking

Revoking a share is for its author and for administrators (system role or `all_agents:share_conversations`), and for no one else.

What it can do:

- Stop the link working at the origin at once.
- Stop it working in shared caches (such as Cloudflare) within 5 minutes.

What it cannot do:

- Recall a copy someone already saw, saved, or downloaded, including a `.vizier.html` file. Offline files are outside the instance and cannot be revoked.
- Change the audience. To narrow access, revoke and create a new share.

Deleting a user also deletes their shares.

## Availability

Everything above is available in the WebUI: the **Share** button in a chat (Link and Offline file tabs, with the audience choice and a people picker that searches usernames of enabled users), the sign-in round trip for a link opened signed out, and **Settings › Shared conversations** to copy or revoke your links (administrators can switch to all shares of the instance). The same operations are in the [Shared Chat API](../api-integration/shares.md).

An "anyone with the link" share asks for a second, explicit confirmation before it can be created, because anyone holding the link can read it without signing in. The page asks search engines not to list it and gives link previews no title, excerpt, author or picture.

Forking a shared conversation into your own agent is not part of this feature; it is planned as a separate "import conversation" spec.

For caching behind Cloudflare, see the API page.
