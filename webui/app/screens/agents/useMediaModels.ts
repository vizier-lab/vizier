import { useCallback, useRef, useState } from 'react'
import { mediaModels, refreshMediaModels, type MediaCapability, type MediaModels } from '../../api/endpoints/catalogue'

export type MediaModelsState = {
    data?: MediaModels
    loading: boolean
    /** A non-blocking notice: the last list stays usable and the form still saves. */
    error?: string
    /** Fetch once, when a picker is first opened. A form render alone never calls this. */
    ensure: () => void
    /** A person-triggered refresh; keeps the current data when it fails. */
    refresh: () => Promise<void>
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e))

/**
 * One OpenRouter media capability's models (spec 028, research R11). It lives at the
 * `ToolsFields` level and is shared by its model and voice pickers, so opening both makes one
 * request. It never touches the form's value: a refresh only replaces the options.
 */
export function useMediaModels(capability: MediaCapability): MediaModelsState {
    const [data, setData] = useState<MediaModels>()
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState<string>()
    const requested = useRef(false)
    const inFlight = useRef(false)

    const ensure = useCallback(() => {
        if (requested.current || inFlight.current) return
        requested.current = true
        inFlight.current = true
        setLoading(true)
        mediaModels(capability)
            .then((next) => {
                setData(next)
                setError(undefined)
            })
            .catch((e) => setError(message(e)))
            .finally(() => {
                inFlight.current = false
                setLoading(false)
            })
    }, [capability])

    const refresh = useCallback(async () => {
        if (inFlight.current) return
        inFlight.current = true
        setLoading(true)
        try {
            const next = await refreshMediaModels(capability)
            setData(next)
            setError(undefined)
        } catch (e) {
            // The previous list stays; the notice is quiet and never blocks saving.
            setError(message(e))
        } finally {
            inFlight.current = false
            setLoading(false)
        }
    }, [capability])

    return { data, loading, error, ensure, refresh }
}
