# WebUI inventory (pre-rewrite baseline)

This is a snapshot of every page and feature in the current WebUI (`webui/`), with screenshots, written before the UI/UX rewrite. Use it as the checklist of what the new UI has to cover, or deliberately drop.

- **Captured from:** `master` @ `cc06bb7` (v0.12.0-rc.3), 2026-10-09
- **Stack being replaced:** React Router v7 (SPA), React 19, Tailwind v4 mixed with inline `style={{…}}`, Zustand stores, MDXEditor, Recharts, react-markdown and highlight.js
- **Data shown:** a throwaway instance seeded with demo data. The agents run on the offline `dummyplug` provider, so chat replies are lorem-ipsum or raw tool output and the token counts read 0. Every credential in the screenshots is fake.
- **Regenerating the screenshots:** see [`capture/`](capture/README.md)

## Page index

| # | Route | Page | Doc |
|---|-------|------|-----|
| 1 | `/onboarding`, `/login` | First-run admin setup, sign in | [01-auth.md](pages/01-auth.md) |
| 2 | `/` | Home: quick chat with last agent, agent picker | [02-home.md](pages/02-home.md) |
| 3 | `/agents/new` | Create-agent wizard (Config → Tools → System Prompt → Review) | [03-create-agent.md](pages/03-create-agent.md) |
| 4 | `/:agentId/chat/:topicId?` | Chat: topics, streaming activity trail, attachments, voice, slash commands, reactions | [04-chat.md](pages/04-chat.md) |
| 5 | `/:agentId/core` | CORE.md editor and version history | [05-core.md](pages/05-core.md) |
| 6 | `/:agentId/memory` | Memory: bundle/concept graph, view/edit/create, history, import/export | [06-memory.md](pages/06-memory.md) |
| 7 | `/:agentId/tasks` | Scheduled tasks: list, create/edit, run history | [07-tasks.md](pages/07-tasks.md) |
| 8 | `/:agentId/skills` | Agent skills CRUD | [08-skills.md](pages/08-skills.md) |
| 9 | `/:agentId/dream` | Dream journal, status, manual trigger | [09-dreams.md](pages/09-dreams.md) |
| 10 | `/:agentId/usage` | Token usage analytics | [10-usage.md](pages/10-usage.md) |
| 11 | `/:agentId/settings` | Agent Config (Config / System Prompt / Tools / Sharing / Danger Zone) | [11-agent-config.md](pages/11-agent-config.md) |
| 12 | `/settings` | Global settings (Profile / Password / API Keys / Providers / Users / Roles) | [12-settings.md](pages/12-settings.md) |
| 13 | — | App shell: sidebar, agent switcher, theme, mobile drawer, toasts | [13-shell-theme-mobile.md](pages/13-shell-theme-mobile.md) |

## Information architecture today

```
Public:   /login   /onboarding
App shell (sidebar, auth guard):
  /                       Home
  /agents/new             Create agent
  /settings               Global settings  (6 sub-sections, permission-gated)
  /:agentId/
      chat[/:topicId]     Chat
      core                Core
      memory              Memory
      tasks               Tasks
      skills              Skills
      dream               Dreams
      usage               Usage
      settings            Agent Config     (owner or all_agents:edit only)
```

The **current agent** is held in the sidebar's agent card. It is persisted per user in `localStorage` as `vizier_last_agent_<user_id>`, and every per-agent nav item links to that agent. The last chat topic is remembered per agent too.

## Cross-cutting features (the rewrite has to keep or redesign these)

- **Auth:** JWT kept in `localStorage.auth_token`. A 401 from any call sends the user to `/login`. First run (`/auth/setup-status`) sends them to `/onboarding`.
- **Permissions (RBAC):** 14 permission strings, granted through roles. They gate the settings sections and Agent Config. See [12-settings.md](pages/12-settings.md#roles).
- **Agent health:** the sidebar polls each agent's `/ping` and shows a green status dot on its avatar.
- **Realtime:** one WebSocket per open chat topic. The client also polls topic `is_thinking` once a second.
- **Markdown everywhere:** MDXEditor is the input for chat, CORE, memory, tasks, skills and system prompts. Rendering uses react-markdown with GFM and highlight.js.
- **Version history:** one shared `VersionHistory` slide-over (list, view, diff, compare two, restore) serves both CORE and memory.
- **Slide-over pattern:** every view, create and edit form for memory, tasks, skills and history opens in a right-hand `SlideOver`, not on its own route, so none of them can be linked to.
- **Toasts:** success and error toasts for nearly every mutation.
- **Destructive confirmations:** these use the native `window.confirm()`. Agent delete is the exception: it asks you to type the agent ID.
- **Theme:** light, dark or system, persisted as `localStorage.theme`.
- **Responsive:** the sidebar becomes a drawer on mobile, and the section navs in Agent Config and Settings become horizontal tabs.

## Observations for the rewrite (found while capturing)

These came from reading the code and driving the UI. They are not a spec, just things worth knowing.

**Structure and debt**
- `components/AgentForm.tsx` (3.4k lines, create) and `routes/agent-settings.tsx` (3.3k lines, edit) re-implement the same agent form, so every agent field is maintained twice. `routes/chat.tsx` is 2.1k lines and `routes/settingsRoot.tsx` is 1.9k.
- Styling is mostly inline `style={{}}` objects with hard-coded hex colours (Dreams, login), with Tailwind and `app.css` classes mixed in.
- Dead components: `AgentCard.tsx`, `chat_bubble.tsx` and `dot_loader.tsx` aren't imported by any page. `ReactionBar.tsx` is only referenced by itself.
- The `agents:mcp_config` and `agents:shell_config` permissions can be granted, but nothing enforces them, in the UI or the backend.
- The Providers list is hard-coded in the UI (`ALL_VARIANTS`, 29 entries) rather than coming from the backend.

**Behaviour quirks**
- Opening **Core** shows "Unsaved changes" right away, because MDXEditor normalises the markdown on load ([screenshot](screenshots/13-core-editor.png)).
- The chat tool-label catalogue (`formatToolChoice` in `chat.tsx`) is stale. It labels `memory_read` as "Searching memory…", still has a case for the retired `memory_detail`, and has no case for `memory_search`, which falls through to the generic "🔧 Using …" label.
- On the **Usage** page, the per-channel "avg …/req" figure runs `formatDuration()` on *tokens per request*, so it shows a time unit for a token count.
- The sidebar defaults to **collapsed** (icons only) for new users.
- New memory bundles have no UI of their own. Naming one in the "Create memory" form creates it.
- The memory graph is canvas-only. Bundles and concepts are reachable only by clicking nodes or searching; there's no list or table view, and on a fresh corpus the nodes start very small.

**Backend features with no UI today**
- `AgentConfig.chunking` (`ChunkLimits`) and `auto_context` (`chat_passages`, `silent_read_passages`, `threshold`, `size_cap`, `per_document`).
- Session/topic files (`list_session_files` and the session file storage) have no browser.
- The Tasks, Skills and Users tables don't reflow on mobile; they're simply clipped ([screenshot](screenshots/39-mobile-tasks.png)).
- Agent-level sharing appears only inside Agent Config, and there's no "shared with me" view.
