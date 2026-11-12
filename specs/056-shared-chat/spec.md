# Feature Specification: Shared Conversations

**Feature Branch**: `056-shared-chat`

**Created**: 2026-11-11

**Status**: Draft

**Input**: User description: "Shared conversations: let a person share a chat as a frozen, read-only snapshot, through a link whose audience they choose, and as a single `.vizier.html` file that opens offline." Source: issue #187 and the maintainer's two review comments on it, which override the issue where they differ.

## Scope in one paragraph

A person shares one of their chats with an agent as a **snapshot**: a frozen, read-only copy. They can share it as a **link**, choosing who may open it (named people on this Vizier instance, anyone signed in to this instance, or anyone with the link), or as a single **offline file** (`.vizier.html`) that opens in a browser with no connection. The agent's activity trail is left out unless the person asks for it, and certain things are never shared at all. This spec covers **sharing only**. Continuing a shared conversation with one's own agent (the issue's "fork", which the maintainer reframed as "import conversation") is a separate, later spec and is not specified here.

## Clarifications

### Session 2026-11-11

The product owner was not available. Each question was answered with the most conservative option (least data exposed, least privilege, least surprise) and may be overruled.

- Q: Beyond choosing the audience, what guards the creation of an "anyone with the link" share, and may such a share be listed or summarised by services outside the instance? → A: A second, explicit confirmation that states anyone holding the link can read it without signing in. The page asks not to be listed by search engines and gives no title, excerpt, author or picture to link previews. The one permission still covers all three audiences.
- Q: When naming specific people, what can the search reveal about the instance's users, and to whom? → A: Only to a person who may share, only after at least 2 typed characters, at most 10 matches per search, each showing display name and username and nothing else, limited to 30 searches per minute.
- Q: What does a viewer of a share learn about its audience and its other viewers? → A: Nothing. The shared view shows neither the kind of audience, nor the named people, nor how many there are, nor who else has opened it.
- Q: What does the administrative list show about a share the administrator is not in the audience of? → A: Author, agent, kind of audience, number of named people, date and size. Not the title (it is derived from the conversation) and not who the named people are.
- Q: Who may read the record of share actions, and are openings of a share recorded? → A: The author reads the record of their own shares. An administrator reads every record under the same limits as the administrative list. Openings are not recorded per viewer.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Share a chat as a link with a chosen audience (Priority: P1)

A person has a conversation worth showing: an answer, some generated images, a 3D scene. They press Share, see exactly what will be shared, choose who may open it, and get a link. People in that audience open the link and read the conversation as it was at that moment. Anyone else gets nothing.

**Why this priority**: This is the feature. The audience choice is the maintainer's first requirement, and a link that cannot be limited to the right people is not safe to ship.

**Independent Test**: With three accounts (author, a named colleague, an unrelated user) and a signed-out browser, create one share of each audience from the same chat and open each link as each viewer. Compare the results with the access table in this spec.

**Acceptance Scenarios**:

1. **Given** a person who may share and a chat with at least one completed exchange, **When** they choose Share, **Then** they see a preview of the snapshot and a choice of three audiences, and nothing has been stored or sent anywhere yet.
2. **Given** the preview, **When** they choose "specific people", pick two people and confirm, **Then** they get a link, and only they and those two people, signed in, can open it.
3. **Given** the preview, **When** they choose "anyone on this instance" and confirm, **Then** every signed-in user of the instance can open the link and a signed-out visitor cannot.
4. **Given** the preview, **When** they choose "anyone with the link" and confirm, **Then** they are asked once more, with a statement that anyone holding the link can read it without signing in, and only after agreeing do they get a link, which opens for a signed-out visitor in a browser that has never visited the instance.
5. **Given** a visitor who is not in a share's audience, **When** they open the link, **Then** they see the same "not available" page they would see for a link that never existed, with nothing that reveals the share, its title, its author or its audience.
6. **Given** a signed-out visitor on the "not available" page, **When** they sign in as a person who is in the audience, **Then** they land on the shared conversation.
7. **Given** an open shared conversation, **When** the visitor looks for ways to act on it, **Then** there is no way to send a message, react, edit, or reach the original chat or any of the author's other chats, and nothing shows who else may open it or has opened it.
8. **Given** a person without the permission to share, **When** they open a chat, **Then** no Share action is offered, and a direct attempt to create a share is refused.

---

### User Story 2 - Control what the snapshot contains (Priority: P1)

A chat holds more than its messages: tool calls and their results, steps the agent took, memories it recalled, its reasoning. Some of that is sensitive. By default a share contains only the conversation itself. The person may choose to include the activity trail, and whatever they choose, secrets, server paths, recalled personal memory and internal reasoning never leave. Before anything is stored or downloaded, they see what will be shared and what was removed.

**Why this priority**: The maintainer asked for the option because the trail "may contain sensitive information". Sharing without this protection can leak credentials or private memory with one click, and a leak cannot be taken back.

**Independent Test**: Prepare a chat whose trail contains a stored agent secret, a recognisable API key, an absolute path on the server, a recalled memory and a reasoning step. Share it once with the trail off and once with it on, as a link and as a file. Search both snapshots, as delivered to a viewer, for each planted item.

**Acceptance Scenarios**:

1. **Given** a chat with tool calls, **When** the person opens Share, **Then** "include the agent's activity" is off, and the preview shows messages, pictures and interactive results only.
2. **Given** the option turned on, **When** the preview updates, **Then** it shows the tool calls and their results that will be shared, and still shows no recalled memory and no internal reasoning.
3. **Given** a chat in which a secret appears in a message or a tool result, **When** the preview is shown with either setting, **Then** the secret is replaced by a visible "removed" marker and the summary counts it.
4. **Given** the preview's summary, **When** the person reads it, **Then** it states, by category, how many items were removed or replaced (activity steps, secrets, server paths, recalled memory, reasoning, files that could not be included).
5. **Given** a completed share, **When** a viewer inspects everything delivered to them, not only what is drawn on screen, **Then** none of the removed material is present.
6. **Given** a person who closes the preview without confirming, **When** they check their shares, **Then** nothing was created and nothing was downloaded.

---

### User Story 3 - Save a chat as one offline file (Priority: P1)

A person wants to keep or send a conversation without depending on a server: to a client, into an archive, to someone with no account. They download a single `.vizier.html` file. Anyone can open it in a browser, with no connection and nothing to install, and see the conversation with its pictures and interactive results working.

**Why this priority**: It is half of the request, and it is the only way to share from the desktop app, whose local engine has no address others can reach.

**Independent Test**: Download the file from a chat that has text, an image, a 3D scene and a chart. Disconnect the machine from every network, open the file in a current browser and use each interactive result. Reconnect, open it again while watching network activity, and confirm that it contacts nothing.

**Acceptance Scenarios**:

1. **Given** the Share preview, **When** the person chooses the offline file and confirms, **Then** one file named after the conversation and ending in `.vizier.html` is downloaded, and no link share is created.
2. **Given** that file on a machine with no network, **When** it is opened in a current desktop or mobile browser, **Then** the whole conversation is readable, every included picture is shown and every interactive result responds.
3. **Given** that file on a connected machine, **When** it is opened and used, **Then** it makes no network request unless the reader deliberately follows a link in the conversation.
4. **Given** the person is about to download, **When** they confirm, **Then** they have been told that a file, once sent, cannot be revoked or limited to an audience.
5. **Given** the desktop app, **When** the person chooses Share, **Then** the offline file is offered, and link sharing is absent with a one-line explanation.
6. **Given** the same chat and the same options, **When** a link share and an offline file are both made, **Then** both show the same conversation content.

---

### User Story 4 - Interactive results stay live, and harmless (Priority: P2)

Agents produce things people can touch: a 3D scene to orbit, a 2D canvas, charts, tabbed widgets, small pages. In a shared conversation they still respond. What they cannot do is anything beyond changing what is shown: a shared view has no agent, no terminal, no files and no network behind it, even if the conversation it came from was written to attack its readers.

**Why this priority**: Live results are what make a shared Vizier conversation worth showing. But a shared view is opened by people who never chose to trust the author, some of them signed in to the same instance, so it has to be safe before it is impressive.

**Independent Test**: Share a chat containing one of each interactive kind, and a second chat built from a set of hostile inputs (markup in messages and titles, results that try to send messages, run commands, read files, call the network or escape their frame). Open both as a signed-in user of the instance and as an offline file, and record every effect outside the result's own display.

**Acceptance Scenarios**:

1. **Given** a shared conversation with a 3D scene, **When** a viewer drags it, **Then** it rotates as it does in the original chat.
2. **Given** a shared interactive result with tabs, inputs or a chart, **When** the viewer uses them, **Then** the display changes for that viewer only, and nothing is saved or sent.
3. **Given** an interactive result with a control that, in the original chat, sends a message to the agent or triggers any action outside the page, **When** a viewer presses it, **Then** nothing happens beyond a visible note that this is a read-only copy.
4. **Given** a conversation whose text or data contains markup or script meant to run in the reader's browser, **When** it is shared and opened as a link or as a file, **Then** it is shown as text and nothing runs.
5. **Given** a result that includes author-written logic, **When** it runs in a shared view, **Then** it runs isolated from the page, the viewer's session and the network, under a time limit, and if that isolation is not available it does not run and the result is shown static.
6. **Given** a viewer signed in to the instance who opens a hostile share, **When** the page has loaded, **Then** nothing was done in that viewer's name and none of their data was read.

---

### User Story 5 - See and revoke what you have shared (Priority: P2)

A person can see every link they have shared, with its audience and date, open it, copy it again, take people off it, and revoke it. A revoked link stops working. An administrator can see that shares exist and revoke any of them, without being able to read conversations that were not shared with them.

**Why this priority**: Sharing without a way back is a liability. It ranks below the first three stories only because they must exist before there is anything to revoke.

**Independent Test**: Create three shares, revoke one, and open all three links as an allowed viewer immediately and again after the stated delay. As an administrator outside the audience of a "specific people" share, list shares, try to open that one, then revoke it.

**Acceptance Scenarios**:

1. **Given** a person with link shares, **When** they open their list of shares, **Then** each shows its title, agent, audience, date, size and whether activity was included, with actions to open, copy the link, download the offline file and revoke.
2. **Given** a share for specific people or for the instance, **When** its author revokes it, **Then** the very next attempt to open it gets the "not available" page.
3. **Given** a public share, **When** its author revokes it, **Then** within 5 minutes no new visitor anywhere can open it.
4. **Given** a share for specific people, **When** its author removes one of them, **Then** that person's next attempt gets the "not available" page. **When** the author tries to add a person or change the audience to a wider one, **Then** that is not offered, and they are pointed to creating a new share.
5. **Given** a chat that has live shares, **When** its author opens it, **Then** the chat shows that it is shared and leads to those shares.
6. **Given** an administrator, **When** they list shares on the instance, **Then** they see each share's author, agent, kind of audience, number of named people, date and size, and can revoke any. They see a share's title and the names of its named people only when they are in its audience. Opening a share they are not in the audience of gets the "not available" page.
7. **Given** an administrator revokes someone's share, **When** that happens, **Then** the author is told which share was revoked.
8. **Given** a share exists, **When** its author edits, continues, clears or deletes the original chat, or the agent is deleted, **Then** the share shows exactly what it showed when it was created.

---

### Audience by viewer

What each viewer gets when opening a link share. "Not available" is one response, identical in every cell where it appears.

| Viewer | Specific people | Anyone on this instance | Anyone with the link | Revoked, or never existed |
|---|---|---|---|---|
| The author, signed in | Read, manage | Read, manage | Read, manage | Not available |
| A named person, signed in | Read | Read | Read | Not available |
| Any other signed-in user | Not available | Read | Read | Not available |
| A signed-out visitor | Not available, may sign in | Not available, may sign in | Read | Not available, may sign in |
| An administrator outside the audience | Not available as a reader. Sees metadata in the administrative list and may revoke | Read. Sees metadata and may revoke | Read. Sees metadata and may revoke | Not available |
| A person who has the offline file | Reads the file. No audience applies and nothing can be revoked | same | same | same |

Reading a share needs no permission to share and no access to the agent the conversation was with. "May sign in" is offered on every "not available" page shown to a signed-out visitor, so its presence says nothing about the link.

### Edge Cases

- **Huge conversation**: a snapshot over the size limit is not created and not cut short. The person is told the size, the limit and what would reduce it (leave out activity, leave out pictures and audio). If it is still over, sharing is refused with those numbers.
- **Empty conversation**: a chat with no completed exchange, or one where nothing is left after removal, cannot be shared. The Share action says why.
- **Running turn**: a share contains completed exchanges only. If a turn is running or queued, the preview says that the reply in progress is not included. A question card the agent asked and nobody answered is shown as unanswered and cannot be answered from the share.
- **Unicode**: right-to-left text, combined emoji, scripts without spaces and very long unbroken strings are shown as in the original, in the title, the messages and the downloaded file's name. Characters a file name cannot hold are replaced, never the cause of a failed download.
- **Hostile content**: a conversation written to attack its readers (markup, script, misleading links, results that try to act) does nothing in a shared view beyond displaying. Links show where they lead before a reader follows them.
- **Two simultaneous Share clicks**: confirming twice in quick succession, or in a double-clicked dialog, creates one share and one link. Confirming again later, on purpose, creates a second independent share.
- **Tampered offline file**: Vizier cannot vouch for a file that has left it. The file says who saved it and when, and that it is an unverified copy. An edited file gains no capability: it holds no credentials and has no way to reach any Vizier instance.
- **Deleted author**: when an author's account is deleted, their link shares are revoked. Files already downloaded are unaffected.
- **Author loses the right to share, or access to the agent**: existing shares keep working, and the author can still list and revoke them. They cannot create new ones.
- **Named person's account is deleted**: they are removed from the audience. A share whose audience becomes empty stays readable by its author until revoked.
- **Agent deleted, or chat deleted**: shares stay, since a snapshot does not depend on its source.
- **A viewer who already has the page open** when a share is revoked keeps what is already on their screen. Revocation governs new visits.
- **File that cannot be included**: an image over the limit that cannot be reduced enough, a vector image, a video or another kind of file is replaced by a labelled placeholder naming the file and the reason, and is counted in the preview.
- **Picture deleted from the original chat before sharing**: it is shown as unavailable, as it is in the chat. After sharing, deleting it from the chat changes nothing in the share.
- **Share limit reached**: the person is told which limit, its value and how to free room (revoke a share).
- **Guessing links**: repeated attempts on links that do not exist are slowed, and every attempt gets the same "not available" response.
- **Project chat**: the project's folder location on disk is an absolute server path, so it never appears. File contents an agent read or wrote appear only inside the activity trail, so only when the author included it.
- **Instance address changes or the instance goes away**: links stop resolving. Offline files keep working.

## Requirements *(mandatory)*

### Functional Requirements

**Who may share, and what**

- **FR-001**: The system MUST add a permission to share conversations, in the two scopes used by other agent permissions: `owned_agents:share_conversations` (conversations with agents the person owns) and `all_agents:share_conversations` (conversations with any agent the person can use). Without one of them, no way to share MUST be offered and every attempt MUST be refused.
- **FR-002**: A person MUST be able to share only conversations that are their own. No permission lets a person share someone else's conversation.
- **FR-003**: Sharing MUST be available for a person's chats with agents in the WebUI and the desktop app, project chats included. Conversations on Discord, Telegram and in Workspaces, and task, dream, subagent and inter-agent conversations, MUST NOT be shareable.
- **FR-004**: A conversation with no completed exchange, or with nothing left after removal (FR-015 to FR-018), MUST NOT be shareable, and the person MUST be told why.

**The snapshot**

- **FR-005**: A share MUST be a snapshot: a copy of the conversation as it was when the person confirmed. It MUST NOT change afterwards for any reason, including edits to, continuation of, clearing of or deletion of the original conversation, deletion of its files, and deletion of the agent.
- **FR-006**: A snapshot MUST contain the conversation's completed exchanges in order: each message with who wrote it (the person or the agent) and when, pictures and audio that are part of it, interactive results, and question cards with the answers given. A running or queued turn MUST be left out and the preview MUST say so.
- **FR-007**: A snapshot MUST carry a title (the conversation's by default, editable before confirming), the agent's name, and the date it was shared. It MUST carry the author's display name only if the author chose to show it. That choice is on by default for the two instance audiences and off by default for a public share and for an offline file.
- **FR-008**: A snapshot MUST NOT contain or lead to anything outside itself: no reference that a viewer could use to reach the original conversation, the author's other conversations, the agent's memory, files or settings.
- **FR-009**: A snapshot MUST be self-describing and structured, not only a rendered page: it MUST record its format version, that it came from a conversation with a named agent, the order and authorship of every entry, and which entries are activity rather than messages. A link share and an offline file made with the same options MUST hold the same snapshot. This is what lets a later feature import a shared conversation, and present it to another agent as clearly delimited history that is not its own, without changing what was shared.
- **FR-010**: A snapshot written in an older format version MUST stay readable after Vizier is upgraded.

**Audience**

- **FR-011**: When creating a link share, the person MUST choose exactly one audience: specific people on this instance, anyone signed in to this instance, or anyone with the link. No audience MUST be preselected in a way that lets a public share be created without choosing it.
- **FR-053**: Creating an "anyone with the link" share MUST require a second, explicit confirmation, separate from the choice of audience, stating that anyone who holds the link can read the conversation without signing in. Declining it MUST create nothing. The permission to share (FR-001) covers all three audiences: no further permission or setting is needed.
- **FR-054**: A public share MUST ask not to be listed or kept by search engines and other automated collectors, and MUST offer no descriptive summary of itself (title, excerpt, author, picture) for link previews. Its content is given only to a visitor who opens the link.
- **FR-012**: For "specific people", the person MUST be able to find people by name or username through a search, without browsing a full list of users, and name up to 50. The search MUST be available only to a person who may share, MUST return nothing until at least 2 characters are typed, MUST return at most 10 matches, each showing a display name and a username and nothing else about that person, and MUST be limited to 30 searches per minute per person.
- **FR-013**: Access MUST follow the "Audience by viewer" table. Reading a share MUST NOT require the permission to share or access to the agent.
- **FR-014**: A visitor who may not read a share MUST get one "not available" response that is identical whether the share is restricted, revoked or never existed, in what is shown and in anything else the visitor can observe about the response. It MUST NOT reveal the title, author, agent or audience. Shown to a signed-out visitor, it MUST always offer signing in, and signing in as an allowed person MUST lead to the share.

**What is left out**

- **FR-015**: The agent's activity trail (tool calls and their results, intermediate steps) MUST be left out by default. The person MUST be able to choose to include it, per share.
- **FR-016**: Whatever the person chooses, a snapshot MUST NOT contain: secrets (values in the agent's secret store, values entered as sensitive input, and text recognisable as an API key, token, password or private key), absolute paths on the server, memory the agent recalled, or the agent's internal reasoning. This MUST hold for messages, activity, interactive results, file names and titles alike.
- **FR-017**: Removal MUST happen before the snapshot is stored or downloaded, so removed material is never held in a share and never delivered to a viewer in any form, visible or not.
- **FR-018**: Nothing MUST be cut silently. Removed text inside something that is otherwise shared MUST be replaced by a visible marker, and a file that cannot be included by a labelled placeholder naming it and the reason.
- **FR-019**: Before a share is created or a file downloaded, the person MUST see a preview: the conversation as viewers will see it, the options in effect, the size, and a summary counting what was removed or replaced by category. Changing an option MUST update the preview. Until the person confirms, nothing MUST be stored or downloaded.
- **FR-020**: The preview MUST tell the person that automatic removal cannot recognise every sensitive thing they or the agent may have written, and that they are responsible for reading it before sharing.

**Pictures, audio and files**

- **FR-021**: Pictures and audio in the conversation MUST be carried inside the snapshot, so a share has no asset that can break later. Only flat (raster) pictures are carried. Vector images MUST NOT be carried as images.
- **FR-022**: A picture or audio clip MUST be at most 1 MB as carried. A larger picture MUST be reduced to fit and marked as reduced. One that cannot be, a larger audio clip, a vector image, a video and any other kind of file MUST be replaced by a placeholder (FR-018).
- **FR-023**: The person MUST be able to leave out all pictures and audio from a share, as a way to reduce its size.

**Interactive results**

- **FR-024**: Interactive results (3D scenes, 2D canvases, charts, widgets, pages) MUST remain usable in a shared view, by link and in the offline file, with the same display behaviour as in the original chat.
- **FR-025**: In a shared view, an interactive result MUST be able to change only what that viewer sees. It MUST NOT be able to send a message, reach an agent, run a command, read or write a file, make a network request, or read anything belonging to the viewer or the instance. Controls that would do so MUST be inert and say that this is a read-only copy.
- **FR-026**: Author-supplied logic in an interactive result MUST run only in isolation from the page, from the viewer's session and stored data, and from the network, under a time limit. Where that isolation cannot be provided, the logic MUST NOT run and the result MUST be shown static.
- **FR-027**: Everything from the conversation MUST be treated as data when a share is displayed. No text, title, name or embedded data MUST be able to become active markup or script, in the link view or in the offline file, including data crafted to break out of the place it is embedded in.
- **FR-028**: A shared view opened by a signed-in user of the instance MUST NOT be able to act as that user or read their data, whatever the snapshot contains.

**The shared view**

- **FR-029**: A shared view MUST be read-only: no composer, no reactions, no editing, no continuing the conversation, and no import or fork in this version.
- **FR-030**: A shared view MUST state that it is a shared copy, when it was shared, the agent it was with, and (if shown) by whom, and MUST be usable on a phone-sized screen and with a keyboard and a screen reader. It MUST tell a viewer nothing about the share's audience or its other viewers: not the kind of audience, not the named people, not how many there are, and not who else has opened it. Only the author sees a share's audience.
- **FR-031**: Links in a shared conversation MUST be followed only by a deliberate action of the viewer, with the destination shown first.

**The offline file**

- **FR-032**: The person MUST be able to download a snapshot as one file whose name ends in `.vizier.html`, named after the conversation, that opens in a current browser with no connection, no account and nothing installed.
- **FR-033**: The offline file MUST be complete in itself and MUST make no network request when opened or used, other than a link the reader deliberately follows.
- **FR-034**: The offline file MUST hold no credential, session or address that could be used to reach the instance it came from or the author's account.
- **FR-035**: The offline file MUST state who saved it (if shown) and when, and that it is an unverified copy that Vizier cannot vouch for or revoke. Before downloading, the person MUST be told that a file cannot be revoked or limited to an audience.
- **FR-036**: Downloading an offline file MUST NOT create a link share. The author of a link share MUST be able to download that share's snapshot as an offline file. Viewers are not offered a download in this version.
- **FR-037**: In the desktop app, sharing MUST offer the offline file only, and say in one line why links are not available there.

**Managing shares**

- **FR-038**: A person MUST be able to list their link shares (title, agent, audience, date, size, whether activity is included), open each, copy its link and revoke it. A chat that has live shares MUST show its author that it is shared.
- **FR-039**: Revoking a share MUST delete its snapshot and cannot be undone. A share for specific people or for the instance MUST stop opening from the next attempt. A public share MUST stop opening for every new visitor within 5 minutes.
- **FR-040**: A public share MUST NOT be kept by any intermediary or browser for longer than 5 minutes without checking again. A share with a restricted audience MUST NOT be kept by any shared intermediary at all.
- **FR-041**: A share's audience MUST NOT be widened after creation: no change from a narrower audience to a wider one, and no adding people. The author MUST be able to remove named people, with effect from that person's next attempt. To reach more people, the person creates a new share.
- **FR-042**: Shares MUST NOT expire by themselves.
- **FR-043**: A holder of `all_agents:share_conversations` (an administrator, for this feature) MUST be able to list every link share on the instance by its metadata (author, agent, kind of audience, number of named people, date, size) and revoke any. That list and that right MUST NOT give access to any snapshot's content: the administrator reads a share only if the audience includes them. For a share whose audience does not include the administrator, the list MUST NOT show the title, which is derived from the conversation, or who the named people are.
- **FR-044**: When someone other than the author revokes a share, the author MUST be told which share it was.
- **FR-045**: Deleting a person's account MUST revoke their link shares and remove them from the audience of other people's shares. Losing the permission to share, or access to the agent, MUST leave existing shares working and still manageable by their author.

**Limits**

- **FR-046**: A link share's snapshot MUST be at most 5 MB, and an offline file's conversation content at most 20 MB. A snapshot over the limit MUST be refused, never shortened, with the size, the limit and the options that reduce it.
- **FR-047**: A person MUST be able to create at most 10 link shares per hour and hold at most 200 live link shares and 250 MB of snapshots in total. A refusal MUST name the limit, its value and how to free room.
- **FR-048**: A conversation of up to 2,000 messages MUST be shareable when it fits the size limit.
- **FR-049**: A link's identifier MUST be unguessable (at least 120 bits of randomness) and MUST NOT be derived from the conversation, the author or the time. Attempts to open links that are not available MUST be limited to 60 per minute per visitor, with the same response throughout.
- **FR-050**: Confirming the same Share dialog more than once MUST create exactly one share.

**Consistency**

- **FR-051**: Every rule above MUST hold identically however a share is requested or opened, including direct requests that bypass the interface. The one honest exception is secrets: the server refuses (`422 secret_detected`) only a snapshot holding a PEM private key or a vendor-prefixed token it can recognise with near certainty. It is a last-resort backstop, does NOT redact, and does not guarantee a clean snapshot; removing secrets stays with the client and the person's review.
- **FR-052**: Creating a share, revoking one and changing its named people MUST be recorded with who did it and when, without recording the snapshot's content. The author MUST be able to read the record of their own shares. An administrator (FR-043) MUST be able to read every record, under the same limits as the administrative list: no title and no names of named people for a share whose audience does not include them. Openings of a share MUST NOT be recorded per viewer, and nobody, the author included, is shown who opened a share.

### Key Entities

- **Share**: one act of sharing by link. Has an author, the agent and conversation it came from (for the author's own reference only), a title, an audience, whether activity is included, whether the author's name is shown, a creation date, a size, an unguessable link, and a state (live or revoked). Belongs to exactly one author.
- **Snapshot**: the frozen content of a share or of an offline file. Versioned and self-describing: ordered entries, each a message, an activity step or a placeholder, with author role and time, plus carried pictures and audio and interactive results. Independent of the conversation it was taken from.
- **Audience**: one of three kinds (specific people, the instance, public). For specific people, the list of named people, which can only shrink.
- **Removal summary**: what the preview reports, by category, about what was left out or replaced. Shown to the author before confirming. Not part of what viewers receive beyond the visible markers and placeholders.
- **Offline file**: a snapshot packaged with everything needed to display it, as one `.vizier.html` file. Not tracked by the instance after download.
- **Permission to share**: `owned_agents:share_conversations` and `all_agents:share_conversations`. The second also gives the administrative list and revocation.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person can go from an open chat to a copied link, including reading the preview and choosing an audience, in under 30 seconds on their first try.
- **SC-002**: Across every combination in the "Audience by viewer" table, 100% of attempts get the stated outcome, and the "not available" responses for restricted, revoked and non-existent links cannot be told apart by a tester who compares them.
- **SC-003**: In a test set of conversations seeded with at least 50 secrets of varied kinds, server paths, recalled memories and reasoning steps, 0 seeded items are found anywhere in what a viewer receives, with activity on and off, by link and by file.
- **SC-004**: With activity left off, 0 tool calls, tool results or intermediate steps are present in what a viewer receives.
- **SC-005**: 100% of what was removed or replaced is accounted for in the preview summary and by a visible marker or placeholder. Testers find no case of content missing without a trace.
- **SC-006**: An offline file opens and is fully usable with the network disconnected in current versions of the four most used desktop and mobile browsers, and makes 0 network requests when opened and used while connected.
- **SC-007**: In a test set of at least 100 hostile conversations, 0 cause any effect outside their own display, in the link view as a signed-in user and in the offline file: nothing sent, nothing run, nothing read, no request made, no markup breakout.
- **SC-008**: Every interactive result kind that works in a chat responds in a shared view, within the same time it takes in the original chat.
- **SC-009**: A revoked restricted share is refused on the next attempt in 100% of cases, and a revoked public share is refused for every new visitor within 5 minutes.
- **SC-010**: After the original chat, its files and its agent are deleted, 100% of existing shares of it display unchanged.
- **SC-011**: A public share of a 200-message conversation with 10 pictures is readable within 3 seconds of opening the link on an ordinary connection, and the preview for it appears within 3 seconds of pressing Share.
- **SC-012**: 1,000 visitors opening one public share within a minute do not slow an ongoing chat on the same instance by more than 10%.
- **SC-013**: An administrator outside a share's audience obtains 0 bytes of its conversation content, and neither its title nor the names of its named people, through any surface, while being able to list and revoke it.
- **SC-014**: Every stated limit (5 MB, 20 MB, 1 MB per picture or clip, 10 per hour, 200 live, 250 MB, 50 named people, 10 matches and 30 searches per minute when finding people, 60 failed attempts per minute) is enforced, and each refusal names the limit and its value.
- **SC-015**: Two confirmations of one Share dialog within one second produce exactly one share in 100% of trials.
- **SC-016**: A snapshot holds enough to rebuild the conversation's order, authorship and message text exactly, checked by comparing a shared snapshot with its source for a set of test conversations, so a later import needs no change to existing shares.

## Assumptions

- **Fork is a separate spec**: as the maintainer asked, this spec is sharing only. Nothing here lets a viewer continue, import or fork a conversation, drag a file into Vizier, or seed an agent with shared history. The only obligation carried for that later work is FR-009, FR-010 and SC-016: the snapshot is rich and well-formed enough, and marks its origin clearly enough, that imported history can later be shown to an agent between clear delimiters as another agent's conversation, without changing what was already shared.
- **Viewers and the instance**: "this instance" means the Vizier server the author is signed in to. People are that server's user accounts. There is no sharing between instances other than a public link or a file.
- **A conversation has one person**: a WebUI chat with an agent belongs to one person, so "the author" is unambiguous. This is why Workspaces, which have many, are out of scope.
- **Whole conversation**: a share is the whole conversation up to its last completed exchange. Choosing a range of messages is not in this version.
- **Activity on means calls and results**: with activity included, tool calls and their results are shared after the removals in FR-016, command output included. Recalled memory and reasoning stay out even then. The maintainer listed those two as part of the trail the person may exclude. This spec goes further and always excludes them.
- **Removal is best effort for unknown secrets**: values Vizier knows to be secret are removed with certainty. Other secrets are recognised by their shape, which cannot be complete. FR-020 says so to the author.
- **Offline files are out of Vizier's hands**: no revocation, no audience, no proof of authenticity. A signature that would let Vizier verify a file is left to the import spec, which is where verification would matter.
- **Open pages are not recalled**: revocation stops new visits. It does not reach into a page already open or a copy a viewer saved.
- **No notification to named people**: the author sends the link themselves. Telling named people through the inbox can be added later.
- **No view counts or analytics** in this version, and no record of who opened a share (FR-052).
- **Seeded roles**: the built-in superadmin role receives both permissions on upgrade, as it does for other new permissions. No other existing role gains them, so sharing is opt-in per role.
- **Limits are fixed numbers** in this version, not settings.
- **Design**: the Share dialog, the preview, the shared view, the list of shares and the offline file's look are new UI and are drawn before they are built, as the project requires.
- **Product principle for planning**: the maintainers want this feature to keep the server's part small and passive. That shapes the plan, not this spec, which says only what must be true.

## Implementation Status: Deferred Requirements

The requirements above are unchanged. These are not yet met by the implementation on this branch; each is tracked in `tasks.md` (Phase 9) with its reason, and none is dropped. The full cross-check is in `analysis.md`.

| Requirement | State | Task |
|---|---|---|
| FR-041, removing named people from an existing share | **Deferred.** The backend has no route for it; the WebUI does not invent one. Until it exists the author revokes the share and creates a new one. Widening the audience is already impossible (no update route) | T091 |
| FR-007, the author's display name (optional, defaults per audience) | Deferred. Needs a field in the snapshot format | T084 |
| FR-023, leaving out all pictures and audio | Deferred | T087 |
| FR-044, telling the author when someone else revokes | Deferred (backend) | T093 |
| FR-046, a 20 MB limit for the offline file | Deferred; the stricter 5 MB link limit applies to both | T095 |
| FR-047, 250 MB of snapshots per person | Deferred (backend) | T096 |
| FR-052, the record of share actions | Deferred (backend) | T099 |
| FR-030, no audience in the read response | Done | T104 |

## Open Decisions for the Product Owner

Each of these was decided here with a best guess so that planning can start. None blocks planning. Each is a place where the product owner may want a different answer.

1. **Permission names.** The maintainer wrote `:share_conversations`. This spec reads that as the action part and follows the existing `<scope>_agents:<action>` form: `owned_agents:share_conversations` and `all_agents:share_conversations` (FR-001).
2. **One permission for all three audiences.** Whoever may share may share publicly. A separate permission or an instance setting for public shares is not included.
3. **The all-agents scope doubles as the administrative right** to list and revoke any share (FR-043), instead of a third permission.
4. **Administrators cannot read shares outside their audience**, only their metadata. The title and the names of the named people do not count as metadata and are withheld (FR-043, clarified 2026-11-11).
5. **Snapshot is immutable, revocation deletes it for good** (FR-005, FR-039). There is no "update this share" and no undo.
6. **Short public cache life: 5 minutes.** The issue proposed one year, immutable. That would make revocation impossible for anything already cached, so this spec differs from the issue on purpose (FR-039, FR-040). The cost is that a very popular public share asks more of the instance than the issue intended. SC-012 bounds it.
7. **Audience can only narrow** (FR-041).
8. **One indistinguishable "not available" response** for restricted, revoked and non-existent links (FR-014), at the price of a less helpful message for someone who merely needs to sign in as someone else.
9. **Activity off by default, and four things never shared** even when it is on (FR-015, FR-016), which is stricter than the maintainer's wording.
10. **Author's name hidden by default on public shares and files**, shown by default inside the instance (FR-007).
11. **Media rules**: flat pictures and audio only, 1 MB each, larger pictures reduced, everything else a placeholder, no vector images (FR-021, FR-022). The issue mentioned images under 1 MB only.
12. **Interactive results stay live** under the restrictions of FR-025 and FR-026, static where isolation is not possible.
13. **No automatic expiry** (FR-042).
14. **Deleting an account revokes its shares** (FR-045), instead of leaving them to an administrator.
15. **Scope**: WebUI and desktop chats including project chats. Not Discord, Telegram or Workspaces (FR-003). Desktop is file only (FR-037), even when its optional web access is on.
16. **The numbers**: 5 MB per link share, 20 MB per file, 10 shares per hour, 200 live shares and 250 MB per person, 50 named people, 2,000 messages, 60 failed attempts per minute (FR-046 to FR-049).
17. **Only the author downloads the offline file** of a link share. Viewers get no download button (FR-036).
18. **Whole conversation only**, no message range.
19. **Public shares need a second confirmation and stay out of search engines and link previews** (FR-053, FR-054), with no extra permission or instance setting. A stricter choice the product owner may prefer: public sharing switched off until an administrator allows it.
20. **Finding people is narrow**: 2 characters, 10 matches, display name and username only, 30 searches per minute, only for people who may share (FR-012).
21. **Viewers learn nothing about the audience or other viewers** (FR-030). Named people cannot see who else was named.
22. **Share actions are recorded, openings are not** (FR-052). The author cannot see who opened a share.
