# Contract: WebUI

## Misc › Config: the "Self-management" section (`components/AgentForm.tsx`, Tools tab, after "Tool Settings")

- On load, `GET …/integrations/grants`. Three checkboxes: **MCP servers**, **Addons**, **Channels**.
- The MCP checkbox always shows the warning "Includes local servers, which run commands on the vizier host." The Channels checkbox notes that the agent can delete the bot it is talking on.
- `can_manage: false` → checkboxes disabled, with the notice "Only people who can manage this agent's integrations can change these."
- Saving the form sends `PUT /agents/{id}` as today, then `PUT …/integrations/grants` only if a checkbox changed. If the grants request fails, the form shows an error for that section; the agent save is not rolled back.
- For a new agent (Agent New), the section is hidden until the agent exists.

## Integrations › MCP / Addons / Channels

- Rows list only this agent's integrations. The MCP attach checkbox becomes an **enabled** switch (`PUT /integrations/{id} {enabled}`). The Addons configuration picker is removed: **Install** = create (or edit if one exists), **Uninstall** = `DELETE /agents/{id}/integrations/{iid}` with a confirmation.
- Each row shows a subtitle `<transport/platform> · added by <name>`, with a 🤖 marker when the creator is an agent.
- `SharedEditWarning` is replaced by `LegacyLinkedNotice`: *"Also linked to X, Y — left over from before integrations were per-agent. Deleting here only unlinks this agent."* Rows the caller can't edit are read-only.
- The edit panel gets **Settings | Activity** tabs. Activity lists `GET …/integrations/events?integration_id=` newest first, showing who (person or 🤖 agent), the action, the changed field names, the time, and for agents a link to the session.

## Types (`interfaces/types.ts`)

`IntegrationItem`:
- loses `attached`, `can_attach`, `also_used_by`, `others`;
- loses `scope` (from the view);
- gains `creator`, `creator_name`, `legacy_linked?: {agents: AgentRef[], others: number}`.

New types: `IntegrationGrants`, `IntegrationEvent`.

`npm run typecheck` must pass.
