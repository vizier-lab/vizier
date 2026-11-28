import { useEffect, useState } from 'react'
import { listConversations } from '../../api/endpoints/conversations'
import { CHANNEL } from '../../api/endpoints/chat'
import { triggerKinds, type TriggerKind } from '../../api/endpoints/triggers'
import { useSession } from '../../stores/session'

export function useTriggerKinds() {
    const [kinds, setKinds] = useState<TriggerKind[] | null>(null)
    useEffect(() => {
        triggerKinds().then(setKinds, () => setKinds([]))
    }, [])
    return kinds
}

/** The person's own WebUI conversations with the agent: where a result may also be posted. */
export function useDestinations(agentId: string) {
    const username = useSession((s) => s.user?.username)
    const [options, setOptions] = useState<{ value: string; label: string }[]>([])
    useEffect(() => {
        let live = true
        listConversations(agentId, { channel: 'webui', limit: 100 }).then(
            (page) => {
                if (!live) return
                setOptions(
                    page.items
                        .filter((c) => c.channel === `http__${username}__${CHANNEL}` && c.latest_topic)
                        .map((c) => ({ value: `webui:${c.latest_topic}`, label: c.title }))
                )
            },
            () => {}
        )
        return () => {
            live = false
        }
    }, [agentId, username])
    return options
}
