# Contract: `/connect` on the bots

`connect` is added to both platforms' built-in command lists, with the description "link this account to your Vizier user". A skill named `connect` is skipped there, the same way skills that clash with other built-ins already are.

Wording lives in `platform_links/text.rs`. `{code}` is shown as `XXXX-XXXX`.

## Discord: the slash command `/connect`

The reply is always an **ephemeral** interaction response, in a guild or a DM.

With `base_url` set:

```text
Link this Discord account to your Vizier user:
{base_url}/connect#{code}

Or enter {code} in Vizier under Settings › Profile › Connected accounts.
The code works once and expires in 10 minutes. Only use a code you asked for yourself.
```

Without `base_url`, the link line is left out.

Rate-limited (more than 5 in 10 minutes for this account):

```text
Too many codes. Try again in {n} minutes.
```

## Telegram: `/connect` or `/connect@bot`

- **Private chat**: the same text as Discord, with "Telegram" in place of "Discord", sent as a reply in that chat.
- **Group or supergroup**: no code is issued or posted. The reply is:

  ```text
  Send /connect to me in a direct message: https://t.me/{bot_username}
  ```

## Never

- A code is never posted where anyone but the requester can read it.
- A code is never logged. Logs record `connect code issued platform=discord`.
- `/connect` is never forwarded to the agent as a chat turn.
