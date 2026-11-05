import { create } from 'zustand'
import { transport, transportReady } from '../api/transport'
import { applyTo, paintOf, readPaint, resolvePaint, type Paint, type Resolved } from '../lib/themes/apply'
import { DEFAULT_SCHEME_ID, type Appearance, type Mode, type Scheme } from '../lib/themes/roles'
import { toast } from './toasts'

const MODE_KEY = 'vizier.theme'
const SCHEME_KEY = 'vizier.scheme'
const PAINT_KEY = 'vizier.scheme.paint'

const hasWindow = typeof window !== 'undefined'
const deviceQuery = () => window.matchMedia('(prefers-color-scheme: dark)')

function read(key: string): string | null {
    try {
        return localStorage.getItem(key)
    } catch {
        return null
    }
}

/** True if it stuck. A failed write keeps the change for this session. */
function write(key: string, value: string | null): boolean {
    try {
        if (value === null) localStorage.removeItem(key)
        else localStorage.setItem(key, value)
        return true
    } catch {
        return false
    }
}

const couldNotSave = () => toast.error("Your theme couldn't be saved in this browser.")

function readMode(): Mode {
    const stored = read(MODE_KEY)
    return stored === 'dark' || stored === 'light' ? stored : 'system'
}

/**
 * The desktop window's frame (its title bar) is native chrome drawn by the OS, so it does not follow
 * `data-theme` on its own: the shell is told which appearance to paint it in. The transport is loaded
 * lazily on the desktop, which is why this waits for it.
 */
function paintWindow(appearance: Appearance) {
    if (transport.authMode !== 'desktop') return
    void transportReady.then(() => transport.desktop?.theme(appearance)).catch(() => {
        /* a window that cannot be painted keeps its system frame */
    })
}

function putOnPage(resolved: Resolved, animate: boolean) {
    const paint = () => applyTo(document.documentElement, resolved)
    paintWindow(resolved.appearance)
    // The court changes light in one breath: the old and new grounds cross-fade instead of snapping.
    // Browsers without the API, and people who asked for less movement, get the change straight away.
    // So does the desktop window: a view transition snapshots the page through the compositor, which
    // WebKitGTK may not have (WSL, no GPU), and the window crashes on the switch.
    const start = (document as Document & { startViewTransition?: (cb: () => void) => unknown }).startViewTransition
    if (!animate || !start || transport.authMode === 'desktop' || window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
        paint()
        return
    }
    start.call(document, paint)
}

type AppearanceStore = {
    schemeId: string
    mode: Mode
    /** What is on the page now. Held in memory so a device switch needs no palette loaded. */
    paint: Paint | null
    deviceIsDark: boolean
    /** Empty until `load`: nobody on the default scheme pays for the palettes. */
    presets: Scheme[]
    loaded: boolean

    init: () => () => void
    load: () => Promise<void>
    select: (schemeId: string) => void
    setMode: (mode: Mode) => void
    useDefault: () => void
}

export const useAppearance = create<AppearanceStore>()((set, get) => {
    const show = (animate: boolean) => {
        const { paint, mode, deviceIsDark } = get()
        putOnPage(resolvePaint(paint, mode, deviceIsDark), animate)
    }

    const find = (schemeId: string) => get().presets.find((scheme) => scheme.id === schemeId)

    /** Falls back to the default for an id nothing answers to, and refreshes the cache. */
    const settle = () => {
        const { schemeId, paint } = get()
        const scheme = find(schemeId)
        if (!scheme) {
            write(SCHEME_KEY, null)
            write(PAINT_KEY, null)
            set({ schemeId: DEFAULT_SCHEME_ID, paint: null })
            show(false)
            return
        }
        const next = paintOf(scheme)
        if (JSON.stringify(next) === JSON.stringify(paint)) return
        write(PAINT_KEY, next && JSON.stringify(next))
        set({ paint: next })
        show(false)
    }

    /** Another tab changed something: read it all again. The cache itself is never listened to. */
    const syncFromStorage = async () => {
        const schemeId = read(SCHEME_KEY) ?? DEFAULT_SCHEME_ID
        set({ mode: readMode(), schemeId })
        if (schemeId === DEFAULT_SCHEME_ID) set({ paint: null })
        else {
            await get().load()
            const scheme = find(schemeId)
            set({ paint: scheme ? paintOf(scheme) : null })
            if (!scheme) set({ schemeId: DEFAULT_SCHEME_ID })
        }
        show(false)
    }

    return {
        schemeId: (hasWindow && read(SCHEME_KEY)) || DEFAULT_SCHEME_ID,
        mode: hasWindow ? readMode() : 'system',
        paint: hasWindow ? readPaint(read(PAINT_KEY)) : null,
        deviceIsDark: hasWindow ? deviceQuery().matches : false,
        presets: [],
        loaded: false,

        init: () => {
            const { schemeId, paint, mode, deviceIsDark } = get()
            // The pre-paint script has already coloured the page; only the window frame is left.
            paintWindow(resolvePaint(paint, mode, deviceIsDark).appearance)
            if (schemeId !== DEFAULT_SCHEME_ID) void get().load().then(settle)

            const onDevice = (e: MediaQueryListEvent) => {
                set({ deviceIsDark: e.matches })
                show(false)
            }
            const onStorage = (e: StorageEvent) => {
                if (e.key === null || e.key === MODE_KEY || e.key === SCHEME_KEY) void syncFromStorage()
            }
            const query = deviceQuery()
            query.addEventListener('change', onDevice)
            window.addEventListener('storage', onStorage)
            return () => {
                query.removeEventListener('change', onDevice)
                window.removeEventListener('storage', onStorage)
            }
        },

        load: async () => {
            if (get().loaded) return
            const { PRESETS } = await import('../lib/themes/presets')
            set({ presets: PRESETS, loaded: true })
        },

        select: (schemeId) => {
            const scheme = find(schemeId)
            if (!scheme) return
            const paint = paintOf(scheme)
            const saved = [write(SCHEME_KEY, schemeId === DEFAULT_SCHEME_ID ? null : schemeId), write(PAINT_KEY, paint && JSON.stringify(paint))]
            set({ schemeId, paint })
            show(true)
            if (!saved.every(Boolean)) couldNotSave()
        },

        setMode: (mode) => {
            const saved = write(MODE_KEY, mode === 'system' ? null : mode)
            set({ mode })
            show(true)
            if (!saved) couldNotSave()
        },

        useDefault: () => {
            const saved = [write(SCHEME_KEY, null), write(PAINT_KEY, null), write(MODE_KEY, null)]
            set({ schemeId: DEFAULT_SCHEME_ID, paint: null, mode: 'system' })
            show(true)
            if (!saved.every(Boolean)) couldNotSave()
        },
    }
})
