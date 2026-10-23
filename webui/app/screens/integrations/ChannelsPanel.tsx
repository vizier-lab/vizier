import { Button, EmptyState, FilterTabs, IntegrationCard, KeyValueList, SecretField, Skeleton, TextField, Dialog } from '@vizier/design'
import { useState } from 'react'
import { ApiError } from '../../api/client'
import { createIntegration, removeIntegration, updateIntegration, type IntegrationItem } from '../../api/endpoints/integrations'
import { cardState } from '../../lib/integrations'
import { useIntegrationList } from '../../lib/useIntegrationList'
import { ActivityCard } from './ActivityCard'
import { SelfManagementCard } from './SelfManagementCard'
import { ConfirmDialog } from '../shared/ConfirmDialog'
import { Panel } from './Panel'
import { TwoColumn } from '../shared/TwoColumn'
import { toast } from '../../stores/toasts'

const PLATFORMS = [
    { value: 'discord', label: 'Discord' },
    { value: 'telegram', label: 'Telegram' },
]

const failureOf = (item: IntegrationItem) =>
    item.status.state === 'failed' || item.status.state === 'reconnecting' || item.status.state === 'unreachable'
        ? item.status.last_error
        : item.status.state === 'incomplete'
          ? 'No bot token is set.'
          : undefined

export function ChannelsPanel({ agentId }: { agentId: string }) {
    const { data, error, reload, version } = useIntegrationList(agentId, 'channel')
    const [adding, setAdding] = useState(false)
    const [replacing, setReplacing] = useState<IntegrationItem | null>(null)
    const [deleting, setDeleting] = useState<IntegrationItem | null>(null)

    const change = async (what: Promise<unknown>, failure: string) => {
        try {
            await what
            await reload()
        } catch (e) {
            toast.error(failure, e instanceof Error ? e.message : undefined)
        }
    }

    return (
        <Panel
            meta="Where this agent talks besides the WebUI · one connection per platform"
            actions={
                data?.can_manage ? (
                    <Button variant="primary" icon="plus" onClick={() => setAdding(true)}>
                        Add connection
                    </Button>
                ) : undefined
            }
        >
            <TwoColumn
                aside={
                    <>
                        <SelfManagementCard agentId={agentId} family="channel" onChanged={reload} />
                        <ActivityCard agentId={agentId} version={version} />
                    </>
                }
            >
            {error ? (
                <EmptyState kind="error" title="Couldn’t load channels" message={error.message} />
            ) : !data ? (
                <Skeleton className="h-120" />
            ) : data.items.length === 0 ? (
                <EmptyState
                    kind="empty"
                    icon="channels"
                    title="No channels yet"
                    message="Connect a Discord or Telegram bot to chat with this agent there."
                    action={data.can_manage ? <Button variant="primary" icon="plus" onClick={() => setAdding(true)}>Add connection</Button> : undefined}
                />
            ) : (
                data.items.map((item) => {
                    const failure = failureOf(item)
                    const state = cardState(item.status, item.enabled)
                    return (
                        <IntegrationCard
                            key={item.id}
                            kind={item.kind}
                            name={item.name}
                            handle={item.kind === 'discord' ? 'Discord bot' : 'Telegram bot'}
                            state={state}
                            enabled={item.enabled}
                            onEnabledChange={(enabled) => void change(updateIntegration(item.id, { enabled }), 'Couldn’t change the connection')}
                            failure={state === 'failed' && failure ? { title: 'This connection is not working', message: failure } : undefined}
                            actions={
                                item.can_edit && (
                                    <>
                                        <Button onClick={() => setReplacing(item)}>Replace token</Button>
                                        <Button variant="destructive" icon="trash" onClick={() => setDeleting(item)}>
                                            {item.legacy_linked ? 'Unlink' : 'Delete'}
                                        </Button>
                                    </>
                                )
                            }
                        >
                            <SecretField label="Token" masked={String(item.settings.token ?? '')} onReplace={() => setReplacing(item)} />
                            <KeyValueList
                                items={[
                                    { label: 'Slash commands', value: 'Kept in step with the agent’s skills' },
                                    { label: 'Added by', value: item.creator_name ?? 'unknown' },
                                    ...(item.legacy_linked ? [{ label: 'Also used by', value: item.legacy_linked.agents.map((a) => a.name).join(', ') || `${item.legacy_linked.others} other agents` }] : []),
                                ]}
                            />
                        </IntegrationCard>
                    )
                })
            )}

            </TwoColumn>

            <AddConnection
                open={adding}
                onClose={() => setAdding(false)}
                onAdd={async (kind, token) => {
                    await createIntegration(agentId, { kind, name: kind === 'discord' ? 'Discord' : 'Telegram', settings: { token } })
                    setAdding(false)
                    await reload()
                }}
            />

            <ReplaceToken
                item={replacing}
                onClose={() => setReplacing(null)}
                onSave={async (token) => {
                    if (!replacing) return
                    await updateIntegration(replacing.id, { settings: { ...replacing.settings, token } })
                    setReplacing(null)
                    await reload()
                }}
            />

            <ConfirmDialog
                open={deleting !== null}
                title={`${deleting?.legacy_linked ? 'Unlink' : 'Delete'} ${deleting?.name ?? 'connection'}?`}
                message="The bot stops answering this agent. The bot itself is not touched."
                confirmLabel={deleting?.legacy_linked ? 'Unlink' : 'Delete connection'}
                onConfirm={() => {
                    const target = deleting
                    setDeleting(null)
                    if (target) void change(removeIntegration(agentId, target.id), 'Couldn’t remove the connection')
                }}
                onCancel={() => setDeleting(null)}
            />
        </Panel>
    )
}

function AddConnection({ open, onClose, onAdd }: { open: boolean; onClose: () => void; onAdd: (kind: string, token: string) => Promise<void> }) {
    const [kind, setKind] = useState('discord')
    const [token, setToken] = useState('')
    const [busy, setBusy] = useState(false)
    const [problem, setProblem] = useState<string>()

    return (
        <Dialog open={open} title="Add connection" confirmLabel="" onConfirm={() => {}} onCancel={onClose}>
            <form
                className="flex flex-col gap-16"
                onSubmit={async (e) => {
                    e.preventDefault()
                    setBusy(true)
                    setProblem(undefined)
                    try {
                        await onAdd(kind, token.trim())
                        setToken('')
                    } catch (err) {
                        setProblem(err instanceof ApiError || err instanceof Error ? err.message : 'It could not be added.')
                    } finally {
                        setBusy(false)
                    }
                }}
            >
                <FilterTabs aria-label="Platform" options={PLATFORMS} value={kind as 'discord' | 'telegram'} onChange={setKind} />
                <TextField label="Bot token" mono type="password" autoComplete="off" value={token} invalid={!!problem} helper={problem} onChange={(e) => setToken(e.target.value)} autoFocus />
                <div className="flex justify-end gap-8">
                    <Button type="button" variant="ghost" onClick={onClose}>Cancel</Button>
                    <Button type="submit" variant="primary" loading={busy} disabled={!token.trim()}>Add connection</Button>
                </div>
            </form>
        </Dialog>
    )
}

function ReplaceToken({ item, onClose, onSave }: { item: IntegrationItem | null; onClose: () => void; onSave: (token: string) => Promise<void> }) {
    const [token, setToken] = useState('')
    const [busy, setBusy] = useState(false)
    const [problem, setProblem] = useState<string>()
    return (
        <Dialog open={item !== null} title="Replace token" confirmLabel="" onConfirm={() => {}} onCancel={onClose}>
            <form
                className="flex flex-col gap-16"
                onSubmit={async (e) => {
                    e.preventDefault()
                    setBusy(true)
                    setProblem(undefined)
                    try {
                        await onSave(token.trim())
                        setToken('')
                    } catch (err) {
                        setProblem(err instanceof Error ? err.message : 'It could not be saved.')
                    } finally {
                        setBusy(false)
                    }
                }}
            >
                <TextField label="New bot token" mono type="password" autoComplete="off" value={token} invalid={!!problem} helper={problem ?? 'The old token stops working for this agent as soon as this is saved.'} onChange={(e) => setToken(e.target.value)} autoFocus />
                <div className="flex justify-end gap-8">
                    <Button type="button" variant="ghost" onClick={onClose}>Cancel</Button>
                    <Button type="submit" variant="primary" loading={busy} disabled={!token.trim()}>Replace token</Button>
                </div>
            </form>
        </Dialog>
    )
}
