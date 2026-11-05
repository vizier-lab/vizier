# Feature Specification: Realtime Notifications

**Feature Branch**: `051-realtime-notifications`

**Created**: 2026-11-06

**Status**: Draft

**Input**: User description: "currently there's two type of notification, the notification itselves, and setup status, and both of this is not realtime. can we make it realtime and unify both of this into one socket/stream/api? as for the webui, currently it is a sliding panel, can we make it instead a small popup? show me the mock up. as the notification itselves can we exclude discord/telegram and or external channels. as for the notification itselves can we show the session title and a small preview/excerpt of the content?"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See what happened the moment it happens (Priority: P1)

A person is working in the WebUI or desktop app while an agent replies in another conversation, a task finishes, or an agent's setup moves forward. The bell's count, the toast, the open inbox and the setup progress all change straight away from one live connection. Nothing waits for a periodic check, and setup progress no longer arrives separately from notifications.

**Why this priority**: This is the core complaint. Today notifications can lag by up to 15 seconds and setup status arrives through a separate periodic check, so the two can disagree about what is happening.

**Independent Test**: With the app open on an unrelated page, finish a task run and start an agent that downloads a model. Confirm the count, toast and setup progress update with no page action, and that only one live connection is carrying both.

**Acceptance Scenarios**:

1. **Given** a person with the app open and the inbox closed, **When** a notification they may see is recorded, **Then** the bell count rises and a toast appears within 2 seconds.
2. **Given** the inbox is open, **When** a notification arrives, **Then** it appears at the top of the list within 2 seconds, without a toast and without moving what the person is reading out from under them.
3. **Given** an agent is downloading a model, **When** progress changes, **Then** the setup progress on screen advances at least once per second while it changes.
4. **Given** two open tabs for the same person, **When** a notification is marked read in one, **Then** the other shows the same read state and count within 2 seconds.
5. **Given** a person is viewing the conversation a reply belongs to, **When** the reply arrives, **Then** it is marked read on arrival with no toast and no rise in the count.

---

### User Story 2 - Check notifications in a small popup (Priority: P1)

A person presses the bell and a small popup opens anchored to it, floating over the page. The page does not move. It shows live setup at the top, then the filters and the list. It closes on its close button, Escape or a press outside it.

**Why this priority**: The inbox takes a whole column today and pushes the page aside, which is too much for a quick glance. This was asked for directly and is independent of the live connection.

**Independent Test**: Press the bell on any page at desktop and phone widths. Confirm the popup opens next to the bell, the page layout does not shift, and all three ways of closing work.

**Acceptance Scenarios**:

1. **Given** any page on a desktop width, **When** the bell is pressed, **Then** a popup about 360 pixels wide opens anchored to the bell, and the page content does not move or resize.
2. **Given** the popup is open, **When** the person presses Escape, a press outside it or its close button, **Then** it closes.
3. **Given** a phone width, **When** the bell is pressed, **Then** the popup opens under the bell, within the screen margins, and never causes horizontal scrolling.
4. **Given** the list is longer than the popup, **When** the person scrolls, **Then** only the list scrolls and the filters stay in reach.
5. **Given** the popup is open, **When** a row is selected, **Then** it is marked read, the popup closes and the person lands on that row's destination.

---

### User Story 3 - Know what a notification is about without opening it (Priority: P2)

Each conversation notification leads with the conversation's title, followed by a short excerpt of what the agent said. A person can tell which one matters from the list alone.

**Why this priority**: "Replied in your conversation." says nothing about which conversation or what was said, so every row has to be opened to find out.

**Independent Test**: Have an agent reply in two titled conversations. Confirm each row shows the right title and a readable excerpt of that reply.

**Acceptance Scenarios**:

1. **Given** an agent replies in a titled conversation, **When** the notification appears, **Then** the row shows the conversation title, an excerpt of the reply of no more than two lines, and the agent's name with the time.
2. **Given** a reply with headings, links, tables or code, **When** it is excerpted, **Then** the excerpt reads as plain text with no markup characters and is cut at about 140 characters with an ellipsis.
3. **Given** a task run closes, **When** its notification appears, **Then** the row shows the task title and, if the run answered, an excerpt of its answer; a failed run shows the reason instead.
4. **Given** a background job ends, **When** its notification appears, **Then** the row shows the title of the conversation that launched it, with the job's outcome as the excerpt.
5. **Given** a notification with no conversation behind it (an integration change, a dream cycle, an agent becoming ready), **When** it appears, **Then** the row shows the existing summary text and the agent's name with the time, with no empty title or excerpt space.

---

### User Story 4 - Keep external channel activity out of the inbox (Priority: P2)

The inbox covers what happens in Vizier itself. Activity in Discord, Telegram or any other external channel does not appear, since people already see it where it happened.

**Why this priority**: Busy channels fill the inbox with hourly "Replied N times" rows that bury the notifications that matter.

**Independent Test**: Have an agent reply in a Discord or Telegram conversation and launch a background job from one. Let an hour close. Confirm no notification, count change or toast results, while a WebUI reply still notifies.

**Acceptance Scenarios**:

1. **Given** an agent replies in a Discord, Telegram or other external conversation, **When** the hour closes, **Then** no notification is recorded.
2. **Given** a background job launched from an external conversation, **When** it ends, **Then** no notification is recorded; its report still goes to that conversation.
3. **Given** channel-reply notifications saved before this feature, **When** a person opens the inbox, **Then** those rows are not listed and are not counted.
4. **Given** someone changes a Discord or Telegram integration in Vizier, **When** the change is saved, **Then** it still notifies, because it is an action in Vizier and not conversation activity.

---

### User Story 5 - Stay correct through disconnects (Priority: P3)

A laptop sleeps, Wi-Fi drops or the server restarts. When the connection returns, the inbox catches up by itself with nothing missing or doubled, and setup progress shows the true current state.

**Why this priority**: Going realtime removes the periodic check that used to correct any drift, so the connection has to recover on its own to be trustworthy.

**Independent Test**: Disconnect the client, record notifications and finish a setup, then reconnect. Confirm every missed row appears once, the count is right, and the finished setup is no longer shown as running.

**Acceptance Scenarios**:

1. **Given** the connection dropped, **When** it returns, **Then** missed notifications appear exactly once and the count matches the server within 5 seconds.
2. **Given** several notifications were missed, **When** the client catches up, **Then** one toast summarises the burst rather than one toast per row.
3. **Given** a setup finished while disconnected, **When** the connection returns, **Then** it no longer shows as running, and its outcome appears as a notification if it warrants one.
4. **Given** the connection cannot be restored, **When** the person keeps using the app, **Then** the last known list stays on screen and retries continue without blocking the page.

---

### Edge Cases

- **Access revoked while connected**: nothing further about that agent is delivered from that moment, and its rows leave the list on the next refresh.
- **Signing out or a token expiring**: the live connection closes; the next person never sees the previous person's rows or counts.
- **Setup of an agent still being created**: visible only to the person allowed to see it, exactly as today, even though the agent has no saved record yet.
- **Reply arrives before its conversation has a title**: the row shows a neutral label such as "New conversation" and picks up the title once it exists.
- **Reply with no text** (a generated page, an attachment only): the excerpt describes it, such as "Sent a page" or "Sent 2 files", rather than appearing blank.
- **A secret in a reply**: an excerpt never shows a value the conversation has scrubbed, because it comes only from the scrubbed saved message.
- **Conversation deleted**: its notifications stop showing its title and excerpt straight away. The row either falls back to generic text or disappears, and it never links to a dead page without saying so.
- **Very long title or an all-code reply**: the title truncates to one line; the excerpt still reads as plain text within two lines.
- **Burst of setup progress**: many progress changes per second are merged so the interface stays smooth and other notifications are not delayed.
- **Hidden tab or minimised desktop app**: the browser may defer updates until it is visible again, then catches up. The desktop app still raises its system notifications while hidden, as it does today.
- **Agent hosted on another node**: live events reach a person's connection whichever server process they are connected to.
- **Older client against a newer server**: the existing way of reading notifications and setup state keeps working for at least the deprecation period.

## Requirements *(mandatory)*

### Functional Requirements

**One live connection**

- **FR-001**: System MUST provide one live, authenticated connection per client session that delivers both new notifications and setup status changes.
- **FR-002**: The WebUI and desktop app MUST stop checking periodically for notifications and setup state while the live connection is healthy.
- **FR-003**: The connection MUST deliver changes to notification count and read state, so that all of a person's open clients agree.
- **FR-004**: On connecting or reconnecting, the system MUST give the client the current setup state and enough information to recover missed notifications, without loss or duplicates.
- **FR-005**: Clients MUST reconnect automatically with increasing delays, and MUST NOT show a failed connection as an empty inbox.
- **FR-006**: The connection MUST work through the same interface in a browser and in the desktop app, including the desktop app with no open network port.
- **FR-007**: Every delivered event MUST pass the same visibility rules as the notification list and setup state, evaluated so that revoked access takes effect immediately.
- **FR-008**: Live delivery MUST be best-effort relative to the event that caused it: a failed or slow delivery MUST NOT fail or delay the task, turn, build or job behind it, and a slow client MUST NOT delay others.
- **FR-009**: Live events MUST reach a person regardless of which server process holds their connection, in single-node and multi-node deployments.
- **FR-010**: History MUST remain available on request: paging, filtering and marking read continue to work alongside the live connection.
- **FR-011**: The existing notification and setup read endpoints MUST keep working for older clients until retired through the project's deprecation tracking.

**Setup status**

- **FR-012**: Live setup status MUST carry what is shown today: agent, phase, message, each download's progress, and errors.
- **FR-013**: Setup status MUST remain live-only: it is never a read or unread row and never counts toward the badge. Only outcomes become notifications, under today's rules.
- **FR-014**: Setup progress MUST be merged to at most a few updates per second per agent.

**Popup**

- **FR-015**: On every page that has a bell, pressing it MUST open a small popup anchored to the bell, in place of both the docked desktop column and the full-screen phone panel.
- **FR-016**: The popup MUST NOT move or resize the page beneath it.
- **FR-017**: The popup MUST close on a visible close button, Escape, and a press outside it, in line with the project's dismissal convention.
- **FR-018**: The popup MUST show, in order: Unread and All filters with mark-all-read and close icons on the same line, live setup activity when any is running, then the list with "Load older". It has no title bar, and the filters show no counts because the bell shows the unread count as a number. The list scrolls within the popup; the filters do not.
- **FR-018a**: Each running setup MUST use the same row format as a notification: the agent's sigil, the current step as the title, the size done and percentage as the detail line, a thin progress bar, and a quiet line with the agent's name and time remaining. Where the size or time is unknown, that text is left out and the bar shows activity without a percentage. A failed download shows its error on the detail line. Rows for individual downloads are not shown, and a setup row has no read toggle.
- **FR-018b**: Rows MUST be borderless: a small agent sigil, then a title, the excerpt and a quiet line with the agent's name and time. There is no unread dot: an unread row uses darker, heavier text than a read row, and says "unread" to screen readers. The read toggle MUST appear on hover or keyboard focus, and MUST remain reachable on touch screens.
- **FR-019**: The popup MUST be usable at phone widths without horizontal scrolling and MUST respect the user's selected color theme and both light and dark modes.

**Content of a notification**

- **FR-020**: A notification tied to a conversation, task or job MUST expose a source title and, where one exists, a short plain-text excerpt.
- **FR-021**: Excerpts MUST have markup removed, be limited to about 140 characters and display in no more than two lines.
- **FR-022**: Excerpts MUST come only from content that has already been through secret scrubbing, and MUST NOT include tool arguments, tool results or the agent's reasoning.
- **FR-023**: A person MUST only see a title or excerpt from content they are permitted to open. Where they may see the notification but not its source, the row MUST show the summary alone.
- **FR-024**: Deleting a conversation or an agent MUST remove its excerpts and titles from the inbox.
- **FR-025**: Notifications with no source content MUST render cleanly with the summary, the agent's name and the time only.

**External channels**

- **FR-026**: System MUST NOT record notifications for replies in Discord, Telegram or other external conversations, nor for background jobs launched from those conversations.
- **FR-027**: Previously saved channel-reply notifications MUST be excluded from lists and counts, and MUST lapse through normal retention or removal.
- **FR-028**: Integration changes, including to Discord and Telegram connections, and task, dream and agent setup outcomes MUST continue to notify. A future external channel MUST be excluded by the same rule with no change to the inbox.

### Key Entities *(include if feature involves data)*

- **Notification**: a persisted record of a notable event, with audience, category, tone, summary, destination and read state per person. Gains an optional source title and excerpt.
- **Setup status**: the ephemeral, per-agent state of a build in progress. Never persisted, and replaced as it changes.
- **Inbox event**: one item on the live connection: a new notification, a change of counts or read state, a change of setup status, or a resynchronise instruction. Scoped to what its recipient may see.
- **Live connection**: one authenticated client's subscription, tied to a person's identity for its lifetime.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 95% of notifications appear on an open client within 2 seconds of the event, compared with up to 15 seconds today.
- **SC-002**: Setup progress on screen is never more than 2 seconds behind the build while it is running.
- **SC-003**: A client with an idle inbox makes no periodic notification or setup requests while connected.
- **SC-004**: After a reconnection, no notification is missing or duplicated in 100% of tested disconnect scenarios, and the count is correct within 5 seconds.
- **SC-005**: Opening or closing the popup moves page content by 0 pixels on every page with a bell.
- **SC-006**: A person can identify which conversation a notification is about from the row alone, with no need to open it, for 100% of conversation notifications with a title.
- **SC-007**: With an active Discord or Telegram agent, 0 inbox rows come from external conversation activity over a 24-hour period.
- **SC-008**: In an automated check, 0 excerpts contain a value the conversation scrubbed as a secret.
- **SC-009**: A person who loses access to an agent receives no further events about it, verified across all event types.

## Assumptions

- "External channels" means conversation activity on Discord, Telegram and any future platform channel. Administrative actions about those connections made inside Vizier are not external activity and keep notifying.
- A dream cycle, an agent coming up and an integration change have no conversation behind them, so they show a summary only. A task run's answer is treated like a reply for excerpt purposes.
- Excerpts are visible to whoever can open the source. A conversation reply's notification already goes only to the person whose conversation it is. A task run's answer is visible to anyone allowed to see that agent's task runs, so showing it in the inbox gives no new access. This changes spec 029's decision that notifications carry no content, deliberately and at the user's request.
- Existing rows are not backfilled with titles or excerpts; they show their stored summary.
- The 30-day retention, per-person read state, toast behaviour and auto-read of the conversation on screen all stay as they are.
- Reading older notifications and marking them read continue to be ordinary requests; only live changes use the new connection.
- A UI change is drawn in FigCraft before code. The mockup shown with this spec was rendered outside FigCraft because the FigCraft connection was unavailable, so the drawing must be made and reviewed in `Vizier.figcraft` (the notifications surface, both themes), then built in `design/`, before any WebUI code.
- Notifications sent to the operating system by the desktop app continue to work; browser push and email are out of scope.
