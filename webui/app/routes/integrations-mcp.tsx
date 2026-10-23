import { Button, Card, FilterTabs, IconButton, KeyValueEditor, ServerRow, SwitchRow, TextField, Textarea, type KeyValuePair } from '@vizier/design'
import { useEffect, useState } from 'react'
import { useParams } from 'react-router'
import { ApiError } from '../api/client'
import { createIntegration, removeIntegration, updateIntegration, type IntegrationItem } from '../api/endpoints/integrations'
import { mcpCategory, serverState, stateDetail } from '../lib/integrations'
import { usePagedList } from '../lib/usePagedList'
import { ActivityCard } from '../screens/integrations/ActivityCard'
import { SelfManagementCard } from '../screens/integrations/SelfManagementCard'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PagedTable } from '../screens/shared/PagedTable'
import { PageFrame } from '../screens/shared/PageFrame'
import { TwoColumn } from '../screens/shared/TwoColumn'
import { toast } from '../stores/toasts'

const SORTS = [
    { value: 'name', label: 'Name' },
    { value: 'tools', label: 'Tools' },
]

const targetOf = (s: Record<string, unknown>) =>
    s.transport === 'http' ? String(s.uri ?? '') : [s.command, ...((s.args as string[] | undefined) ?? [])].filter(Boolean).join(' ')

export default function IntegrationsMcp() {
    const { agentId = '' } = useParams()
    const list = usePagedList<IntegrationItem>(`/agents/${agentId}/integrations?family=mcp`, { sort: 'name' })
    // The paged answer carries the permission with its items.
    const canManage = (list.data as { can_manage?: boolean } | null)?.can_manage ?? false
    const [editing, setEditing] = useState<IntegrationItem | 'new' | null>(null)
    const [deleting, setDeleting] = useState<IntegrationItem | null>(null)
    const [version, setVersion] = useState(0)

    // Connections come and go on their own: look again while the tab is visible.
    useEffect(() => {
        const id = setInterval(() => document.visibilityState === 'visible' && list.reload(), 8000)
        return () => clearInterval(id)
    }, [list.reload])
    const changed = () => {
        list.reload()
        setVersion((v) => v + 1)
    }

    const toggle = async (item: IntegrationItem, enabled: boolean) => {
        try {
            await updateIntegration(item.id, { enabled })
            changed()
        } catch (e) {
            toast.error('Couldn’t change the server', e instanceof Error ? e.message : undefined)
        }
    }

    return (
        <PageFrame
            title="MCP servers"
            meta="Each server’s tools appear to this agent as <server>_<tool> · a server that is down only drops its own tools"
            actions={
                canManage ? (
                    <Button variant="primary" icon="plus" onClick={() => setEditing('new')}>
                        Add server
                    </Button>
                ) : undefined
            }
        >
            <TwoColumn
                aside={
                    editing !== null ? (
                        <Card
                            title={editing === 'new' ? 'Add MCP server' : 'Edit MCP server'}
                            headerTrailing={<IconButton icon="close" aria-label="Close" onClick={() => setEditing(null)} />}
                        >
                            <ServerForm
                                key={editing === 'new' ? 'new' : editing.id}
                                item={editing === 'new' ? undefined : editing}
                                onCancel={() => setEditing(null)}
                                onDelete={editing !== 'new' ? () => (setDeleting(editing), setEditing(null)) : undefined}
                                onSave={async (name, settings, enabled) => {
                                    if (editing === 'new') await createIntegration(agentId, { kind: 'mcp', name, settings, enabled })
                                    else await updateIntegration(editing.id, { name, settings, enabled })
                                    setEditing(null)
                                    changed()
                                }}
                            />
                        </Card>
                    ) : (
                        <ActivityCard agentId={agentId} version={version} />
                    )
                }
            >
            <PagedTable<IntegrationItem>
                label="MCP servers"
                searchLabel="Search servers"
                list={list}
                rowKey={(i) => i.id}
                sorts={SORTS}
                filters={[
                    { value: 'all', label: 'All' },
                    { value: 'down', label: 'Down' },
                    { value: 'disabled', label: 'Disabled' },
                ]}
                columns={[{ label: 'Server' }, { label: 'Status', width: 220 }, { label: 'Category', width: 160 }, { label: 'Tools', width: 70 }, { label: 'On', width: 70, align: 'end' }]}
                endAligned={[4]}
                renderRow={(item) => (
                    <ServerRow
                        key={item.id}
                        name={item.name}
                        transport={item.settings.transport === 'http' ? 'remote' : 'local'}
                        target={targetOf(item.settings)}
                        state={serverState(item.status, item.enabled)}
                        detail={stateDetail(item.status)}
                        category={mcpCategory(item.name)}
                        tools={item.status.state === 'running' ? (item.status.tools ?? undefined) : undefined}
                        enabled={item.enabled}
                        onEnabledChange={(on) => void toggle(item, on)}
                        onSelect={item.can_edit ? () => setEditing(item) : undefined}
                    />
                )}
                empty={{
                    icon: 'mcp',
                    title: 'No MCP servers',
                    message: 'An MCP server gives the agent more tools: files, databases, other services.',
                    action: canManage ? (
                        <Button variant="primary" icon="plus" onClick={() => setEditing('new')}>
                            Add server
                        </Button>
                    ) : undefined,
                }}
            />

            <SelfManagementCard agentId={agentId} family="mcp" onChanged={changed} />
            </TwoColumn>

            <ConfirmDialog
                open={deleting !== null}
                title={`${deleting?.legacy_linked ? 'Unlink' : 'Delete'} ${deleting?.name ?? 'server'}?`}
                message="The agent loses this server’s tools from the next turn."
                confirmLabel={deleting?.legacy_linked ? 'Unlink' : 'Delete server'}
                onConfirm={async () => {
                    const target = deleting
                    setDeleting(null)
                    if (!target) return
                    try {
                        await removeIntegration(agentId, target.id)
                        changed()
                    } catch (e) {
                        toast.error('Couldn’t remove the server', e instanceof Error ? e.message : undefined)
                    }
                }}
                onCancel={() => setDeleting(null)}
            />
        </PageFrame>
    )
}

type FormProps = {
    item?: IntegrationItem
    onSave: (name: string, settings: Record<string, unknown>, enabled: boolean) => Promise<void>
    onCancel: () => void
    onDelete?: () => void
}

function ServerForm({ item, onSave, onCancel, onDelete }: FormProps) {
    const s = item?.settings ?? {}
    const [transport, setTransport] = useState<'http' | 'local'>(s.transport === 'http' ? 'http' : 'local')
    const [name, setName] = useState(item?.name ?? '')
    const [command, setCommand] = useState(String(s.command ?? ''))
    const [args, setArgs] = useState(((s.args as string[] | undefined) ?? []).join('\n'))
    const [uri, setUri] = useState(String(s.uri ?? ''))
    const [env, setEnv] = useState<KeyValuePair[]>(Object.entries((s.env as Record<string, string> | undefined) ?? {}).map(([key, value]) => ({ key, value })))
    const [enabled, setEnabled] = useState(item?.enabled ?? true)
    const [busy, setBusy] = useState(false)
    const [problem, setProblem] = useState<string>()

    const ready = name.trim() !== '' && (transport === 'http' ? uri.trim() !== '' : command.trim() !== '')

    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                setBusy(true)
                setProblem(undefined)
                const settings: Record<string, unknown> =
                    transport === 'http'
                        ? { transport, uri: uri.trim() }
                        : {
                              transport,
                              command: command.trim(),
                              args: args.split('\n').map((a) => a.trim()).filter(Boolean),
                              // Masked values the person left alone go back as they came: the server keeps the secret.
                              env: Object.fromEntries(env.filter((p) => p.key.trim()).map((p) => [p.key.trim(), p.value])),
                          }
                try {
                    await onSave(name.trim(), settings, enabled)
                } catch (err) {
                    setProblem(err instanceof ApiError || err instanceof Error ? err.message : 'It could not be saved.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            <FilterTabs
                aria-label="Server type"
                options={[
                    { value: 'http', label: 'Remote URL' },
                    { value: 'local', label: 'Local command' },
                ]}
                value={transport}
                onChange={setTransport}
            />
            <TextField label="Name" mono value={name} helper={`Its tools are listed under ${mcpCategory(name.trim() || 'name')}.`} onChange={(e) => setName(e.target.value)} autoFocus />
            {transport === 'http' ? (
                <TextField label="URL" mono value={uri} placeholder="https://example.com/mcp" onChange={(e) => setUri(e.target.value)} />
            ) : (
                <>
                    <TextField label="Command" mono value={command} placeholder="npx" onChange={(e) => setCommand(e.target.value)} />
                    <Textarea label="Arguments" mono rows={3} value={args} helper="One argument per line." onChange={(e) => setArgs(e.target.value)} />
                    <KeyValueEditor label="Environment" secret value={env} onChange={setEnv} addLabel="Add variable" />
                </>
            )}
            <SwitchRow label="Enabled" checked={enabled} onCheckedChange={setEnabled} />
            {problem && <p className="type-small text-failed-text">{problem}</p>}
            <div className="flex items-center justify-between gap-8">
                <div>
                    {onDelete && (
                        <Button type="button" variant="destructive" icon="trash" onClick={onDelete}>
                            Delete
                        </Button>
                    )}
                </div>
                <div className="flex gap-8">
                    <Button type="button" variant="ghost" onClick={onCancel}>
                        Cancel
                    </Button>
                    <Button type="submit" variant="primary" loading={busy} disabled={!ready}>
                        {item ? 'Save server' : 'Add server'}
                    </Button>
                </div>
            </div>
        </form>
    )
}
