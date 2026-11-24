import { create } from 'zustand'
import { transport, transportReady } from '../api/transport'
import { applyTo, paintOf, readPaint, resolvePaint, THEME_COLORS, type Paint, type Resolved } from '../lib/themes/apply'
import { nameProblem, serializeCustoms } from '../lib/themes/names'
import { DEFAULT_SCHEME_ID, MAX_CUSTOM_SCHEMES, type Appearance, type Mode, type Scheme } from '../lib/themes/roles'
import { toast } from './toasts'

const MODE_KEY = 'vizier.theme'
const SCHEME_KEY = 'vizier.scheme'
const PAINT_KEY = 'vizier.scheme.paint'
const CUSTOMS_KEY = 'vizier.schemes'

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
 * the page on its own: the shell is told the appearance and the frame colour on screen, so call this
 * after painting. The transport is loaded lazily on the desktop, which is why this waits for it.
 */
function paintWindow(appearance: Appearance) {
    if (transport.authMode !== 'desktop') return
    const frame = getComputedStyle(document.documentElement).getPropertyValue('--vz-shell').trim()
    void transportReady.then(() => transport.desktop?.theme(appearance, frame)).catch(() => {
        /* a window that cannot be painted keeps its system frame */
    })
}

/**
 * Paints the browser's own chrome, so the address bar and the installed app's frame follow the
 * scheme. The pre-paint script does this for the first paint; this does it for every change.
 */
function putThemeColor(appearance: Appearance) {
    const meta = document.querySelector('meta[name="theme-color"]')
    if (!meta) return
    meta.setAttribute('content', THEME_COLORS[appearance])
}

function putOnPage(resolved: Resolved, animate: boolean) {
    const paint = () => {
        applyTo(document.documentElement, resolved)
        putThemeColor(resolved.appearance)
        paintWindow(resolved.appearance)
    }
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
    customs: Scheme[]
    loaded: boolean

    init: () => () => void
    load: () => Promise<void>
    /** Falls back to the default for a selected id nothing answers to, and refreshes the cache. */
    settle: () => void
    select: (schemeId: string) => void
    setMode: (mode: Mode) => void
    /** The other of light and dark than the one showing now, whatever chose it. */
    toggleMode: () => void
    useDefault: () => void
    /** Adds or replaces a custom scheme. The reason it can't, or null. Needs `load`. */
    saveCustom: (scheme: Scheme) => string | null
    deleteCustom: (schemeId: string) => void
}

/** Whether the current scheme has both light and dark. Reads the paint, so no palette is loaded. */
export const hasBothVariants = (paint: Paint | null) => !paint || (!!paint.light && !!paint.dark)

export const useAppearance = create<AppearanceStore>()((set, get) => {
    const show = (animate: boolean) => {
        const { paint, mode, deviceIsDark } = get()
        putOnPage(resolvePaint(paint, mode, deviceIsDark), animate)
    }

    const find = (schemeId: string) => [...get().presets, ...get().customs].find((scheme) => scheme.id === schemeId)

    /** Another tab changed something: read it all again. The cache itself is never listened to. */
    const syncFromStorage = async () => {
        const schemeId = read(SCHEME_KEY) ?? DEFAULT_SCHEME_ID
        set({ mode: readMode(), schemeId })
        if (get().loaded) {
            const { parseCustoms } = await import('../lib/themes/customs')
            set({ customs: parseCustoms(read(CUSTOMS_KEY)) })
        } else if (schemeId !== DEFAULT_SCHEME_ID) await get().load()
        const scheme = find(schemeId)
        const lost = schemeId !== DEFAULT_SCHEME_ID && !scheme
        set({ paint: scheme && !lost ? paintOf(scheme) : null, ...(lost && { schemeId: DEFAULT_SCHEME_ID }) })
        show(false)
    }

    return {
        schemeId: (hasWindow && read(SCHEME_KEY)) || DEFAULT_SCHEME_ID,
        mode: hasWindow ? readMode() : 'system',
        paint: hasWindow ? readPaint(read(PAINT_KEY)) : null,
        deviceIsDark: hasWindow ? deviceQuery().matches : false,
        presets: [],
        customs: [],
        loaded: false,

        init: () => {
            const { schemeId, paint, mode, deviceIsDark } = get()
            // The pre-paint script has already coloured the page; only the window frame is left.
            paintWindow(resolvePaint(paint, mode, deviceIsDark).appearance)
            if (schemeId !== DEFAULT_SCHEME_ID) void get().load().then(get().settle)

            const onDevice = (e: MediaQueryListEvent) => {
                set({ deviceIsDark: e.matches })
                show(false)
            }
            const onStorage = (e: StorageEvent) => {
                if (e.key === null || e.key === MODE_KEY || e.key === SCHEME_KEY || e.key === CUSTOMS_KEY) void syncFromStorage()
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
            const [{ PRESETS }, { parseCustoms }] = await Promise.all([import('../lib/themes/presets'), import('../lib/themes/customs')])
            set({ presets: PRESETS, customs: parseCustoms(read(CUSTOMS_KEY)), loaded: true })
        },

        settle: () => {
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

        toggleMode: () => {
            const { paint, mode, deviceIsDark, setMode } = get()
            setMode(resolvePaint(paint, mode, deviceIsDark).appearance === 'dark' ? 'light' : 'dark')
        },

        useDefault: () => {
            const saved = [write(SCHEME_KEY, null), write(PAINT_KEY, null), write(MODE_KEY, null)]
            set({ schemeId: DEFAULT_SCHEME_ID, paint: null, mode: 'system' })
            show(true)
            if (!saved.every(Boolean)) couldNotSave()
        },

        saveCustom: (scheme) => {
            const { presets, customs, schemeId } = get()
            const editing = customs.some((s) => s.id === scheme.id)
            if (!editing && customs.length >= MAX_CUSTOM_SCHEMES) return `You've reached ${MAX_CUSTOM_SCHEMES} themes. Delete one to make another.`
            const problem = nameProblem(scheme.name, [...presets, ...customs], scheme.id)
            if (problem) return problem
            const saved = { ...scheme, name: scheme.name.trim(), kind: 'custom' as const }
            const next = editing ? customs.map((s) => (s.id === scheme.id ? saved : s)) : [...customs, saved]
            const ok = [write(CUSTOMS_KEY, serializeCustoms(next))]
            set({ customs: next })
            if (schemeId === scheme.id) {
                const paint = paintOf(saved)
                ok.push(write(PAINT_KEY, paint && JSON.stringify(paint)))
                set({ paint })
                show(false)
            }
            if (!ok.every(Boolean)) couldNotSave()
            return null
        },

        deleteCustom: (id) => {
            const { customs, schemeId } = get()
            const next = customs.filter((s) => s.id !== id)
            const ok = [write(CUSTOMS_KEY, serializeCustoms(next))]
            set({ customs: next })
            if (schemeId === id) {
                ok.push(write(SCHEME_KEY, null), write(PAINT_KEY, null))
                set({ schemeId: DEFAULT_SCHEME_ID, paint: null })
                show(true)
            }
            if (!ok.every(Boolean)) couldNotSave()
        },
    }
})
