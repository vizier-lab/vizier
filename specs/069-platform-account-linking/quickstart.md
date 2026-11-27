# Quickstart: checking platform account linking

## 0. Automated

```sh
cargo test -p vizier-core platform_links            # codes, resolve, stamp, owner_md, storage contract
cargo test -p vizier-core --test dummyplug_e2e connect_code
cargo test -p vizier-http --test platform_links      # routes, refusals, limits, unlink, user delete
cargo test -p vizier-http --test conformance
cd webui && npm run typecheck && npx vitest run lib/connect
```

`dummyplug_e2e` cases (offline, no platform). Each builds a platform request with the same `resolve` + `stamp` the readers use, sends it to a dummyplug agent, sends `sender`, and checks the reply:

| Case | Setup | Expect in the `sender` reply |
|---|---|---|
| unregistered | no link for discord `552013874411` | `sender: Unregistered User`, `verified: false`, `platform_user_id: '552013874411'` |
| linked | issue + redeem a code for discord `381926457021` as `rin` | `sender: rin`, `verified: true`, `platform_username: rin_dev` |
| unlink is immediate | then `DELETE …/connections/discord` | the next request reads `Unregistered User` |
| web unchanged | a WebUI chat request | no `verified` line |
| owner | unit tests in `system_prompt/user.rs` (dummyplug doesn't echo system prompts) | OWNER.md lists `discord: "rin_dev" (id "381926457021"), verified link`; a hand-typed id is absent |

## Checked without a bot (Windows)

`vizier-http`'s `dev_serve` test serves the real runtime and WebUI where the `vizier` binary does not build. With a code written into `platform_connect_code` in the dev workspace (its SHA-256, as the bot would store it), `/connect#CODE` signed out → sign-in → confirm, the profile card (replace, unlink, remove an unverified id, expired code, phone and light theme) and the sender names of a Discord conversation were checked in a browser on 2026-11-27.

## 1. Live: Discord (needs a bot token)

1. `just run` with `base_url: http://localhost:9999` set in `dev.vizier.yaml`. Use a dummyplug agent with a Discord integration.
2. In a guild channel, run `/connect`. **Expect** an ephemeral reply ("Only you can see this") with a link and a `XXXX-XXXX` code.
3. Sign out of the WebUI and open the link. **Expect** `/login`, then the confirmation after sign-in. The address bar never shows the code after the first load.
4. Confirm. **Expect** Settings › Profile › Connected accounts lists Discord with the handle and id.
5. Mention the bot with `sender`. **Expect** `sender: <your username>` and `verified: true`.
6. Open the same link again. **Expect** "This code has expired or was already used. Send /connect again."
7. From a second Discord account, mention the bot with `sender`. **Expect** `Unregistered User` and that account's id.
8. Unlink from the row menu (⋯ › Unlink, confirm). Mention `sender` again. **Expect** `Unregistered User`.

## 2. Live: Telegram (needs a bot token)

1. In a group, send `/connect`. **Expect** "Send /connect to me in a direct message: https://t.me/…" and no code.
2. In the DM, send `/connect`. **Expect** a code, and a link when `base_url` is set.
3. Unset `base_url`, restart, and send `/connect` in the DM again. **Expect** a code only. Enter it in Settings › Profile › Connected accounts and confirm.
4. In the group, send `@bot sender`. **Expect** your Vizier username, and `platform_user_id` equal to **your** Telegram id, not the group's (the chat-id fix).

## 3. Upgrade path

1. On a master build, type a Discord id into the profile. Switch to this branch and run.
2. **Expect** the profile to show it under "Not verified — send /connect to confirm". A message from that id reads `Unregistered User`.
3. Connect that account. **Expect** the not-verified row to be gone and a connected row in its place.
