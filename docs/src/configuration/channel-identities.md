# Connected Accounts

An agent on Discord or Telegram is told who sent each message. Until a person **connects** their platform account to their Vizier user, the agent reads them as **Unregistered User**. Once connected, it reads their Vizier username, marked as verified.

A connection is proof: only someone who can read the bot's private reply **and** is signed in to Vizier can make one. Ids typed into a profile prove nothing and are no longer used.

## Connect an account

1. On Discord or Telegram, send `/connect` to any agent's bot.
2. The bot answers privately with a **code** such as `ABCD-EFGH`, and a **link** when Vizier knows its public address (see below).
3. Open the link, or enter the code under **Settings › Profile › Connected accounts**. Sign in if asked.
4. Check the account shown and **confirm**.

- **Private only.** On Discord the reply is visible to you alone, in a server or a direct message. In a Telegram group the bot gives no code; send `/connect` to it in a direct message.
- **One use, ten minutes.** A code works once and expires after 10 minutes. Sending `/connect` again cancels the earlier code. One account can ask for 5 codes in 10 minutes, and a person or address can try 10 codes in 10 minutes.
- **Only confirm your own code.** The confirmation names the account being connected and your Vizier user. A link someone else sent you would connect **their** account to **you**, making their messages read as yours.
- **One account per platform.** A Vizier user has at most one Discord and one Telegram account; connecting another replaces it. An account connected to someone else is refused.
- **Every agent.** The connection belongs to the account, not to one bot, so it applies to every agent on that platform, from the next message.

## The public address

The link needs to know where people reach Vizier. Set it with `--base-url`, `VIZIER_BASE_URL` or in the config file (the desktop app has it on its Desktop settings page):

```yaml
vizier:
  base_url: "https://vizier.example.com"
```

Without it the bot replies with the code only, which always works. The code sits after the `#` in the link, so it never reaches server or proxy logs.

## What the agent reads

```yaml
# Connected                          # Not connected
sender: rin                          sender: Unregistered User
verified: true                       verified: false
metadata:                            metadata:
  platform: discord                    platform: discord
  platform_user_id: "381926457021"     platform_user_id: "552013874411"
  platform_username: rin_dev           platform_username: kai.k
  platform_display_name: Rin           platform_display_name: Kai
```

The platform id and names are always there, so an agent can address people by their platform name and tell two unconnected people apart. The agent's owner is recognised only through the owner's own connected accounts.

Messages saved before this feature keep the sender they were saved with.

Where you read a Discord or Telegram conversation in the WebUI, a connected person shows as their Vizier username marked **connected**; anyone else shows by their platform name marked **unregistered**.

## Unlink

On **Settings › Profile › Connected accounts**, open an account's menu and choose **Unlink**. Its next message reads as Unregistered User. Deleting a Vizier user removes their connections.

## Upgrading

Discord and Telegram ids typed into a profile before this feature are shown as **Not verified** and identify nobody. Connect the account with `/connect` to replace them, or remove them from the profile.
