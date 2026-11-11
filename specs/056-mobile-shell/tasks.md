---
description: "Task list for the phone shell"
---

# Tasks: The Phone Shell

**Input**: Issue #152 and `specs/056-mobile-shell/spec.md`. The design is the `Shell · mobile`
and `Components · Mobile` pages of `Vizier shell.figcraft`.

**Tests**: Included. Which Agent details rows show is a pure function with vitest tests; the
layout is checked by the Playwright mobile projects.

**Organization**: grouped by repository, because the commit order is design → webui → vizier.

## Format: `[ID] [P?] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)

## Path conventions

- Design and WebUI are submodules: `design/`, `webui/`.
- Commit order is design → webui → vizier, with `just commit` and `just push` on branch
  `056-mobile-shell` in every repository (`just checkout 056-mobile-shell`).

---

## Phase 0: Design (done)

- [X] T001 Draw the phone shell in FigCraft: `M/TopBar`, the chat, conversation, switcher,
      menu drawer, topics, Home, Agent details, Tasks and Memory frames, and the 360×800 frame.
- [X] T002 Take the maintainer's two decisions into the drawing: Tasks and Artifacts in Agent
      details, Tasks out of the drawer; Home without greeting or activity.
- [X] T003 Merge the working copy into `Vizier shell.figcraft`.

## Phase 1: `design/` (done)

- [X] T004 [P] Add the `sliders` icon to `src/icons/map.ts` (the 1.5 stroke of the set).
- [X] T005 [P] Add safe-area utilities (`pt-safe`, `pb-safe`, from `env(safe-area-inset-*)`) to
      `src/styles.css`, with a story that shows them on a framed phone.
- [X] T006 Rework `MobileTopBar` (`src/components/MobileTopBar/`): bare ghost controls with 44px
      targets and no rule under the bar; an `agent` slot (sigil, name, status, chevron) with
      `onSwitch` and `onDetails`; a `topic` row (title, chevron, meta) with `onTopic`; a `title`
      mode; a `back` mode (back, title, agent name) for agent pages. Stories: Home, chat, a page,
      360 and 402 wide, both themes.
- [X] T007 Give the bell badge a ring in the ground colour (`IconButton`'s badge), so it reads on
      a bare icon.
- [X] T008 Add `side="bottom"` to `SlideOver`: full width, 20px top corners, a grab handle, the
      bottom safe area, the same Escape / scrim / × dismissal and focus return. Story.
- [X] T009 [P] Add a details button to `AgentRow` (`onDetails`, its own label, not inside the
      row's own button), and a story of the switcher list with workspaces.
- [X] T010 [P] Add `AgentDetailsList`: grouped rows (THE AGENT, ESSENTIALS, CONNECTIONS, RUNNING
      IT) from the same page types as `AgentMenu`, taking `hiddenPages`, `disabledPages`, an
      optional count per page and `hrefFor`. Story with the permission variants.
- [X] T011 [P] Add a section heading with a trailing action (`SectionHeading`, label plus an
      optional + button) and a borderless list shape for `AgentRow`. Story of the phone Home.
- [X] T012 Commit design (`feat(design): …`), one commit per component.

## Phase 2: `webui/` (done)

- [X] T013 Write `app/lib/agentDetails.ts`: the rows Agent details shows for an agent and a
      person (the `AgentMenu` page sets, `canViewArtifacts`, `canManageSecrets`, `canEditAgent`),
      with tests.
- [X] T014 Add the route `:agentId/details` (`app/routes/agent-details.tsx`): the agent header,
      `AgentDetailsList`, counts from what the stores already hold; on desktop, redirect to
      `/:agentId/config`.
- [X] T015 Add `app/screens/shell/AgentSwitcherSheet.tsx`: agents then workspaces in a bottom
      `SlideOver`, the current one marked, each row's details button to `/{agent}/details`, no
      "New agent".
- [X] T016 Add `app/screens/shell/MobileNavDrawer.tsx`: Chat, Home, Settings, Theme (the
      appearance store's `toggleMode`, shown only when the theme has both variants), and the
      person's row with profile and sign out (Sign out hidden in desktop mode, as `ShellRail`
      does).
- [X] T017 Rewrite the phone branch of `app/layouts/AppShell.tsx`: the new drawer in place of
      `ShellRail` + `AgentSidebar`; the top bar in title mode on Home and Settings, in back mode
      on agent pages (from `pageOf`), with the agent from the route or `drawerAgent`; the
      switcher sheet mounted once.
- [X] T018 Move the topic picker into a bottom sheet for the phone
      (`app/screens/chat/ConversationHeader.tsx`): the conversations, `ProjectsSection`, search,
      paging, rename and delete, from the same hooks `AgentSidebar` uses (extract the shared
      parts rather than copy them).
- [X] T019 Pad the composer by the bottom safe area on a phone (`app/screens/chat/Composer.tsx`).
- [X] T020 Rewrite `app/screens/home/MobileHome.tsx`: a Vizier title bar, AGENTS and WORKSPACES
      lists with + gated by `all_agents:create` and `workspaces:create`, the current agent marked,
      no greeting, no activity panel, no borders; and drop the greeting call from the phone
      branch of `app/routes/home.tsx`.
- [X] T021 Rewrite `e2e/mobile.spec.ts`: the new Home, the drawer's four places, the switcher and
      its details buttons, Agent details and back, the topic sheet; add `/{agent}/details` to the
      no-sideways-scroll routes; run `a11y.spec.ts` on the new sheets.
- [X] T022 `npm run typecheck`, `npm test`, `npm run build`, `npm run e2e` (mobile projects).
- [X] T023 Commit webui (`feat(webui): …`).

## Phase 3: `vizier/` (done)

- [X] T024 Bump the `design` and `webui` pins.
- [X] T025 Update the WebUI section of `AGENTS.md`/`CLAUDE.md`: the phone shell (top bar,
      switcher, Agent details, drawer).
- [X] T026 `cargo build -p vizier-http` with the new WebUI embedded, and a look at it on a phone
      over loopback.
- [X] T027 Commit vizier (`feat(webui): …` for the pin bump, `docs(spec): …` for this folder).

## What the build changed from the plan

- **The conversation's own buttons moved to the topic row.** New and Delete sat in the bar's top
  row, which the drawing keeps to the agent, its details and the bell; `MobileTopBar`'s topic row
  gained an `actions` slot and the chat passes them there, bare.
- **The bar inside an agent names the agent, not the page.** The page keeps its own header
  (title and actions) on a phone, so the bar says whose page it is and goes back to the details.
- **The last agent is remembered per browser** (`vizier.nav.lastAgent`), so Home still marks it
  after a reload.
- **The Essentials keep the desktop's labels and order** (Artifacts, Schedule, Skills, Dreams),
  from the same list the agent column reads.
- **`e2e/mobile.spec.ts` lost its "Conversation details" check.** That panel was retired for the
  status strip (`e3ca9b1`) and the check had been failing since.

## Verification

- `design`: `tsc` clean; the new and changed stories pass in both themes; Storybook builds. 26
  story failures in Sigil, Dialog, NotificationItem, Picker and SetupItem fail the same way on the
  base commit and are not this branch's.
- `webui`: `typecheck` clean, 1,126 unit tests pass, `build` passes, lint has one error fewer than
  the base. `e2e/mobile.spec.ts` and `redirects.spec.ts`: 66 of 66 on the phone projects. The
  desktop failures in `chat`, `pages`, `agents`, `admin` and `a11y`, and the phone notifications
  width check, fail the same way on the base commit.
- `vizier`: `cargo build -p vizier` with the new WebUI embedded, run on a scratch workspace.

## Not done, and why

- **No cross-agent Tasks page.** The API is per agent; Tasks are reached through Agent details.
- **No new Memory view.** The real memory explorer stays, under the back bar.
- **No desktop change.**
