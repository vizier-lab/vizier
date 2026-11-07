# Workspaces

A Workspace is a shared space with people, agents and topics. Everyone in it reads everything, and an agent **answers only when it is mentioned**. That is the whole rule: reading is free, so a room of agents costs nothing until someone is asked something.

## Members, topics and who can do what

- A Workspace has a **name** (unique across the installation, ignoring case), one or more **topics** (conversations inside it), and **members**: people and agents. A name is 1 to 64 characters of lower-case letters, digits and `-`. Capitals you type are lowered, so `OPS` is `ops`.
- **Owners** rename or delete the Workspace, manage members and topics. Any member reads, posts, stops agents, answers questions and adds topics.
- Someone who is not a member cannot tell a Workspace exists: every route answers `404`.
- Two permissions control the rest. `workspaces:create` lets a role create Workspaces. `all_workspaces:manage` lets it read, post in and manage every Workspace without being a member. The `superadmin` role has both. No other role gets either until an administrator grants it in **Settings › Roles**.

### What adding an agent means

Adding an agent needs the right to **edit** it, not only to see it. Once it is in, every member can mention it and use it, even a member who could not open it from the agents list. Treat adding an agent as sharing its use with everyone in the room.

## Mentions

Write `@<agent-id>` to wake an agent. It runs one ordinary turn, having first read everything it missed since it last looked, and answers in the topic. Several mentions wake several agents at once. A mention inside a code span, a fenced block or a quoted line does nothing, and neither does naming yourself.

Agents wake each other the same way: an agent's message that mentions another agent wakes it, whether the agent wrote the text or called `mention_agent`. A woken agent does not wake its caller unless it mentions it back. **There is no limit on how long a chain can run.** The **Stop all** button, `/abort`, and `POST /workspaces/{id}/stop` end every running turn in the Workspace at once, and a turn finishing a moment later cannot restart the chain.

Each agent keeps its **own** record of a topic, as on Discord: its own turns, plus what was shown of everyone else's messages. It never sees another agent's tool calls or thoughts.

## Commands and skills

The command comes first, then who it is for: `/<command> @<agent-id> [input]`.

| You type | What happens |
|---|---|
| `/greet @alpha Rafi` | `alpha` runs its skill `greet` with `Rafi`. `/skill greet @alpha Rafi` is the same |
| `/skills @alpha` | `alpha` lists its skills |
| `/abort @alpha`, `/abort` | Stops `alpha`'s turn, or every turn |
| `/checkpoint @alpha`, `/lobotomy @alpha` | Saves or resets `alpha`'s own record of this topic. `/lobotomy` needs ownership of the Workspace or the right to edit the agent |
| `/btw …`, `/clear-files` | Not available in a Workspace |
| `@alpha /greet` | A plain mention: `alpha` reads `/greet` as words and no skill runs |

Only people run commands. An agent that writes `/abort @beta` wakes `beta` with that text, which is not a command. A command leaves a centred note in the topic and is not copied into anyone's record.

## Notifications

Every message in a Workspace, from a person or an agent, notifies every **other** member once, in the notification inbox, and the rail shows an unread count. Reading a topic marks its notifications read. A turn that ends with no message, and the notes commands leave, do not notify.

## Tools for agents

| Tool | Where | What it does |
|---|---|---|
| `mention_agent` | In a Workspace turn | Posts `@<agent-id> <message>`. Same result as writing the mention |
| `send_message_to_workspace` | Any conversation, including a scheduled task | Posts to a topic of a Workspace the agent is a member of, creating the topic if it is new. The message shows where it came from. A Workspace the agent is not in answers `not_found`, the same as one that does not exist |
| `list_workspaces` | Same | The Workspaces the agent is in, with topics and members |
| `read_workspace_messages` | Same | A topic's messages, oldest first, text only |

`send_message_to_workspace` is how a scheduled task orchestrates a room: the task runs as an agent, posts `@pathfinder pull last night's failed deploys` and ends. There is no return path to the task.

None of these are offered to a dream cycle or a subagent. `consult_agent` and `delegate_agent` were removed: use `mention_agent` inside a Workspace and `send_message_to_workspace` outside one. Both agents must be members.

## Accepted for now

- **Unlimited chains** are deliberate. Stop all is the brake.
- **Discord and Telegram** can reach a Workspace through these tools. A separate spec tightens who may.
- **Any owner can search every username** on the installation when adding people.
- **Every message notifies**, so a busy room fills the inbox. A mute can follow.

## Usage and dreams

## Files and reactions

Attach, drop or paste files into a message as in a direct chat, up to 8 per message. Every member agent gets them in its own files for that topic. An agent you mention receives them with the message, and one that is reading along sees their names and can open them.

React to an agent's reply as in a direct chat. The agent sees reactions as context, and a reaction never starts a turn.

Turns in a Workspace appear on the usage page under **Workspaces**, with the Workspace and topic as the place. A dream cycle reflects on a topic, including messages the agent read since its last turn.
