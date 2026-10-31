import { AddonTile, Button, Callout, EmptyState, PanelHeader, SearchField, Skeleton, Status, Switch } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { useParams } from 'react-router'
import {
    createIntegration,
    integrationKinds,
    removeIntegration,
    updateIntegration,
    type IntegrationItem,
    type Kind,
} from '../api/endpoints/integrations'
import { PERMISSIONS, useCan } from '../lib/access'
import { cardState, stateDetail } from '../lib/integrations'
import { useIntegrationList } from '../lib/useIntegrationList'
import { ActivityCard } from '../screens/integrations/ActivityCard'
import { AddonDialog } from '../screens/integrations/AddonForm'
import { SelfManagementCard } from '../screens/integrations/SelfManagementCard'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PageFrame } from '../screens/shared/PageFrame'
import { Panes } from '../screens/shared/Panes'
import { toast } from '../stores/toasts'

/** Addons are tools, so the Shell addon keeps the permission that guarded the old switch. */
const needsPermission = (kind: string) => kind === 'shell'

export default function IntegrationsAddons() {
    const { agentId = '' } = useParams()
    const { data, error, reload, version } = useIntegrationList(agentId, 'addon')
    const canShell = useCan(PERMISSIONS.shellConfig)
    const [kinds, setKinds] = useState<Kind[]>()
    const [query, setQuery] = useState('')
    const [installing, setInstalling] = useState<Kind | null>(null)
    const [adding, setAdding] = useState<string>()
    const [managing, setManaging] = useState<IntegrationItem | null>(null)
    const [removing, setRemoving] = useState<IntegrationItem | null>(null)

    useEffect(() => {
        integrationKinds('addon').then(setKinds, () => setKinds([]))
    }, [])

    const q = query.trim().toLowerCase()
    const matches = (name: string, description: string) => !q || name.toLowerCase().includes(q) || description.toLowerCase().includes(q)
    const kindOf = (id: string) => kinds?.find((k) => k.id === id)
    const allowed = (kind: string) => !needsPermission(kind) || canShell
    const installed = (data?.items ?? []).filter((i) => matches(kindOf(i.kind)?.name ?? i.name, kindOf(i.kind)?.description ?? ''))
    const available = useMemo(
        () => (kinds ?? []).filter((k) => !data?.items.some((i) => i.kind === k.id) && matches(k.name, k.description)),
        [kinds, data, q]
    )

    const change = async (what: Promise<unknown>, failure: string) => {
        try {
            await what
            await reload()
        } catch (e) {
            toast.error(failure, e instanceof Error ? e.message : undefined)
        }
    }

    return (
        <PageFrame
            fill
            title="Addons"
            meta="Tools this agent can use, installed like apps"
            actions={<SearchField aria-label="Search addons" placeholder="Search addons" value={query} onValueChange={setQuery} className="w-full sm:w-260" />}
        >
            <Panes
                paneLabel="Activity"
                pane={
                    <div className="flex flex-col gap-24">
                        <ActivityCard agentId={agentId} version={version} />
                        <SelfManagementCard agentId={agentId} />
                    </div>
                }
            >
                {error ? (
                    <EmptyState kind="error" title="Couldn’t load addons" message={error.message} />
                ) : !data || !kinds ? (
                    <div className="grid grid-cols-1 gap-12 md:grid-cols-2 2xl:grid-cols-3">
                        <Skeleton className="h-156" />
                        <Skeleton className="h-156" />
                        <Skeleton className="h-156" />
                    </div>
                ) : (
                    <div className="flex flex-col gap-32">
                        {installed.length > 0 && (
                            <section aria-label="Installed" className="flex flex-col gap-12">
                                <PanelHeader title="Installed" meta={`${installed.length} on this agent`} level={2} />
                                <div className="grid grid-cols-1 gap-12 md:grid-cols-2 2xl:grid-cols-3">
                                    {installed.map((item) => {
                                        const kind = kindOf(item.kind)
                                        const state = cardState(item.status, item.enabled)
                                        const detail = stateDetail(item.status)
                                        const blocked = !allowed(item.kind)
                                        return (
                                            <AddonTile
                                                key={item.id}
                                                kind={item.kind}
                                                name={kind?.name ?? item.name}
                                                description={kind?.description ?? 'An addon on this agent.'}
                                                badge={
                                                    <span className="ml-auto flex shrink-0 items-center gap-6">
                                                        <Status state={state} label={state === 'reading' ? 'Active' : undefined} font="sans" />
                                                    </span>
                                                }
                                            >
                                                {state === 'failed' && detail && <p className="mr-auto type-small text-failed-text">{detail}</p>}
                                                <span className="flex items-center gap-8 type-ui text-muted">
                                                    <Switch
                                                        checked={item.enabled}
                                                        disabled={!item.can_edit}
                                                        aria-label={`${kind?.name ?? item.name} enabled`}
                                                        onCheckedChange={(enabled) => void change(updateIntegration(item.id, { enabled }), 'Couldn’t change the addon')}
                                                    />
                                                    {item.enabled ? 'On' : 'Off'}
                                                </span>
                                                {item.can_edit && (
                                                    <Button
                                                        size="sm"
                                                        variant="ghost"
                                                        icon="settings"
                                                        disabled={blocked}
                                                        onClick={() => setManaging(item)}
                                                    >
                                                        Manage
                                                    </Button>
                                                )}
                                            </AddonTile>
                                        )
                                    })}
                                </div>
                            </section>
                        )}

                        {available.length > 0 && (
                            <section aria-label="Available" className="flex flex-col gap-12">
                                <PanelHeader title="Available" meta={`${available.length} to add`} level={2} />
                                <div className="grid grid-cols-1 gap-12 md:grid-cols-2 2xl:grid-cols-3">
                                    {available.map((k) => {
                                        const blocked = !allowed(k.id)
                                        return (
                                            <AddonTile key={k.id} kind={k.id} name={k.name} description={k.description}>
                                                <span className="type-small text-faint">
                                                    {blocked ? 'Needs the shell permission' : k.fields.some((f) => f.required) ? 'Needs setup' : 'Ready to add'}
                                                </span>
                                                {data.can_manage && (
                                            <Button
                                                size="sm"
                                                icon="plus"
                                                disabled={blocked}
                                                loading={adding === k.id}
                                                onClick={() => {
                                                    // Nothing to ask: add it on the spot. Anything with settings opens its form.
                                                    if (k.fields.length > 0) return setInstalling(k)
                                                    setAdding(k.id)
                                                    void change(createIntegration(agentId, { kind: k.id, name: k.name, settings: {} }), 'Couldn’t add the addon').finally(() => setAdding(undefined))
                                                }}
                                            >
                                                Add
                                            </Button>
                                        )}
                                            </AddonTile>
                                        )
                                    })}
                                </div>
                            </section>
                        )}

                        {installed.length === 0 && available.length === 0 && <EmptyState kind="no-results" title="No addon matches" message="Try a different search." />}

                        {available.length === 0 && (
                            <Callout title="That’s every addon in this version.">New addons ship with Vizier releases. For anything else, connect an MCP server.</Callout>
                        )}
                    </div>
                )}
            </Panes>

            <AddonDialog
                kind={installing}
                mode="install"
                onClose={() => setInstalling(null)}
                onSave={async (kind, settings) => {
                    await createIntegration(agentId, { kind: kind.id, name: kind.name, settings })
                    setInstalling(null)
                    await reload()
                }}
            />

            <AddonDialog
                kind={managing ? kindOf(managing.kind) ?? null : null}
                mode="manage"
                item={managing ?? undefined}
                onClose={() => setManaging(null)}
                onRemove={() => {
                    const target = managing
                    setManaging(null)
                    if (target) setRemoving(target)
                }}
                onSave={async (_kind, settings) => {
                    if (!managing) return
                    await updateIntegration(managing.id, { settings })
                    setManaging(null)
                    await reload()
                }}
            />

            <ConfirmDialog
                open={removing !== null}
                title={`Remove ${kindOf(removing?.kind ?? '')?.name ?? 'addon'}?`}
                message="The agent loses its tools from the next turn. Its settings are forgotten."
                confirmLabel="Remove addon"
                onConfirm={() => {
                    const target = removing
                    setRemoving(null)
                    if (target) void change(removeIntegration(agentId, target.id), 'Couldn’t remove the addon')
                }}
                onCancel={() => setRemoving(null)}
            />
        </PageFrame>
    )
}
