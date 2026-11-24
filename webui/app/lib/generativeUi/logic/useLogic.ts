import { useEffect, useMemo, useRef } from 'react'
import { LogicHost, iframeTransport, type Defs } from './host'

const IDLE_RELEASE_MS = 60_000

/**
 * The function that runs an interface's `logic`, or undefined when it has none. The runner starts
 * on the first call, is released after a minute unused (the next call starts it again), and is
 * destroyed when the interface goes away.
 */
export function useLogicRunner(logic: Defs | undefined): ((fn: string, args: unknown[]) => Promise<unknown>) | undefined {
    const host = useRef<LogicHost | null>(null)
    const idle = useRef<ReturnType<typeof setTimeout> | null>(null)
    const hasLogic = !!logic && Object.keys(logic).length > 0

    useEffect(
        () => () => {
            if (idle.current) clearTimeout(idle.current)
            host.current?.dispose()
            host.current = null
        },
        []
    )

    return useMemo(() => {
        if (!hasLogic) return undefined
        return (fn: string, args: unknown[]) => {
            host.current ??= new LogicHost(logic!, iframeTransport)
            if (idle.current) clearTimeout(idle.current)
            const done = host.current.call(fn, args)
            const release = () => {
                idle.current = setTimeout(() => host.current?.release(), IDLE_RELEASE_MS)
            }
            void done.then(release, release)
            return done
        }
        // The logic of a saved call never changes.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [hasLogic])
}
