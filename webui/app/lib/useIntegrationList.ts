import { useCallback, useEffect, useRef, useState } from 'react'
import { listIntegrations, type IntegrationFamily, type IntegrationList } from '../api/endpoints/integrations'

const POLL_MS = 8000

/** An agent's integrations of one family, kept fresh while the tab is visible (statuses move). */
export function useIntegrationList(agentId: string, family: IntegrationFamily) {
    const [data, setData] = useState<IntegrationList>()
    const [error, setError] = useState<Error>()
    const [version, setVersion] = useState(0)
    const alive = useRef(true)

    const load = useCallback(async () => {
        try {
            const list = await listIntegrations(agentId, family)
            if (alive.current) {
                setData(list)
                setError(undefined)
            }
        } catch (e) {
            if (alive.current) setError(e instanceof Error ? e : new Error('Could not load integrations'))
        }
    }, [agentId, family])

    useEffect(() => {
        alive.current = true
        setData(undefined)
        void load()
        const id = setInterval(() => document.visibilityState === 'visible' && void load(), POLL_MS)
        return () => {
            alive.current = false
            clearInterval(id)
        }
    }, [load])

    return {
        data,
        error,
        /** Counts up whenever something was changed from this page, for the activity log. */
        version,
        reload: useCallback(async () => {
            await load()
            setVersion((v) => v + 1)
        }, [load]),
    }
}
