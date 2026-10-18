# Quickstart: User-Invokable Skills (dummyplug e2e)

**Feature**: `021-user-invokable-skills`

Checks against a running binary with an agent on the offline `dummyplug` provider (`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`). Dummyplug's `context` message echoes the per-request context block, which is where the invoked skill appears, because the model's message is the *input* after the command.

## 0. Setup

1. `just run`; create an agent with provider `dummyplug`; open topic **T1**.
2. Create a skill `release-notes` (description `Draft release notes`, content `Use bullet points.`) via Settings or `POST /api/v1/agents/{id}/skills`. Create a global one `standup` the same way.

## 1. Invoke with input (US1, FR-001..003)

In T1 send `/release-notes context`. Expect the echoed context block to contain `## Invoked Skill`, `release-notes` and `Use bullet points.`, and the history entry for the message to read `/release-notes context`.

## 2. Invoke without input

Send `/release-notes`. Expect a normal reply (model message `Run the skill.`) and no error.

## 3. Alternate form and global skill (FR-006, edge)

Send `/skill standup context`: the context block contains `standup`. Send `/Release-Notes context`: matches case-insensitively.

## 4. List (US2, FR-004)

Send `/skills`. Expect `release-notes — Draft release notes` and `standup — …`. In the WebUI, typing `/` in the composer also shows both. `GET /api/v1/agents/{id}/skills/available` returns the same two.

## 5. Not found (US3, FR-007, SC-004)

Send `/relase-notes`. Expect `No skill named "relase-notes".` and `Did you mean: release-notes`. Check the usage report: no new model call for the turn. Send `/etc/hosts`: ordinary dummyplug reply.

## 6. Built-in wins (FR-006)

Create a skill named `abort`. Send `/abort` (WebUI command) while a turn runs: the turn is aborted, the skill is not run. `/skill abort context` runs the skill.

## 7. Freshness (SC-005)

Delete `release-notes`; without restarting, `/release-notes` is now not found. Re-create it; it works again.

## 8. Telegram and Discord (channel steps need real bots, run after 1–7)

- Telegram DM and group: `/release-notes context` and `/skill release-notes context` run with no mention (group bot with privacy mode off); `/relase-notes` and `/skills` in the group do nothing unless addressed (`/skills@mybot`).
- Discord: `/release-notes` appears in the command picker under this bot; run it with `input` = `context`. With two bots in the guild that both have the skill, each shows its own `/release-notes` entry. Add a skill and within 60 s it shows in the picker; delete one and it disappears. A skill whose name Discord rejects (for example one with spaces or capitals beyond what the slug allows) is not registered, logs one warning, and still runs, with no mention, through `/skill` (the autocomplete lists it; `input` = `context`). Typed text in a DM also works.

Steps 1–7 use dummyplug alone; step 8 needs platform bot tokens and is marked as such.

## 9. Gates

`cargo clippy`, `cargo test` (parser and resolver tests), `cd webui && npm run typecheck`.
