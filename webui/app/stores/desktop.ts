import { create } from 'zustand'
import { transport, transportReady, type DesktopState, type UpdateInfo } from '../api/transport'

type DesktopStore = {
    /** Whether this window is the desktop app. Known once the transport is chosen. */
    desktop: boolean
    /** The transport has been chosen (the desktop module loads lazily). */
    ready: boolean
    state: DesktopState | null
    update: UpdateInfo | null
    /** Starts listening to the shell; idempotent. */
    init: () => void
}

let started = false

export const useDesktop = create<DesktopStore>()((set) => ({
    desktop: false,
    ready: false,
    state: null,
    update: null,
    init: () => {
        if (started || typeof window === 'undefined') return
        started = true
        // A failure to load the desktop transport or to reach the shell must show as an error:
        // a blank window gives the person nothing to report.
        const fail = (error: unknown) =>
            set({ desktop: true, ready: true, state: { state: 'failed', detail: { error: String(error) } } })
        transportReady
            .then(async () => {
                const shell = transport.desktop
                if (!shell) {
                    set({ ready: true })
                    return
                }
                set({ desktop: true, ready: true })
                await shell.onState((state) => set({ state }))
                await shell.onUpdate((update) => set({ update: update.available ? update : null }))
                set({ state: await shell.state() })
            })
            .catch(fail)
    },
}))
