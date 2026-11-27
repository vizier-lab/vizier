# Feature Specification: Platform Account Linking

**Feature Branch**: `069-platform-account-linking`
**Created**: 2026-11-27
**Status**: Draft
**Input**: User description: "currently user input their discord/telegram user id and username manually. instead we will provide `/connect` command, my proposal is this command will generate a link with correct base url, when clicking those link it will connect the discord/telegram it with currently login user, or if there's any login user, user just can login then it'll connect. but let me know if there's a simpler approach. then on agent side from now on, any message by unconnected user shown it as "Unregistered User", but we still included the discord/telegram id and username in additional metadata. if user is registered we could "stamp" the username with vizier username"

## Context

Today a person types their Discord and Telegram ids and usernames into their Vizier profile by hand. Nothing checks them: anyone can claim any id, so an agent cannot trust that a Discord or Telegram message comes from the Vizier person it appears to come from. That is why features that act on a person's behalf (consultations, waking sessions) are withheld on Discord and Telegram until identities are verified (vizier-lab/vizier#116).

This feature replaces the hand-typed fields with a **proven link**: the person asks the bot for a one-time connect code while signed in to the platform, and redeems it while signed in to Vizier. Holding both sides at once is the proof.

**Chosen flow (a simplification of the proposal).** The bot's `/connect` reply always carries a short one-time **code**, and also a **link** that has the code built in whenever Vizier knows its public address. The link is the convenient path (open, sign in if needed, confirm); the code is the path that always works — typed into *Settings › Profile › Connected accounts* — including in the desktop app, behind a reverse proxy whose address Vizier was never told, or on a phone where the link opens in the wrong browser. One code, two ways to redeem it, one confirmation screen.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Connect a Discord or Telegram account with `/connect` (Priority: P1)

A person who talks to an agent on Discord (or Telegram) sends `/connect` to the agent's bot. The bot answers privately with a one-time code and, when available, a link. The person opens the link (or enters the code in their profile), signs in to Vizier if they are not already, sees which platform account is about to be connected ("Discord: @rin (id 1234…)"), and confirms. From then on that platform account is connected to their Vizier account.

**Why this priority**: It is the whole point: without a proven link nothing else in this feature means anything, and it removes the hand-typed fields.

**Independent Test**: With an agent on a Discord (or Telegram) integration, send `/connect`, redeem the code as a signed-in person, and see the account listed under Connected accounts with the platform name and id the bot saw.

**Acceptance Scenarios**:

1. **Given** a person signed in to Vizier in their browser, **When** they send `/connect` to the bot and open the link it returns, **Then** they see a confirmation naming the platform, the platform username and id, and their own Vizier username, and after confirming the account appears under Connected accounts.
2. **Given** a person *not* signed in, **When** they open the link, **Then** they are asked to sign in and, after signing in, land on the same confirmation with the code still valid.
3. **Given** Vizier does not know its public address, **When** the person sends `/connect`, **Then** the reply carries only the code and says where to enter it (Settings › Profile › Connected accounts), and entering it there leads to the same confirmation.
4. **Given** `/connect` is sent in a Discord server channel, **When** the bot replies, **Then** the reply is visible only to the sender; **given** it is sent in a Telegram group, **Then** the bot does not post a code in the group and instead tells the person to send `/connect` in a direct message to the bot.
5. **Given** a code, **When** it is redeemed once, older than 10 minutes, or the person sends `/connect` again, **Then** the old code no longer works and redeeming it shows a plain "this code has expired or was already used — send /connect again".

---

### User Story 2 - Agents see who really sent a message (Priority: P1)

When a message reaches an agent from Discord or Telegram, the agent is told who sent it in terms it can trust. A message from a connected platform account names the sender by their **Vizier username**; a message from anyone else names the sender **"Unregistered User"**. In both cases the platform's own id and display name are still given to the agent as additional metadata, so it can address the person by their platform name and tell two unregistered people apart.

**Why this priority**: This is what the link buys. It lets an agent (and later, the features held back by #116) rely on the sender, and it stops a self-declared profile from making a stranger look like the owner.

**Independent Test**: Using the offline `dummyplug` provider on a Discord or Telegram integration, send a message from a connected account and from an unconnected one, ask the agent to echo its per-request context, and compare the sender and metadata.

**Illustration** (the header an agent reads above a Discord message; field names settle in the plan):

```yaml
# Connected account
sender: rin                 # Vizier username
verified: true
sent_at: 2026-11-27T14:02:11+07:00
metadata:
  platform: discord
  platform_user_id: "381926457021"
  platform_username: rin_dev
  platform_display_name: Rin
  discord_channel_id: "1100"
  is_dm: false
```

```yaml
# Not connected
sender: Unregistered User
verified: false
sent_at: 2026-11-27T14:03:40+07:00
metadata:
  platform: discord
  platform_user_id: "552013874411"
  platform_username: kai.k
  platform_display_name: Kai
  discord_channel_id: "1100"
  is_dm: false
```

Today the same message reads `sender: "@Rin (DiscordId: 381926457021)"` for everyone, with nothing saying whether that person is anyone in Vizier.

**Acceptance Scenarios**:

1. **Given** a connected Discord account belonging to Vizier user `rin`, **When** it messages the agent, **Then** the agent sees the sender as `rin`, marked as a verified Vizier user, with the Discord id and display name in the metadata.
2. **Given** a Discord account that is not connected, **When** it messages the agent, **Then** the agent sees the sender as `Unregistered User`, with the Discord id and display name in the metadata.
3. **Given** a person connects their account, **When** their next message arrives, **Then** it is stamped with their Vizier username without restarting the agent or the bot.
4. **Given** a person unlinks their account, **When** their next message arrives, **Then** it reads as `Unregistered User` again.
5. **Given** the agent's owner has a connected account, **When** the owner writes on Discord or Telegram, **Then** the agent can recognise its owner from the verified link, not from any hand-typed field.

---

### User Story 3 - See and unlink connected accounts (Priority: P2)

A person opens *Settings › Profile* and sees a **Connected accounts** section: each connected Discord and Telegram account with its platform username, id and when it was connected, an **Unlink** item in each row's menu, and a field to enter a connect code. The old free-text id and username fields are gone.

**Why this priority**: People must be able to review and undo a link; but connecting (US1) already works without this page beyond the code field.

**Independent Test**: Connect an account, open the profile, see it listed, unlink it from the row menu, and check that the agent now reads that platform account as `Unregistered User`.

**Acceptance Scenarios**:

1. **Given** a person with a connected Telegram account, **When** they open their profile, **Then** they see it listed with username, id and connected date, and no editable id or username fields.
2. **Given** they choose Unlink from the row's menu and confirm, **Then** the account is removed from the list and its next message to any agent reads as `Unregistered User`.
3. **Given** a person who has not connected a platform, **Then** that platform's row says how to connect: "Send /connect to any agent's bot on Discord".
4. **Given** a connected Discord account, **When** its owner sends `/unlink` to any agent's bot, **Then** the bot asks privately "Unlink this Discord account from Vizier user rin?" with Unlink and Cancel controls; pressing Unlink removes the link at once and the bot confirms privately.
5. **Given** an account that is not connected, **When** it sends `/unlink`, **Then** the bot answers privately "This Discord account isn't linked to a Vizier user" and changes nothing.
6. **Given** `/unlink` in a Telegram group, **Then** the bot asks the person to send it in a direct message and changes nothing there.

---

### User Story 4 - Existing hand-typed ids after upgrading (Priority: P2)

A person who had typed their Discord or Telegram id into their profile before this feature upgrades Vizier. Their old values are no longer trusted: they are shown on the profile as **"Not verified — send /connect to confirm"**, they no longer make messages from that id read as theirs, and connecting the same platform account replaces them.

**Why this priority**: Keeping the old values trusted would leave the hole open; dropping them silently would confuse people whose agents suddenly call them "Unregistered User".

**Independent Test**: Start from a workspace whose profile has a hand-typed Discord id, upgrade, send a message from that id, and see `Unregistered User` plus the "Not verified" row on the profile; connect with `/connect` and see the row become a connected account.

**Acceptance Scenarios**:

1. **Given** an upgraded workspace with a hand-typed Discord id on a profile, **When** a message arrives from that id, **Then** the agent sees `Unregistered User`.
2. **Given** the same profile, **When** the person connects that Discord account with `/connect`, **Then** the unverified entry is replaced by the connected account.
3. **Given** the person unlinks or never connects, **Then** the unverified value stays visible (and removable) on the profile until they remove it.

---

### Edge Cases

- **A platform account already connected to another Vizier user**: redeeming its code is refused with "this Discord account is already connected to another Vizier user", naming no one; the other person's link is untouched. The current owner must unlink it first.
- **A Vizier user who already has a Discord account connected redeems a code from a different Discord account**: the confirmation says the new account replaces the current one; confirming swaps them.
- **A forwarded link** (someone sends their link to another person to make their platform account read as that person): the confirmation names the platform account and warns "only confirm a code you requested yourself"; nothing is linked without the explicit confirm by the signed-in person.
- **`/unlink` from a platform account the person no longer controls** (a lost or stolen Discord account): unlinking is also always available from the Vizier profile, so the Vizier side can cut the link without the platform account.
- **Guessing codes**: codes are long enough and redemption attempts are rate-limited per person and per address so guessing a live code is impractical.
- **Two agents on the same platform**: a link is to the platform account, not the bot, so connecting once through any agent's bot stamps that account's messages to every agent on that platform.
- **A Vizier user is deleted**: their connected accounts go with them, and those platform accounts read as `Unregistered User`.
- **Bot not able to reply privately** (Telegram group, Discord DM blocked): the bot never posts a code where others can see it; it tells the person how to ask privately instead.
- **Code redeemed while signed in as the wrong Vizier user**: the confirmation shows which Vizier user the account will be connected to, so the person can sign out first.
- **Messages already in history**: earlier messages keep the sender label they were saved with; only new messages use the new labels.
- **Side chats (`/btw`), question answers, reactions, skill slash commands and Telegram/Discord interactions** that carry a sender: they use the same verified-or-unregistered label as an ordinary message.

## Requirements *(mandatory)*

### Functional Requirements

**Connecting**

- **FR-001**: Every Discord and Telegram integration MUST answer a `/connect` command (a registered slash command on Discord, a menu command on Telegram) with a one-time connect code bound to the sender's platform account (platform, platform id, platform username/display name at the time of the request).
- **FR-002**: The reply MUST include a link with the code built in whenever Vizier has a configured public address, and MUST always include the code itself and where to enter it.
- **FR-003**: The reply MUST be visible only to the sender: ephemeral on Discord, a direct message on Telegram. In a Telegram group the bot MUST NOT post a code and MUST tell the person to message the bot directly.
- **FR-004**: A code MUST expire after 10 minutes, MUST be usable once, and a new `/connect` from the same platform account MUST void its earlier unused code.
- **FR-005**: Opening a link while signed out MUST lead through sign-in and back to the confirmation with the code intact.
- **FR-006**: Redeeming a code (by link or by typing it into the profile) MUST show a confirmation naming the platform, the platform username and id, and the signed-in Vizier username, with a warning to confirm only a code the person requested; nothing is linked until the person confirms.
- **FR-007**: One platform account MUST be connected to at most one Vizier user; a Vizier user MUST have at most one connected account per platform. Redeeming a code for an account connected to someone else MUST be refused without revealing who; redeeming a code for a second account of the same platform MUST state that it replaces the current one.
- **FR-008**: Redemption attempts MUST be rate-limited per person and per address, and a wrong, expired or used code MUST give one indistinguishable "expired or already used" answer.
- **FR-009**: Connecting and unlinking MUST take effect on the next message without restarting any agent or bot.
- **FR-010**: Connecting and unlinking MUST be recorded (who, which platform account, when) for the person's own review; no code value is ever recorded or logged.

**Managing**

- **FR-011**: The profile MUST show a Connected accounts section listing each connected account (platform, username, id, connected date) with an Unlink item in each row's menu (asking for confirmation, then taking effect at once), and a field to enter a connect code.
- **FR-011a**: Every Discord and Telegram integration MUST also answer `/unlink`: from a connected account it asks privately for confirmation, naming the Vizier user, and unlinks on confirm; from an unconnected account it says so and changes nothing. Its replies follow the same privacy rule as `/connect` (FR-003), and it needs no Vizier sign-in, because holding the platform account is enough to give it up.
- **FR-012**: The hand-entered Discord/Telegram id and username fields MUST be removed from the profile form and from the profile update operation for new values.
- **FR-013**: Deleting a Vizier user MUST remove their connected accounts.
- **FR-014**: Values entered by hand before this feature MUST NOT identify anyone; they MUST be shown as "Not verified — send /connect to confirm", MUST be removable, and MUST be replaced when the same platform is connected. (Compatibility kept until removal before 1.0.0, tracked in #82.)

**What agents see**

- **FR-015**: Every request an agent receives from Discord or Telegram MUST name its sender by the Vizier username of the connected account, marked as verified, or as `Unregistered User` when the platform account is not connected.
- **FR-016**: Every such request MUST still carry the platform, platform id and platform display name/username in its metadata, whether connected or not.
- **FR-017**: The agent's owner information MUST list the owner's connected platform accounts only, never hand-typed values, so an agent recognises its owner on a platform only through a verified link.
- **FR-018**: The same labelling MUST apply to every path on which a platform person reaches an agent: messages, mentions, silent reads, slash/skill commands, side chats, question answers and any other request built from a platform user.
- **FR-019**: History saved before this feature MUST be shown as saved; the new labels apply to new requests only.

**Out of scope**

- **FR-020**: Turning on the features withheld until verified identities (consultations and waking sessions on Discord/Telegram, #116) is NOT part of this feature; this feature only provides the verified link they need. Restricting what unregistered users may do (e.g., refusing to answer them) is also out of scope: unregistered users keep talking to agents exactly as today.

### Key Entities

- **Connected account**: a proven link between one platform account (platform, platform id) and one Vizier user; keeps the platform username/display name last seen and when it was connected.
- **Connect code**: a short-lived, single-use secret bound to one platform account, issued by `/connect`, redeemed by a signed-in Vizier user; never shown to anyone but the platform account it was issued to.
- **Unverified legacy identity**: a Discord/Telegram id or username a person typed into their profile before this feature; displayed for cleanup only, trusted for nothing.
- **Sender label**: what an agent is told about who sent a platform request — a verified Vizier username or `Unregistered User` — plus the platform identity in metadata.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person can go from sending `/connect` to seeing the account under Connected accounts in under 1 minute, in at most 3 actions after the bot replies (open link, sign in if needed, confirm).
- **SC-002**: The code path works in 100% of deployments, including the desktop app and deployments without a configured public address.
- **SC-003**: 0 platform messages from an unconnected account are attributed to a Vizier user, including accounts whose id was typed into a profile before the upgrade.
- **SC-004**: 100% of platform requests reaching an agent carry the platform id and display name in metadata, connected or not.
- **SC-005**: A connect or unlink is reflected in the very next message the account sends, with no restart.
- **SC-006**: A code is never visible to anyone other than the platform account that requested it, and an expired, used or wrong code gives the same answer.

## Assumptions

- **Simplification over the proposal**: one code redeemable by link *or* by typing it in, instead of a link only, because Vizier does not always know its public address (desktop app, reverse proxies, localhost). The link is added whenever a public address is configured; a missing address is not an error.
- The public address comes from configuration the operator sets (seed config or environment); Vizier does not guess it from incoming requests, since a guessed address could send people to the wrong host.
- A link is per platform account, not per bot or agent: connecting through one agent's bot applies to every agent on that platform.
- One connected account per platform per Vizier user, matching the single Discord and single Telegram field the profile has today.
- The confirmation step is required even when the person is already signed in, because it is the only defence against a forwarded link.
- Platform username/display name is refreshed from each new message and shown as last seen; it is never used to identify anyone, only the platform id is.
- The connect code is about 8 characters from an unambiguous alphabet (no 0/O, 1/I) so it is easy to type on a phone, with rate limiting making guessing impractical within its 10-minute life.
- UI work (Connected accounts section, confirmation page, the code field) is drawn first per the "Design first" rule: in FigCraft when reachable, otherwise as an HTML or ASCII mockup, before any WebUI code.
- `connect` and `unlink` become reserved command names so neither can be shadowed by a skill of that name.
- Existing agent prompts that read the sender label (BOOT.md guidance, side-chat limits keyed by platform identity) keep working on the platform identity in metadata.
