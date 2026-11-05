// lint-tokens-exempt: published palettes, written as data; they are what a scheme is made of.
//
// Every palette comes from the repositories credited in docs/src/configuration/themes.md and is written
// here as published. A published colour that misses the contrast rule is moved when the module loads,
// by `fitted`: that colour only, in lightness only, as little as possible. presets.test.ts holds the
// result to the rule. In each palette the lighter background is the inset and the darker the frame.
import { ensureContrast } from './color'
import { CONTRAST_PAIRS } from './contrast'
import { makeVariant } from './derive'
import { KEY_COLORS, KEY_ROLE, ROLES, type Appearance, type Hex, type Keys, type Role, type Scheme, type Variant } from './roles'
import { VIZIER_DARK, VIZIER_LIGHT } from './vizier'

type Code = { keyword: Hex; string: Hex; number: Hex; comment: Hex; function: Hex; punctuation: Hex }
type Palette = Keys & { code: Code; roles?: Partial<Record<Role, Hex>> }

/** Moves what misses a pair. Button text is the ground, which stays, so its fill moves instead. */
function fitted(variant: Variant): Variant {
    const roles = { ...variant.roles }
    for (const { fg, bg, min } of CONTRAST_PAIRS) {
        if (fg === 'bg/ground') roles[bg] = ensureContrast(roles[bg], roles[fg], min)
        else roles[fg] = ensureContrast(roles[fg], roles[bg], min)
    }
    const keys = Object.fromEntries(KEY_COLORS.map((key) => [key, roles[KEY_ROLE[key]]])) as Keys
    return { ...variant, keys, roles }
}

function variant({ code, roles, ...keys }: Palette): Variant {
    return fitted(
        makeVariant(keys, {
            'code/keyword': code.keyword,
            'code/string': code.string,
            'code/number': code.number,
            'code/comment': code.comment,
            'code/function': code.function,
            'code/punctuation': code.punctuation,
            ...roles,
        })
    )
}

function preset(id: string, name: string, palettes: { light?: Palette; dark?: Palette }, detail?: string): Scheme {
    return {
        id,
        name,
        kind: 'preset',
        detail,
        ...(palettes.light && { light: variant(palettes.light) }),
        ...(palettes.dark && { dark: variant(palettes.dark) }),
    }
}

const VIZIER: Scheme = {
    id: 'vizier',
    name: 'Vizier',
    kind: 'preset',
    detail: 'Default',
    light: VIZIER_LIGHT,
    dark: VIZIER_DARK,
    paints: { light: [], dark: [] },
}

/** Vizier with the frame and the inset exchanged, and whatever must move to keep content readable. */
function swapped(base: Variant, others: Partial<Record<Role, Hex>> = {}): Variant {
    return {
        keys: { ...base.keys, content: base.keys.frame, frame: base.keys.content },
        roles: { ...base.roles, 'bg/shell': base.roles['bg/panel'], 'bg/panel': base.roles['bg/shell'], ...others },
        overridden: [],
    }
}

/** Only what differs from Vizier is painted, so everything else stays on the stylesheet. */
const changedFrom = (base: Variant, next: Variant): Role[] => ROLES.filter((role) => base.roles[role] !== next.roles[role])

const SWAPPED_LIGHT = swapped(VIZIER_LIGHT, {
    // Cards on the swapped inset would be 1.03:1; on white they are one clear step off it.
    'bg/surface': '#ffffff',
    // The swapped inset is a touch darker than white, which took this just under 4.5:1.
    'text/faint': '#6e6d77',
})
const SWAPPED_DARK = swapped(VIZIER_DARK)

const VIZIER_SWAPPED: Scheme = {
    id: 'vizier-swapped',
    name: 'Vizier Swapped',
    kind: 'preset',
    detail: 'Sidebar and inset exchanged',
    light: SWAPPED_LIGHT,
    dark: SWAPPED_DARK,
    paints: { light: changedFrom(VIZIER_LIGHT, SWAPPED_LIGHT), dark: changedFrom(VIZIER_DARK, SWAPPED_DARK) },
}

export const PRESETS: Scheme[] = [
    VIZIER,
    VIZIER_SWAPPED,
    preset('oxocarbon', 'Oxocarbon', {
        light: {
            content: '#ffffff',
            frame: '#f3f3f3',
            text: '#37474f',
            accent: '#0f62fe',
            live: '#42be65',
            retrying: '#ff6f00',
            failed: '#ee5396',
            roles: { 'bg/surface': '#f3f3f3', 'bg/raised': '#d5d5d5', 'line/border': '#d5d5d5', 'text/muted': '#525252' },
            code: { keyword: '#ee5396', string: '#be95ff', number: '#ffab91', comment: '#525252', function: '#ff7eb6', punctuation: '#525252' },
        },
        dark: {
            content: '#2a2a2a',
            frame: '#161616',
            text: '#f3f3f3',
            accent: '#78a9ff',
            live: '#42be65',
            retrying: '#ffab91',
            failed: '#ee5396',
            roles: { 'line/border': '#404040', 'text/muted': '#d5d5d5' },
            code: { keyword: '#78a9ff', string: '#be95ff', number: '#82cfff', comment: '#5c5c5c', function: '#3ddbd9', punctuation: '#d5d5d5' },
        },
    }),
    preset('catppuccin', 'Catppuccin', {
        light: {
            content: '#eff1f5',
            frame: '#e6e9ef',
            text: '#4c4f69',
            accent: '#8839ef',
            live: '#40a02b',
            retrying: '#df8e1d',
            failed: '#d20f39',
            roles: { 'bg/surface': '#e6e9ef', 'bg/raised': '#ccd0da', 'line/border': '#ccd0da', 'text/muted': '#5c5f77', 'text/faint': '#6c6f85' },
            code: { keyword: '#8839ef', string: '#40a02b', number: '#fe640b', comment: '#7c7f93', function: '#1e66f5', punctuation: '#7c7f93' },
        },
        dark: {
            content: '#1e1e2e',
            frame: '#181825',
            text: '#cdd6f4',
            accent: '#cba6f7',
            live: '#a6e3a1',
            retrying: '#f9e2af',
            failed: '#f38ba8',
            roles: { 'bg/surface': '#313244', 'bg/raised': '#45475a', 'line/border': '#45475a', 'text/muted': '#bac2de', 'text/faint': '#a6adc8' },
            code: { keyword: '#cba6f7', string: '#a6e3a1', number: '#fab387', comment: '#9399b2', function: '#89b4fa', punctuation: '#9399b2' },
        },
    }),
    preset('gruvbox', 'Gruvbox', {
        light: {
            content: '#fbf1c7',
            frame: '#ebdbb2',
            text: '#3c3836',
            accent: '#af3a03',
            live: '#79740e',
            retrying: '#b57614',
            failed: '#9d0006',
            roles: { 'bg/surface': '#f2e5bc', 'bg/raised': '#d5c4a1', 'line/border': '#d5c4a1', 'text/muted': '#504945', 'text/faint': '#665c54' },
            code: { keyword: '#9d0006', string: '#79740e', number: '#8f3f71', comment: '#928374', function: '#79740e', punctuation: '#af3a03' },
        },
        dark: {
            content: '#282828',
            frame: '#1d2021',
            text: '#ebdbb2',
            accent: '#fe8019',
            live: '#b8bb26',
            retrying: '#fabd2f',
            failed: '#fb4934',
            roles: { 'bg/surface': '#32302f', 'bg/raised': '#3c3836', 'line/border': '#504945', 'text/muted': '#d5c4a1', 'text/faint': '#a89984' },
            code: { keyword: '#fb4934', string: '#b8bb26', number: '#d3869b', comment: '#928374', function: '#b8bb26', punctuation: '#fe8019' },
        },
    }),
    preset('rose-pine', 'Rosé Pine', {
        light: {
            content: '#faf4ed',
            frame: '#f2e9e1',
            text: '#464261',
            accent: '#907aa9',
            live: '#286983',
            retrying: '#ea9d34',
            failed: '#b4637a',
            roles: { 'bg/surface': '#fffaf3', 'bg/raised': '#f2e9e1', 'line/border': '#dfdad9', 'text/muted': '#797593', 'text/faint': '#9893a5' },
            code: { keyword: '#286983', string: '#ea9d34', number: '#ea9d34', comment: '#9893a5', function: '#d7827e', punctuation: '#797593' },
        },
        dark: {
            content: '#1f1d2e',
            frame: '#191724',
            text: '#e0def4',
            accent: '#c4a7e7',
            live: '#9ccfd8',
            retrying: '#f6c177',
            failed: '#eb6f92',
            roles: { 'bg/surface': '#26233a', 'bg/raised': '#403d52', 'line/border': '#403d52', 'text/muted': '#908caa', 'text/faint': '#6e6a86' },
            code: { keyword: '#31748f', string: '#f6c177', number: '#f6c177', comment: '#6e6a86', function: '#ebbcba', punctuation: '#908caa' },
        },
    }),
    preset('tokyo-night', 'Tokyo Night', {
        light: {
            content: '#e6e7ed',
            frame: '#d6d8df',
            text: '#343b59',
            accent: '#2959aa',
            live: '#385f0d',
            retrying: '#8f5e15',
            failed: '#8c4351',
            code: { keyword: '#65359d', string: '#385f0d', number: '#965027', comment: '#888b94', function: '#2959aa', punctuation: '#006c86' },
        },
        dark: {
            content: '#1a1b26',
            frame: '#16161e',
            text: '#a9b1d6',
            accent: '#7aa2f7',
            live: '#9ece6a',
            retrying: '#e0af68',
            failed: '#f7768e',
            code: { keyword: '#bb9af7', string: '#9ece6a', number: '#ff9e64', comment: '#51597d', function: '#7aa2f7', punctuation: '#89ddff' },
        },
    }),
    preset('everforest', 'Everforest', {
        light: {
            content: '#fdf6e3',
            frame: '#efebd4',
            text: '#5c6a72',
            accent: '#3a94c5',
            live: '#8da101',
            retrying: '#dfa000',
            failed: '#f85552',
            roles: { 'bg/surface': '#f4f0d9', 'bg/raised': '#efebd4', 'line/border': '#e6e2cc', 'text/faint': '#829181' },
            code: { keyword: '#f85552', string: '#8da101', number: '#df69ba', comment: '#939f91', function: '#8da101', punctuation: '#939f91' },
        },
        dark: {
            content: '#2d353b',
            frame: '#232a2e',
            text: '#d3c6aa',
            // Purple, as blue and aqua are too close to the live green for a sigil's dot to show.
            accent: '#d699b6',
            live: '#a7c080',
            retrying: '#dbbc7f',
            failed: '#e67e80',
            roles: { 'bg/surface': '#343f44', 'bg/raised': '#3d484d', 'line/border': '#475258', 'text/muted': '#9da9a0', 'text/faint': '#859289' },
            code: { keyword: '#e67e80', string: '#a7c080', number: '#d699b6', comment: '#859289', function: '#a7c080', punctuation: '#859289' },
        },
    }),
    preset('solarized', 'Solarized', {
        light: {
            content: '#fdf6e3',
            frame: '#eee8d5',
            text: '#586e75',
            accent: '#268bd2',
            live: '#859900',
            retrying: '#b58900',
            failed: '#dc322f',
            roles: { 'bg/surface': '#eee8d5', 'text/muted': '#657b83', 'text/faint': '#657b83' },
            code: { keyword: '#859900', string: '#2aa198', number: '#d33682', comment: '#93a1a1', function: '#268bd2', punctuation: '#657b83' },
        },
        dark: {
            content: '#073642',
            frame: '#002b36',
            text: '#93a1a1',
            accent: '#268bd2',
            live: '#859900',
            retrying: '#b58900',
            failed: '#dc322f',
            roles: { 'text/muted': '#839496', 'text/faint': '#839496' },
            code: { keyword: '#859900', string: '#2aa198', number: '#d33682', comment: '#586e75', function: '#268bd2', punctuation: '#839496' },
        },
    }),
    preset(
        'nord',
        'Nord',
        {
            dark: {
                content: '#3b4252',
                frame: '#2e3440',
                text: '#eceff4',
                accent: '#81a1c1',
                live: '#a3be8c',
                retrying: '#ebcb8b',
                failed: '#bf616a',
                roles: { 'bg/surface': '#434c5e', 'bg/raised': '#4c566a', 'line/border': '#4c566a', 'text/muted': '#d8dee9' },
                code: { keyword: '#81a1c1', string: '#a3be8c', number: '#b48ead', comment: '#616e88', function: '#88c0d0', punctuation: '#eceff4' },
            },
        },
        'Dark only'
    ),
    preset(
        'dracula',
        'Dracula',
        {
            dark: {
                content: '#282a36',
                frame: '#21222c',
                text: '#f8f8f2',
                accent: '#bd93f9',
                live: '#50fa7b',
                retrying: '#f1fa8c',
                failed: '#ff5555',
                roles: { 'bg/surface': '#343746', 'bg/raised': '#44475a', 'line/border': '#44475a', 'text/faint': '#6272a4' },
                code: { keyword: '#ff79c6', string: '#f1fa8c', number: '#bd93f9', comment: '#6272a4', function: '#50fa7b', punctuation: '#f8f8f2' },
            },
        },
        'Dark only'
    ),

]

export const presetById = (id: string) => PRESETS.find((scheme) => scheme.id === id)

/** Which variants a scheme has, for its tile's second line. */
export function schemeDetail(scheme: Scheme): string {
    if (scheme.detail) return scheme.detail
    if (scheme.kind === 'custom') return 'Custom'
    return scheme.light && scheme.dark ? 'Light and dark' : scheme.dark ? 'Dark only' : 'Light only'
}

/** The appearances a scheme offers. */
export const appearancesOf = (scheme: Scheme): Appearance[] => (['light', 'dark'] as const).filter((a) => scheme[a])
