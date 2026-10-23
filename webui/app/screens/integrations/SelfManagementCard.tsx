import { Card, SwitchRow } from '@vizier/design'
import { useEffect, useState } from 'react'
import { getGrants, setGrant, type Grants, type IntegrationFamily } from '../../api/endpoints/integrations'
import { toast } from '../../stores/toasts'

const FAMILIES: { key: IntegrationFamily; label: string; note?: string }[] = [
    { key: 'mcp', label: 'MCP servers', note: 'Includes local servers, which run commands on the vizier host.' },
    { key: 'addon', label: 'Addons' },
    { key: 'channel', label: 'Channels', note: 'Can add, replace tokens on or delete its Discord and Telegram bots, including the one it is talking on.' },
]

export type GrantDraft = Record<IntegrationFamily, boolean>
export const noGrants: GrantDraft = { mcp: false, addon: false, channel: false }

/** Grants are stored apart from the agent's config (017): changing one never restarts it. */
const DESCRIPTION = 'Let this agent change its own integrations from conversation. Changes take effect from its next turn and never restart it.'

/** An existing agent's grants. Each switch applies at once. */
export function SelfManagementCard({ agentId }: { agentId: string }) {
    const [grants, setGrants] = useState<Grants>()
    const [busy, setBusy] = useState(false)

    useEffect(() => {
        getGrants(agentId).then(setGrants, () => setGrants(undefined))
    }, [agentId])

    const canManage = grants?.can_manage ?? false
    const change = async (family: IntegrationFamily, value: boolean) => {
        setBusy(true)
        try {
            setGrants(await setGrant(agentId, family, value))
        } catch (e) {
            toast.error('Couldn’t change this', e instanceof Error ? e.message : undefined)
        } finally {
            setBusy(false)
        }
    }

    return (
        <Card title="Self-management" description={canManage || !grants ? DESCRIPTION : 'Only people who manage this agent’s integrations can change these.'}>
            <div className="flex flex-col gap-12">
                {FAMILIES.map(({ key, label, note }) => {
                    const g = grants?.grants[key]
                    return (
                        <SwitchRow
                            key={key}
                            label={label}
                            badge={g?.on ? 'granted' : undefined}
                            helper={g?.on && g.granted_by ? `Switched on by ${g.granted_by}. ${note ?? ''}` : note}
                            checked={g?.on ?? false}
                            disabled={!grants || !canManage || busy}
                            onCheckedChange={(v) => void change(key, v)}
                        />
                    )
                })}
            </div>
        </Card>
    )
}

/** The same switches for an agent that doesn't exist yet: held in the form, applied once it is created. */
export function SelfManagementDraft({ value, onChange }: { value: GrantDraft; onChange: (next: GrantDraft) => void }) {
    return (
        <Card title="Self-management" description={`${DESCRIPTION} Applied when the agent is created.`}>
            <div className="flex flex-col gap-12">
                {FAMILIES.map(({ key, label, note }) => (
                    <SwitchRow key={key} label={label} helper={note} checked={value[key]} onCheckedChange={(v) => onChange({ ...value, [key]: v })} />
                ))}
            </div>
        </Card>
    )
}
