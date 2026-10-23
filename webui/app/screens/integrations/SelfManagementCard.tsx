import { Card, SwitchRow } from '@vizier/design'
import { useEffect, useState } from 'react'
import { getGrants, setGrant, type Grants, type IntegrationFamily } from '../../api/endpoints/integrations'
import { toast } from '../../stores/toasts'

const WORDS: Record<IntegrationFamily, { label: string; intro: string }> = {
    channel: { label: 'Channels', intro: 'connect, change or remove its own channel bots from chat.' },
    addon: { label: 'Addons', intro: 'install, configure or unlink its own addons from chat.' },
    mcp: { label: 'MCP servers', intro: 'add, change or remove its own MCP servers from chat.' },
}

type Props = {
    agentId: string
    /** This page's family only. */
    family: IntegrationFamily
    /** Called after a change, so the page's activity list shows it. */
    onChanged?: () => void
}

/** The grant switch for one family (017): applies from the agent's next turn, without a restart. */
export function SelfManagementCard({ agentId, family, onChanged }: Props) {
    const [grants, setGrants] = useState<Grants>()
    const [busy, setBusy] = useState(false)

    useEffect(() => {
        getGrants(agentId).then(setGrants, () => setGrants(undefined))
    }, [agentId])

    const words = WORDS[family]
    const on = grants?.grants[family].on ?? false
    const who = grants?.grants[family].granted_by
    const canManage = grants?.can_manage ?? false

    const change = async (value: boolean) => {
        setBusy(true)
        try {
            setGrants(await setGrant(agentId, family, value))
            onChanged?.()
        } catch (e) {
            toast.error('Couldn’t change this', e instanceof Error ? e.message : undefined)
        } finally {
            setBusy(false)
        }
    }

    return (
        <Card title="Self-management" description={`Let the agent ${words.intro} Changes never restart it.`}>
            <SwitchRow
                label={words.label}
                badge={on ? 'granted' : undefined}
                helper={
                    !canManage && grants
                        ? 'Only people who manage this agent’s integrations can change grants.'
                        : on && who
                          ? `Switched on by ${who}.`
                          : undefined
                }
                checked={on}
                disabled={!grants || !canManage || busy}
                onCheckedChange={(v) => void change(v)}
            />
        </Card>
    )
}
