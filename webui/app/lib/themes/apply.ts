// lint-tokens-exempt: the browser chrome colours, which no token can carry: a meta tag's content
// is a plain string the browser reads, not a CSS rule.
// Which colours a scheme puts on the page, and putting them there. Deliberately light: the startup path
// imports this and nothing that holds a palette, so the default scheme costs nothing to load.
import { isHex } from './color'
import { ROLE_VAR, ROLE_VARS, ROLES, type Appearance, type Mode, type Role, type Scheme } from './roles'

/**
 * The colour the browser's own chrome takes, per appearance: the ground the page is on.
 *
 * A `theme-color` meta tag takes a plain colour, not a token: the browser reads the string, so it
 * cannot follow `--vz-ground` when a scheme paints a different ground. These are the two grounds
 * the design system ships (`design/src/tokens/tokens.css`), and the manifest declares the same.
 */
export const THEME_COLORS: Record<Appearance, string> = { dark: '#0b0b0c', light: '#ffffff' }


/** A `--vz-*` name to a `#rrggbb` value. */
export type Vars = Record<string, string>

/** What a scheme paints, for each variant it has: the form cached for the pre-paint script. */
export type Paint = { id: string; light?: Vars; dark?: Vars }

export type Resolved = {
    appearance: Appearance
    /** The attribute to set; null leaves the page following the device. */
    dataTheme: Appearance | null
    vars: Vars
}

const VAR_NAME = /^--vz-[a-z0-9-]+$/

/** The variables for every role of one variant, whatever it paints: a preview draws all of them. */
export function allVars(roles: Record<Role, string>): Vars {
    return Object.fromEntries(ROLES.map((role) => [ROLE_VAR[role], roles[role]]))
}

function paintedVars(scheme: Scheme, appearance: Appearance): Vars | undefined {
    const variant = scheme[appearance]
    if (!variant) return undefined
    const only = scheme.paints?.[appearance]
    return only ? Object.fromEntries(only.map((role) => [ROLE_VAR[role], variant.roles[role]])) : allVars(variant.roles)
}

/** Null for a scheme that leaves the stylesheet alone, so it has no cache at all. */
export function paintOf(scheme: Scheme): Paint | null {
    const light = paintedVars(scheme, 'light')
    const dark = paintedVars(scheme, 'dark')
    const empty = (vars?: Vars) => !vars || Object.keys(vars).length === 0
    if (light && dark && empty(light) && empty(dark)) return null
    return { id: scheme.id, ...(light && { light }), ...(dark && { dark }) }
}

/**
 * One variant shows whatever the device says; with both, a fixed mode picks one and system follows
 * the device, leaving `data-theme` unset so the stylesheet's own media query agrees.
 */
export function resolvePaint(paint: Paint | null, mode: Mode, deviceIsDark: boolean): Resolved {
    const only = paint && (!paint.light || !paint.dark) ? (paint.light ? 'light' : 'dark') : null
    if (only) return { appearance: only, dataTheme: only, vars: paint?.[only] ?? {} }
    const appearance: Appearance = mode === 'system' ? (deviceIsDark ? 'dark' : 'light') : mode
    return { appearance, dataTheme: mode === 'system' ? null : mode, vars: paint?.[appearance] ?? {} }
}

export const resolve = (scheme: Scheme, mode: Mode, deviceIsDark: boolean): Resolved => resolvePaint(paintOf(scheme), mode, deviceIsDark)

/** Which variant of a scheme is on show for this mode and device. */
export function appearanceOf(scheme: Scheme, mode: Mode, deviceIsDark: boolean): Appearance {
    if (!scheme.light) return 'dark'
    if (!scheme.dark) return 'light'
    return mode === 'system' ? (deviceIsDark ? 'dark' : 'light') : mode
}

function cleanVars(raw: unknown): Vars | undefined {
    if (!raw || typeof raw !== 'object') return undefined
    const out: Vars = {}
    for (const [name, value] of Object.entries(raw)) {
        if (VAR_NAME.test(name) && isHex(value)) out[name] = value
    }
    return out
}

/** The cache as stored, trusting nothing about it. Anything unreadable is no cache. */
export function readPaint(raw: string | null): Paint | null {
    if (!raw) return null
    try {
        const parsed = JSON.parse(raw) as { id?: unknown; light?: unknown; dark?: unknown } | null
        if (!parsed || typeof parsed !== 'object' || typeof parsed.id !== 'string') return null
        const light = cleanVars(parsed.light)
        const dark = cleanVars(parsed.dark)
        if (!light && !dark) return null
        return { id: parsed.id, ...(light && { light }), ...(dark && { dark }) }
    } catch {
        return null
    }
}

/** Puts a resolved scheme on an element: only role variables, only values that are colours. */
export function applyTo(el: Pick<HTMLElement, 'style' | 'setAttribute' | 'removeAttribute'>, { dataTheme, vars }: Resolved): void {
    for (const name of ROLE_VARS) {
        if (!(name in vars)) el.style.removeProperty(name)
    }
    for (const [name, value] of Object.entries(vars)) {
        if (VAR_NAME.test(name) && isHex(value)) el.style.setProperty(name, value)
    }
    if (dataTheme) el.setAttribute('data-theme', dataTheme)
    else el.removeAttribute('data-theme')
}

/**
 * Runs before the stylesheet, so the first paint is already the chosen scheme. It can't import
 * anything, so it checks each name and value itself, and it can never stop the page from loading.
 *
 * It also paints the browser's own chrome, so the address bar follows the scheme before the first
 * paint: the appearance it resolves is the same one `resolvePaint` resolves.
 */
export const PRE_PAINT_SCRIPT = `try{var d=document.documentElement,t=localStorage.getItem('vizier.theme'),f=t==='dark'||t==='light',a=f?t:null;if(f)d.setAttribute('data-theme',t);try{var p=JSON.parse(localStorage.getItem('vizier.scheme.paint'));if(p){var v;if(p.light&&p.dark)v=(f?t==='dark':matchMedia('(prefers-color-scheme: dark)').matches)?p.dark:p.light;else if(p.light||p.dark){v=p.light||p.dark;a=p.light?'light':'dark';d.setAttribute('data-theme',a)}if(v)for(var n in v)if(/^--vz-[a-z0-9-]+$/.test(n)&&/^#[0-9a-fA-F]{6}$/.test(v[n]))d.style.setProperty(n,v[n])}}catch(e){}if(!a)a=matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';var m=document.querySelector('meta[name="theme-color"]');if(m)m.setAttribute('content',a==='dark'?'${THEME_COLORS.dark}':'${THEME_COLORS.light}')}catch(e){}`
