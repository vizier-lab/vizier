# Contract: what the agent reads about a platform sender

## Request frontmatter

Every request built from a Discord or Telegram person goes through `platform_links::sender::resolve` + `stamp`. That covers chat, silent read, mention, slash or skill command, side chat, question answer, mid-turn steer and `client_event`-like arms.

Linked:

```yaml
sender: rin
verified: true
sent_at: 2026-11-27T14:02:11+07:00
metadata:
  platform: discord
  platform_user_id: "381926457021"
  platform_username: rin_dev
  platform_display_name: Rin
  # …the reader's existing keys (discord_channel_id, is_dm, is_reply_message, …)
```

Not linked:

```yaml
sender: Unregistered User
verified: false
sent_at: …
metadata:
  platform: telegram
  platform_user_id: "55120"
  platform_username: kai_k        # omitted when the account has no handle
  platform_display_name: Kai K
```

Rules:

- `verified` sits beside `sender` (keys are written in the frontmatter's usual alphabetical order) and appears **only** on platform requests. Web, scheduler, background, subagent, consultation and wake frontmatter is byte-identical to before.
- `sender` is the Vizier **username**, the same string a WebUI turn from that person shows.
- Requests saved before 069 render as saved: no `verified`, and the old `@name (DiscordId: …)` sender.
- Mentions inside message text stay `@alice (DiscordId: 2)`.

## OWNER.md

```text
# OWNER.md -- Your Owner

Below is the data of your owner:
vizier_username: "rin"
discord: "rin_dev" (id "381926457021"), verified link
alias: ["Rin"],
…
```

Only `platform_link` rows are listed. Hand-typed legacy values are never printed.

## BOOT.md (one added sentence)

> A Discord or Telegram message with `verified: true` comes from the Vizier user named in `sender`, proven by a linked account; `sender: Unregistered User` is someone with no linked account, and `metadata` says who they are on that platform.

## dummyplug §9 `sender` (test harness)

When the newest user message's command text, trimmed, is `sender` (case-insensitive), the reply is that message's frontmatter block verbatim (between the `---` lines), or `(no frontmatter)`. It is added to `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md` in the same change.
