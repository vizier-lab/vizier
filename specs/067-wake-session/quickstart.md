# Quickstart: verifying the session tools

These checks run offline against `dummyplug` agents (constitution: e2e through dummyplug). Each
step is a message sent on the WebUI socket or `POST /agents/{id}/prompt`, and its expected
outcome. In code mode every tool is reached through `execute_python`, so a "call" below is
`{"tool": "execute_python", "arguments": {"code": "…"}}` sent to the agent.

A woken agent is scripted the same way as any other: the instruction the caller sends **is** a
dummyplug message. So an instruction of `{"say": "pong"}` makes the woken agent answer `pong`, and
an instruction carrying a `{"tool": "execute_python", …}` JSON makes it run that tool.

The automated form of everything below is a case in `crates/vizier-core/tests/dummyplug_e2e.rs`.

## Setup

1. Run `just run`, which uses `dev.vizier.yaml`.
2. Create the dummyplug agent `alpha`, owned by `admin`.
3. Create a second user `viewer`, who may use `alpha` too, so the two can talk to the same agent.
4. Open a WebUI conversation with `alpha` as `admin`. Note its topic id — the checks call it
   `TOPIC`.

## 1. Listing the conversations (Story 2)

1. In `alpha`'s chat:
   ```python
   list_sessions()
   ```
   **Expect**: a list with `alpha`'s own WebUI conversation (`channel` starts `http__admin__`,
   `topic_id == TOPIC`, `title` non-empty), and **no** row for a conversation of machinery.
2. Narrow it:
   ```python
   list_sessions(channel="http__admin__vizier-webui")
   ```
   **Expect**: only the WebUI rows.
3. Search by title:
   ```python
   list_sessions(query="<a word from the title>")
   ```
   **Expect**: only the conversations whose title contains that word, case-insensitively.
4. Open a **second** WebUI conversation as `admin`, say one thing in it, then run `list_sessions()`
   as `admin` again. **Expect**: both conversations are listed — one row per **topic**, not one row
   per channel.
5. Have the same agent serve two people: as `admin` and as `viewer`, talk to `alpha` in the WebUI.
   Then as `admin`, run `list_sessions()`. **Expect**: only `admin`'s own conversation is listed —
   `viewer`'s is absent, and its title is never shown.
6. In a conversation where the bot has never replied (a Discord channel it is a member of but has
   not spoken in), run `list_sessions()`. **Expect**: that channel is **not** listed, because it has
   no session record — and waking it by its slug still works. The listing says what the agent can
   find; the wake's own check decides what may be woken.
7. Read the two tools' arguments in the agent's own tool list. **Expect**: neither has an
   `agent_id` argument — the calling agent is injected by the system.

## 2. Waking a conversation (Story 1, SC-001)

1. In `alpha`'s chat, wake `alpha`'s **own** WebUI conversation (the one from step 1):
   ```python
   wake_session(target="http__admin__vizier-webui", topic_id="TOPIC", instruction="{\"say\": \"pong\"}")
   ```
   **Expect**: the call returns at once with `channel: "http__admin__vizier-webui"`,
   `topic_id: "TOPIC"` and **no** `reply`.
2. Open `alpha`'s WebUI conversation. **Expect**: the woken turn's reply `pong` is **in that
   conversation**, after the request that woke it.
3. Read that conversation's history. **Expect**: the request that woke it is stored with the
   frontmatter `sender: alpha`, `via: wake`, and `sent_at`. The reply follows it.

## 3. What the woken turn sees (Story 1, FR-008, FR-009, FR-021)

1. In `alpha`'s chat, wake the WebUI conversation again with an instruction that asks the woken
   agent something only answerable from the conversation:
   ```python
   wake_session(target="http__admin__vizier-webui", topic_id="TOPIC", instruction="{\"say\": \"what did I ask you before?\"}")
   ```
   **Expect**: the woken turn's answer refers to the earlier message in that conversation, not to
   the caller's turn — the history is in view.
2. In the same turn, check the woken agent's tools. **Expect**: the woken turn is **not** offered
   `ask_question` or `ask_sensitive_input`. A woken turn has nobody to answer it.
3. Check the woken turn's own prompt. **Expect**: it carries the woken framing — the agent started
   this turn itself, the request is not a person's message, and nobody is waiting on the answer
   there.

## 4. Waiting for the answer (Story 5, SC-006)

1. In `alpha`'s chat:
   ```python
   wake_session(target="http__admin__vizier-webui", topic_id="TOPIC", instruction="{\"say\": \"later\", \"delay_ms\": 3000}", wait=True)
   ```
   **Expect**: the call returns after about 3 s with `reply == "later"`.
2. Repeat without `wait`. **Expect**: the call returns at once with `reply == null`, and `later`
   still appears in the target conversation a moment later.
3. Repeat with `wait=True` and abort the caller's turn while it waits. **Expect**: the wait ends,
   the woken turn carries on, and its answer still lands in the target conversation.

## 5. Access (Story 3, SC-002, SC-003)

1. As `admin`, wake **another person's** WebUI conversation — the one `viewer` has with `alpha`:
   ```python
   wake_session(target="http__viewer__vizier-webui", topic_id="<viewer's topic>", instruction="x")
   ```
   **Expect**: `{status: "other_person"}`.
2. As `admin`, wake a conversation the machinery owns — a scheduled run's own conversation, or a
   subagent worker's. **Expect**: `{status: "not_wakeable"}`, and the message names what owns it.
3. Wake an address that is not an address. **Expect**: `{status: "unknown_channel"}`.
4. Wake with an empty instruction. **Expect**: `{status: "bad_request"}`.
5. Try to wake a conversation of another agent. **Expect**: there is nothing to try — neither tool
   takes an agent id, so no conversation of another agent can be named at all.

## 6. Depth (SC-004)

1. From a woken turn, wake another conversation. **Expect**: the hop count rises, and a wake past
   the hop limit is refused with `{status: "too_deep"}`.
2. Wake a conversation that wakes the first one back. **Expect**: refused by the hop limit — there
   is one agent, so the chain has nothing to catch and the hop count is the guard.

## 7. A scheduled run putting its work in the person's thread (Story 4, SC-005)

1. As `admin`, create a one-time task whose instruction is:
   ```
   Wake my WebUI conversation (http__admin__vizier-webui, topic TOPIC) with the instruction
   {"say": "the daily report"} and wait for its answer.
   ```
2. Let the task fire. **Expect**: the work happens in the WebUI conversation, and `the daily
   report` is in **that** conversation's history — not in the task's own conversation.
3. Create a task with no conversation named. **Expect**: it behaves exactly as today: the work
   happens in the task's own conversation, and the result is a notification.
4. Inside the task's own conversation, call `list_sessions()` and then
   `wake_session(target="current", instruction="x")`. **Expect**: the task's own conversation is
   listed under `"current"`, and waking it is refused with `{status: "not_wakeable"}` — the
   scheduler owns it. A scheduled run reaches the person's conversation by naming it.

## 8. The trail in the WebUI

1. Open `alpha`'s WebUI conversation after a wake. **Expect**: the activity trail names the
   session tools — the category appears as its own entry, and the woken request is drawn as an
   agent's message, not as the person's.
