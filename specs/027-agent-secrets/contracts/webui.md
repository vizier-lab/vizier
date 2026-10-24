# Contract: WebUI

**Feature**: `027-agent-secrets`

## Navigation

- Add `"secrets"` to `SidebarPage` in `design/src/components/Sidebar/Sidebar.tsx`.
- Put its item in the **Misc** group, before Config, with the existing `key` icon and label `Secrets`.
- Add an optional `hiddenPages?: SidebarPage[]` prop; filter those pages out of every group before rendering.
- Update the Sidebar story's `PAGES` list and add `secrets`.
- `webui/app/layouts/AppShell.tsx`: `PAGE_PATH.secrets = "/secrets"`. Hide the page from the sidebar unless an agent is selected and `canManageSecrets(agent)` is true.

The existing `pageOf` logic then marks `secrets` active for `/{agentId}/secrets`.

## Route and access

Add to `webui/app/routes.ts`:

```ts
route(':agentId/secrets', 'routes/secrets.tsx'),
```

`webui/app/lib/access.ts` gains:

```ts
export function canManageSecrets(agent: { owner_id?: string }): boolean {
    const { user, can } = useSession.getState()
    if (!user) return false
    return (
        can('all_agents:secret') ||
        (can('owned_agents:secret') && agent.owner_id === user.user_id)
    )
}
```

This mirrors the server rule. Hiding the nav item is UX only; the page handles a `403` from a direct URL or a revoked permission with an `EmptyState kind="error"` (or a dedicated permission notice), never a blank page.

## Page

`webui/app/routes/secrets.tsx`, using `PageFrame` and the existing design components.

**Header**: title `Secrets`, description: "Credentials this agent can use without seeing them. Values are masked after saving."

**Actions**: `Add secret` (primary, `plus` icon), disabled while the create dialog is open.

**Loading**: a skeleton or the existing `Skeleton`; **error**: `EmptyState kind="error"` with the server message; **empty**: `EmptyState` explaining that no secrets are stored, with the Add action.

**Rows** (one per secret, sorted by name; a plain bordered list is enough, no paging):

- name in `type-code`/mono;
- `SecretField` with the server's `masked` value and `onReplace`;
- last change: relative time and `updated_by_name` (e.g. "replaced by alice 2h ago");
- `Delete` action.

Clicking a row does not reveal anything. There is no value column and no reveal action.

**Recent changes** (below the list): the newest 20 events from `listSecretEvents`, each showing the action (`created`/`replaced`/`deleted`), name, actor name and time. It is read-only, has no pagination in this release, and shows no value, mask or hash. A failed load shows an inline error rather than hiding the list.

**Create dialog** (`Dialog`):

- `TextField` label `Name`, mono, helper: "1–128 characters: letters, digits, '.', '_' or '-'."
- `TextField` label `Value`, `type="password"`, autoComplete off, helper: "8–65,536 bytes. You won't be able to view it again."
- Confirm calls `createSecret`; disable while saving; on success close, refresh the list, show a toast.
- On `422`, show the field error from the server and keep the dialog open.

**Replace dialog** (`Dialog`):

- Name shown read-only/locked.
- Value field as above; confirm calls `replaceSecret` (the same `PUT`).
- The old value is never fetched or shown.

**Delete** (`ConfirmDialog`):

- Message names the secret and says the agent will get `not_found` next time.
- Confirm calls `deleteSecret`; on success remove the row and show a toast.

## API client

`webui/app/api/endpoints/secrets.ts`:

```ts
export type SecretItem = {
    name: string
    masked: string
    created_at: string
    updated_at: string
    updated_by: { user: string } | { agent: string }
    updated_by_name: string
}

export type SecretEvent = {
    id: string
    name: string
    action: 'created' | 'replaced' | 'deleted'
    actor: { user: string } | { agent: string }
    actor_name: string
    at: string
}

export const listSecrets = (agentId: string) =>
    api.get<{ items: SecretItem[] }>(`/agents/${agentId}/secrets`)

export const createSecret = (agentId: string, name: string, value: string) =>
    api.put<{ status: 'created' | 'replaced'; secret: SecretItem }>(
        `/agents/${agentId}/secrets/${encodeURIComponent(name)}`,
        { value }
    )

export const deleteSecret = (agentId: string, name: string) =>
    api.delete<{ name: string; deleted: true }>(
        `/agents/${agentId}/secrets/${encodeURIComponent(name)}`
    )

export const listSecretEvents = (agentId: string, limit = 20) =>
    api.get<{ items: SecretEvent[] }>(
        `/agents/${agentId}/secrets/events?limit=${limit}`
    )
```

The API client and `schema.d.ts` are generated from `webui/openapi.json`; the file above is hand-written and typechecked against the generated schema only where the project's patterns do so.

## Role editor

Add a **Secrets** group to `webui/app/lib/permissions.ts`, after Agents:

| Permission | Label |
|---|---|
| `all_agents:secret` | Manage every agent's secrets |
| `owned_agents:secret` | Manage secrets of agents they own |

`groupsFor` already filters by the server's available permissions and falls back to "Other", so the role editor needs no other change. Both labels must appear and be grantable/revocable.

## Submodule order

1. `design/`: Sidebar page + `hiddenPages` + story; run its checks.
2. `webui/`: route, access helper, permissions group, endpoint client, page; `npm run typecheck`; regenerate `openapi.json`/`schema.d.ts` against the running server.
3. vizier parent: bump the `design` and `webui` pins, then update docs.

Do not edit `legacy/`.
