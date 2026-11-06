# Quickstart: Workspace Channel

End-to-end checks against a running binary with `dummyplug` agents (no keys, no network). `scripts/workspace-e2e.mjs` scripts the steps marked *script*. These require research R16's additive `say` key on dummyplug.

**Tool calls in this file are shorthand.** Every agent runs in code mode, so a model can only call `think`, `execute_python`, `list_tool_functions` and `describe_tool_function`. `{"tool":"mention_agent","arguments":A}` below means `{"tool":"execute_python","arguments":{"intent":"...","code":"mention_agent(...A...)"}}`, and `scripts/workspace-e2e.mjs` has one helper that builds it. With `say`, the key sits beside `tool` on that same outer object.

## Setup

```bash
just run
```

Create agents `alpha`, `beta` and `gamma` on provider `dummyplug` (`POST /api/v1/agents`, with `silent_read_initiative_chance` left at its default). Sign in as `owner`, who can edit all three. Create users `member` and `outsider`, neither with any agent permission.

## 1. Membership and access (script)

1. `POST /workspaces {name:"ops"}`. **Expect** a topic `general` and `owner` as owner.
2. Repeat with `OPS`. **Expect** `409 name_in_use`.
3. `GET /workspaces/{id}/candidates?q=al` as `owner`. **Expect** `alpha`, addable.
4. Add `alpha`, `beta` and `member`.
5. As `member`, `POST .../members` adding `gamma`. **Expect** a refusal.
6. As `outsider`, `GET /workspaces/{id}` and open the socket. **Expect** `404` from both.

## 2. Silent read costs nothing (script)

1. Note the `usage_event` count. As `owner`, post `the billing lock is back` 5 times, with no mention.
2. **Expect** 5 `workspace_entry` frames on `member`'s socket, no reply, no new usage rows, and no `session_turn` rows.
3. **Expect** no `session_history` rows yet for `alpha` in this topic (research R2).

## 3. A mention wakes one agent, who has read the rest (script)

1. Post `@alpha context`.
2. **Expect** a reply from `alpha` only, its `## Workspace` section naming the members and the mention rule, and `workspace_mentions` with `alpha: woken`.
3. **Expect** `alpha`'s session history to hold the 5 messages as `SilentRead` before the request, in order.
4. Post `@alpha context` again. **Expect** no duplicates (position moved).

## 4. Several at once, and what doesn't count (script)

1. Post `@alpha @beta tools`. **Expect** each replies once, both running at once.
2. Post `@alpha @alpha tools`. **Expect** one activation.
3. Post `` `@alpha` and > @alpha and @nobody ``, with the second on a quoted line. **Expect** no activation and plain text.

## 5. Mention wakes, however it is written (script)

1. Post `@alpha ` followed by `{"tool":"mention_agent","arguments":{"agents":["beta"],"message":"tools"}}`.
2. **Expect** an entry attributed to `alpha`, with no history row, and `beta` woken.
3. Post `@alpha ` followed by `{"say":"asking @beta now","tool":"current_time","arguments":{}}`.
4. **Expect** a mid-turn entry attributed to `alpha`, `beta` woken before `alpha`'s turn ends, and `alpha`'s final message after it.
5. **Expect** `alpha` did not wake when `beta` replied without mentioning it, and that its next turn's history holds `beta`'s reply.
6. With `mention_agent` naming `gamma`, **expect** `status: not_a_member` and no entry.

## 5a. Each agent's record (script)

1. After step 5, read `alpha`'s and `beta`'s session history for the topic.
2. **Expect** `alpha`'s record to hold `beta`'s messages as `SilentRead` and none of `beta`'s `ToolCall`, `ToolResult` or thought rows, and the same the other way round.
3. **Expect** `workspace_entry` to hold only messages, with no tool calls.

## 5b. Skills and commands (script)

Install a skill `greet` on `alpha` only.

1. Post `/greet @alpha Rafi`. **Expect** `alpha` runs it, with input `Rafi`, and `metadata.invoked_skill` on its request.
2. Post `/greet @beta`. **Expect** an unknown-skill reply from `beta`, with no model call and no usage row.
3. Post `/skills @alpha`. **Expect** a list, with no usage row.
4. Post `@alpha /greet` and `/greet please @alpha`. **Expect** `alpha` wakes each time as an ordinary mention, with no skill invoked (research R19).
5. Post `/greet` with no mention. **Expect** a plain message that wakes no one.
6. Post `/checkpoint @alpha`. **Expect** a `note` entry, a checkpoint in `alpha`'s record, and nothing in `beta`'s. On `beta`'s next activation, **expect** no copy of the note.
7. As `member` (not an owner, and not able to edit `alpha`), post `/lobotomy @alpha`. **Expect** `403 cannot_lobotomy`. As `owner`, **expect** it works.
8. Post `/btw @alpha hello` and `/clear-files`. **Expect** `422 not_in_workspace`, and nothing posted.
9. Have `alpha` post `/abort @beta` through `mention_agent` while `beta` runs a slow tool. **Expect** `beta`'s running turn to continue.
10. `GET /workspaces/{id}/commands` as `member`. **Expect** `greet` listed with `agents: ["alpha"]`.

## 6. Stop all ends a chain (script)

1. With `{"say":"@beta go","tool":"current_time"}`-style messages, have `alpha` and `beta` trigger each other, using a slow tool.
2. `POST /workspaces/{id}/stop`. **Expect** within 5 seconds no turn running or queued in the Workspace, and `dropped` mention rows.
3. Wait 10 seconds. **Expect** no new entries. Post `@alpha tools`. **Expect** `alpha` answers.

## 7. Restart (script)

1. Start a slow tool under `alpha`. Kill the process. Restart. **Expect** the turn resumes and no read or mention repeats.
2. Stop the process right after a mid-turn mention is saved, using a test hook or a `pending` row inserted by hand. **Expect** the mention is dispatched at startup.

## 8. Orchestrating from outside (script)

1. In a direct chat with `alpha`, send `{"tool":"list_workspaces","arguments":{}}`. **Expect** only `ops`.
2. Create a one-time task on `alpha` whose run sends `{"tool":"send_message_to_workspace","arguments":{"workspace_name":"ops","workspace_topic":"daily","message":"@beta tools"}}`. A dummyplug task prompt is the JSON itself.
3. **Expect** topic `daily` created, origin `task`, and `beta` woken.
4. As `gamma` (not a member), send the same. **Expect** `not_found`, the same as for a made-up name.
5. Confirm that a dream session's tool list lacks all four `workspaces` tools.

## 9. Old tools are gone (script)

1. In any chat, send `tools`. **Expect** no `consult_agent` or `delegate_agent`.
2. Run a script calling `consult_agent`. **Expect** a catchable error with the new guidance.
3. On a database with saved delegation history, **expect** old conversations and jobs to read as before.

## 10. Every message notifies (script)

1. With `member` on no topic socket, as `owner` post `hello`. **Expect** one `workspace_message` notification for `member`, linking to the entry, and none for `owner`.
2. Post `{"say":"working on it @member","tool":"current_time","arguments":{}}` to `alpha` with a leading mention. **Expect** two more for `member`, one mid-turn and one final, and two for `owner`, with `@member` highlighted but no extra notification.
3. `POST .../topics/{tid}/read` as `member`. **Expect** the topic's and the Workspace's unread counts at 0.
4. With `member`'s socket open on the topic and marking read, post again. **Expect** no unread left behind.
5. Run `/checkpoint @alpha`. **Expect** no notification for the note.
6. `DELETE` `member` from the Workspace. **Expect** their socket gets `closed` within 1 second, and `GET /notifications` no longer lists its rows.
7. Add `outsider` as a member, then have `alpha` reply. **Expect** `outsider` is notified, though they can't view `alpha`.

## 11. Usage and dreams (script)

1. `GET` the usage report. **Expect** activity **Workspaces**, with place labels.
2. Run a dream cycle on `alpha`. **Expect** it reflects on the topic, including messages read since its last activation.

## 12. Mid-turn messages in any chat (browser)

Open the Mid-turn messages tab of [mockup.html](mockup.html) beside the app.

1. In a direct chat with `alpha`, send `{"say":"checking first","tool":"current_time","arguments":{}}`. **Expect** a trail, the message, then the final message, live.
2. Reload. **Expect** the same.
3. Open a conversation saved before this feature that has mid-turn text. **Expect** it renders this way too.
4. Confirm the same text is not posted to a Discord channel. *(Needs a Discord token. Skip without one and rely on the unchanged reader path.)*

## 13. Screens (browser)

Check against [mockup.html](mockup.html), in both themes, at desktop width and at 375 px:

1. Rail: Home, separator, Workspace tiles, separator, agents, one `+`. Unread count and working dot on a tile. The `+` asks Workspace or Agent. With only `workspaces:create` or only `all_agents:create`, it goes straight to that form, and with neither it is absent.
2. Topic screen: other people on the left, mention highlight, no line for a successful mention, a note only for an unavailable one, origin badge, repeated agent head, a centred note for a command.
3. Stop all: appears only while working, and leaves the stopped note.
4. Members: search first, capped results, addable first, the warning before adding an agent, Esc and × close it.
5. Composer: `@` menu with agents and people, arrow keys, Enter and Esc. `/` at the start lists commands and skills. After choosing one, `@` offers only the agents that have it.
6. New workspace: the duplicate-name error.
7. A real model, *live provider*: check that agents use the mention convention sensibly, and that filler text is tolerable.
