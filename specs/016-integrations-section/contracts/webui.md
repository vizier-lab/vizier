# Contract: WebUI Navigation and Pages

**Feature**: `016-integrations-section` · Visual reference: the mockup canvas (Integrations › MCP / Channels / Addons, Misc › Config).

## Sidebar (`webui/app/layout.tsx`)

```
[agent switcher]
Home · Chat · Core · Memory · Tasks · Skills · Dreams       ← per-agent menus (Usage and Agent Config removed)
── INTEGRATIONS ──
Channels · Addons · MCP                                     ← /:agentId/integrations/{channels|addons|mcp}
── MISC ──
Usage · Config                                              ← /:agentId/usage, /:agentId/config
[spacer]
Settings · Theme · Logout                                   ← unchanged
```

- **No agent selected:** Integrations and Misc items are disabled, the same way the per-agent items are today.
- **Read-only viewers:** a user who can view the agent but holds neither integration permission still sees the Integrations items. They lead to read-only pages showing what is attached (spec FR-005).
- **Config** keeps today's rule: disabled with a tooltip unless the user can edit the agent.
- **Old links:** `/:agentId/settings` redirects to `/:agentId/config`.
- **Collapsed sidebar:** section headings collapse to a divider, like the existing `.collapsed` rules.

## Routes (`webui/app/routes.ts`)

| Route | File | Notes |
|---|---|---|
| `:agentId/integrations/channels` | `routes/integrations-channels.tsx` | cards per connection attached to the agent, plus "Connect another bot" (new Discord, new Telegram, or take over an unassigned one) |
| `:agentId/integrations/addons` | `routes/integrations-addons.tsx` | app-store tile grid from `GET /integrations/kinds?family=addon`; Installed / Get badge per tile; details panel with Install/Uninstall for this agent, shared settings and also-used-by |
| `:agentId/integrations/mcp` | `routes/integrations-mcp.tsx` | table: server, status, also used by, On-for-agent switch; side edit panel; shared-edit warning |
| `:agentId/config` | `routes/agent-settings.tsx` (renamed export) | `AgentForm` without the moved sections; page title "Config" |
| `:agentId/settings` | redirect | → `:agentId/config` |

## Behaviour rules

- **Following the agent switcher:** each Integrations page reloads its store when `agentId` changes (spec US1 scenario 6).
- **Install from the Addons page:**
  - when the addon is complete: `PUT …/attachment {attached:true}`;
  - when it's incomplete: open the settings form inline, then `POST`/`PUT` the integration, then attach.
- **Saving a shared integration** (`also_used_by` not empty): show the warning "also used by …, applies to them from their next turn" before sending the `PUT`.
- **Toasts after a save** say the change "applies from the next turn". They never say "restarting".
- **Secret inputs** show the redacted value with a **Replace** button. They send the field only when it was replaced.
- **Typecheck:** `cd webui && npm run typecheck` must pass.

## Store

`webui/app/hooks/integrationStore.tsx`:
- **State:** `{ agentId, family, items, kinds, statuses, loading }`.
- **Actions:** `load(agentId, family)`, `attach`, `detach`, `create`, `update`, `remove`, `refreshStatus`.
- **Polling:** status is refreshed every 10 s while a page is open, and stops when it closes.
