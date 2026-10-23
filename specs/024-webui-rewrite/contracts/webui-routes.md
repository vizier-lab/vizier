# Contract: WebUI routes and repository layout

## Repository layout (vizier)

```text
vizier/
├── design/            # submodule vizier-lab/design (unchanged role: components + catalogue)
├── webui/             # submodule vizier-lab/webui (NEW: the rewritten app; build output webui/build/client/)
├── legacy/            # the previous webui/, moved unchanged; never built, served or packaged
├── build.rs           # unchanged: builds webui/ when webui/node_modules exists
└── src/channels/http/webui/mod.rs   # unchanged: embeds webui/build/client
```

`.gitmodules` gains `[submodule "webui"] path = webui, url = https://github.com/vizier-lab/webui.git`. The HTTPS URL is used so CI and anonymous clones work; pushes use the SSH remote the maintainer configures (`git@github.com:vizier-lab/webui.git`), matching how `design` is set up.

## webui repository layout

```text
webui/
├── app/
│   ├── root.tsx              # html shell, theme attribute, styles import
│   ├── routes.ts             # the table below
│   ├── api/                  # typed fetch client, schema.d.ts (generated), frames.ts, socket.ts
│   ├── stores/               # session, agents, connection, turns, drafts, toasts
│   ├── lib/                  # pure logic + tests: frames.ts, clientTools.ts, trail.ts, pageDocument.ts, paging.ts, format.ts
│   ├── layouts/              # AppShell (Sidebar / MobileTopBar), SettingsLayout, AuthLayout
│   ├── routes/               # one file per screen
│   └── screens/<screen>/     # screen-only compositions (forms, drawers, panels)
├── e2e/                      # Playwright smoke suite against a running vizier + dummyplug agent
├── openapi.json              # copy of vizier's /openapi.json the types are generated from
├── package.json              # "@vizier/design": "file:../design"
├── vite.config.ts            # resolve.dedupe react/react-dom
└── README.md                 # builds inside a vizier checkout (needs ../design)
```

## Routes

| Path | Screen (design page) | Notes |
|------|---------------------|-------|
| `/login` | Login | Public. Uses `GET /status` |
| `/onboarding` | Onboarding | Public, only while `setup_required` |
| `/` | Home (+ first-run variant) | |
| `/agents/new` | New agent | Wizard steps in `?step=` |
| `/:agentId/chat` | Chat (latest topic, or new) | Kept from legacy |
| `/:agentId/chat/:topicId` | Chat | Kept from legacy; `?c=<channel>` opens a Discord/Telegram conversation read-only |
| `/:agentId/core` | Core | `?rev=` opens a revision |
| `/:agentId/memory` | Memory, "All bundles" | `?bundle=&doc=` |
| `/:agentId/memory/:bundle` | Memory, inside a bundle | |
| `/:agentId/tasks` | Tasks | `?task=&run=` |
| `/:agentId/skills` | Skills | `?skill=` |
| `/:agentId/skills/:slug/edit` | Skill editor | |
| `/:agentId/dream` | Dreams | Kept from legacy; `?cycle=` |
| `/:agentId/usage` | Usage | |
| `/:agentId/config` | Config | `?tab=` |
| `/:agentId/integrations/channels` · `/addons` · `/mcp` | Channels · Addons · MCP | Kept from legacy |
| `/settings/profile` · `/password` · `/api-keys` · `/providers` · `/users` · `/roles` | Settings pages | Admin pages hidden without permission |

Redirects (FR-016): `/settings` → `/settings/profile`; `/:agentId/settings` → `/:agentId/config`; anything unknown → `/`.

Permission gating (FR-012): routes read the current user's permissions from the `session` store. A page the person can't use is not in the navigation. A direct visit shows the EmptyState "You don't have access to this page", never a page that then fails.
