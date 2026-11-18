import type { BrandName, CallGlyph, CallIconItem, IconName } from '@vizier/design'
import type { ExecutionReport } from '../api/frames'
import {
    ADDON_IDS,
    BUILTIN_TOOLS,
    PROJECT_TOOLS,
    UNCATEGORISED_TOOLS,
    type AddonId,
    type BuiltinCategory,
} from './toolCategories'

// What a script's calls were, in words and icons, worked out from their names alone: the report
// carries only names. Built-in, project and addon tools are exact; functions are exact when the
// agent's list could be read; anything else is an MCP server's tool, recognised by the agent's
// MCP server names when they could be read and guessed from the name otherwise.

/** What the trail knows about the agent beyond the built-ins. Either part may be empty. */
export type ToolContext = {
    /** Function name (as a script calls it) → its category. */
    functions: ReadonlyMap<string, string>
    /** The agent's MCP server keys, as they prefix its tools' names. */
    mcpServers: readonly string[]
}

export const EMPTY_CONTEXT: ToolContext = { functions: new Map(), mcpServers: [] }

/** Where a call belongs: its group on the trail. */
export type CallKind = {
    /** Stable group id: "memory", "addon:brave_search", "functions", "mcp:github". */
    key: string
    label: string
    glyph: CallGlyph
    /** For a function: its own name, listed under Functions. */
    detail?: string
    /** The words for a sentence about the run: "searched the web". */
    phrase: string
}

type Described = { label: string; icon: IconName; phrase: string }

const BUILTIN: Record<BuiltinCategory, Described> = {
    memory: { label: 'Memory', icon: 'memory', phrase: 'used its memory' },
    scheduling: { label: 'Schedule', icon: 'calendar', phrase: 'worked on the schedule' },
    agents: { label: 'Background work', icon: 'layers', phrase: 'checked background work' },
    skills: { label: 'Skills', icon: 'skills', phrase: 'used its skills' },
    files: { label: 'Files', icon: 'file', phrase: 'worked with files' },
    scratchpad: { label: 'Scratchpad', icon: 'notebook-pen', phrase: 'took notes' },
    web: { label: 'Web', icon: 'globe', phrase: 'browsed the web' },
    audio: { label: 'Audio', icon: 'volume-2', phrase: 'worked with audio' },
    media: { label: 'Images', icon: 'image', phrase: 'made an image' },
    discord: { label: 'Discord', icon: 'channels', phrase: 'used Discord' },
    telegram: { label: 'Telegram', icon: 'paper-plane', phrase: 'used Telegram' },
    webui: { label: 'Messages', icon: 'chat', phrase: 'sent a message' },
    shell: { label: 'Shell', icon: 'square-terminal', phrase: 'ran commands' },
    coding: { label: 'Code', icon: 'code', phrase: 'worked on the code' },
    integrations: { label: 'Integrations', icon: 'addons', phrase: 'managed its integrations' },
    secrets: { label: 'Secrets', icon: 'key', phrase: 'used a secret' },
    questions: { label: 'Questions', icon: 'question', phrase: 'asked a question' },
    time: { label: 'Time', icon: 'clock', phrase: 'checked the time' },
    workspaces: { label: 'Workspaces', icon: 'briefcase', phrase: 'used a workspace' },
    subagents: { label: 'Subagents', icon: 'users', phrase: 'worked with subagents' },
    history: { label: 'Earlier output', icon: 'history', phrase: 'recalled earlier output' },
    custom_tools: { label: 'Managing functions', icon: 'functions', phrase: 'managed its functions' },
    ui: { label: 'Page', icon: 'layout', phrase: 'drew a page' },
    lookup: { label: 'Tool lookup', icon: 'list-tree', phrase: 'looked up its tools' },
    dreams: { label: 'Dream journal', icon: 'dreams', phrase: 'wrote in its journal' },
}

/** Discord and Telegram draw their own logos rather than the generic glyphs above. */
const BUILTIN_BRAND: Partial<Record<BuiltinCategory, BrandName>> = { discord: 'discord', telegram: 'telegram' }

const ADDONS: Record<AddonId, { label: string; glyph: CallGlyph; phrase: string }> = {
    brave_search: { label: 'Brave Search', glyph: { brand: 'brave' }, phrase: 'searched the web' },
    office: { label: 'Documents', glyph: { icon: 'file-text' }, phrase: 'worked with documents' },
}

const PROJECT = new Set<string>(PROJECT_TOOLS)

// Brands, by the words that name them. Checked before topics, so `github` is the logo, not a branch.
const BRAND_WORDS: Record<string, BrandName> = {
    github: 'github',
    gitlab: 'gitlab',
    bitbucket: 'bitbucket',
    sentry: 'sentry',
    vercel: 'vercel',
    cloudflare: 'cloudflare',
    docker: 'docker',
    kubernetes: 'kubernetes',
    k8s: 'kubernetes',
    linear: 'linear',
    jira: 'jira',
    confluence: 'confluence',
    notion: 'notion',
    obsidian: 'obsidian',
    trello: 'trello',
    asana: 'asana',
    todoist: 'todoist',
    airtable: 'airtable',
    figma: 'figma',
    slack: 'slack',
    whatsapp: 'whatsapp',
    discord: 'discord',
    telegram: 'telegram',
    gmail: 'gmail',
    googledrive: 'googledrive',
    gdrive: 'googledrive',
    googlecalendar: 'googlecalendar',
    gcal: 'googlecalendar',
    postgres: 'postgresql',
    postgresql: 'postgresql',
    supabase: 'supabase',
    firebase: 'firebase',
    mongo: 'mongodb',
    mongodb: 'mongodb',
    redis: 'redis',
    stripe: 'stripe',
    shopify: 'shopify',
    hubspot: 'hubspot',
    spotify: 'spotify',
    youtube: 'youtube',
    reddit: 'reddit',
    twitter: 'x',
    homeassistant: 'homeassistant',
    hass: 'homeassistant',
    brave: 'brave',
}

// Topics, by the words that suggest them. Whole words only: `digital` never reads as git.
const TOPIC_WORDS: Record<string, IconName> = {}
const topic = (icon: IconName, words: string) => {
    for (const word of words.split(' ')) TOPIC_WORDS[word] = icon
}
topic('globe', 'web browser browse fetch http https url scrape crawl playwright puppeteer')
topic('search', 'search find lookup')
topic('cloud-sun', 'weather forecast temperature climate')
topic('calendar', 'calendar event events meeting meetings booking schedule')
topic('mail', 'mail email emails inbox smtp imap')
topic('chat', 'chat message messages messaging sms')
topic('database', 'db database databases sql mysql sqlite')
topic('folder', 'file files fs filesystem drive storage')
topic('file-text', 'docs doc document documents notes wiki knowledge')
topic('code', 'code dev lint compile')
topic('square-terminal', 'shell terminal exec cli ssh')
topic('git-branch', 'git repo repos commit commits')
topic('map-pin', 'map maps geo location places directions')
topic('usage', 'finance stock stocks market trading crypto')
topic('credit-card', 'payment payments billing invoice invoices')
topic('shopping-cart', 'shop store cart order orders ecommerce')
topic('image', 'image images photo photos picture screenshot')
topic('video', 'video videos stream movie')
topic('music', 'music audio podcast')
topic('mic', 'speech voice transcribe')
topic('languages', 'translate translation language')
topic('clock', 'time clock timezone')
topic('calculator', 'math calc calculator')
topic('list-checks', 'todo todos task tasks issue issues ticket tickets')
topic('users', 'contacts crm people')
topic('chart-column', 'analytics metrics stats report reports dashboard')
topic('activity', 'monitor monitoring logs alerts observability')
topic('cloud', 'cloud deploy infra')
topic('shield', 'auth security vault')
topic('newspaper', 'news rss feed')
topic('plane', 'flight flights travel hotel')
topic('lightbulb', 'iot smarthome lights')
topic('skills', 'ai llm model generate')

/** Words that never decide an icon: the app's own pages, and words that could mean anything. */
const NEVER = new Set(['home', 'memory', 'core', 'query', 'mcp', 'server', 'api', 'tool', 'tools', 'my', 'the'])

/** The words of a name: split on `_`, `-`, `.`, `/` and case changes, lower-cased. */
export function words(name: string): string[] {
    return name
        .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
        .split(/[^A-Za-z0-9]+/)
        .map((w) => w.toLowerCase())
        .filter((w) => w && !NEVER.has(w))
}

/** A brand for the words of a name, joining neighbours too ("google_drive" is Google Drive). */
function brandOf(ws: string[]): BrandName | undefined {
    for (let i = 0; i < ws.length; i += 1) {
        const pair = i + 1 < ws.length ? BRAND_WORDS[ws[i] + ws[i + 1]] : undefined
        if (pair) return pair
        if (BRAND_WORDS[ws[i]]) return BRAND_WORDS[ws[i]]
    }
    return undefined
}

function topicOf(ws: string[]): IconName | undefined {
    for (let i = 0; i < ws.length; i += 1) {
        const pair = i + 1 < ws.length ? TOPIC_WORDS[ws[i] + ws[i + 1]] : undefined
        if (pair) return pair
        if (TOPIC_WORDS[ws[i]]) return TOPIC_WORDS[ws[i]]
    }
    return undefined
}

/**
 * The glyph for a name the trail does not know: a brand, else a topic, from the group's own name
 * first and the tool's name second; else a tile with the group's first letter.
 */
export function guessGlyph(group: string, tool = ''): CallGlyph {
    const own = words(group)
    const rest = words(tool)
    const brand = brandOf(own) ?? brandOf(rest)
    if (brand) return { brand }
    const icon = topicOf(own) ?? topicOf(rest)
    if (icon) return { icon }
    const letter = group.match(/[A-Za-z0-9]/)?.[0] ?? '?'
    return { letter }
}

/** The way core makes a script name from an MCP server key (`sandbox::docs::python_identifier`). */
export function pythonIdentifier(name: string): string {
    let ident = name.replace(/[^A-Za-z0-9_]/g, '_')
    if (!ident || /^[0-9]/.test(ident)) ident = '_' + ident
    return ident
}

const BRAND_LABEL: Partial<Record<BrandName, string>> = {
    github: 'GitHub',
    gitlab: 'GitLab',
    googledrive: 'Google Drive',
    googlecalendar: 'Google Calendar',
    postgresql: 'PostgreSQL',
    mongodb: 'MongoDB',
    hubspot: 'HubSpot',
    youtube: 'YouTube',
    homeassistant: 'Home Assistant',
    whatsapp: 'WhatsApp',
}

/** "acme_internal" → "Acme internal". */
function humanize(name: string): string {
    const text = name.replace(/[_-]+/g, ' ').trim()
    return text.charAt(0).toUpperCase() + text.slice(1)
}

/** A server's name in words; a recognised brand keeps its own spelling. */
function serverLabel(server: string, glyph: CallGlyph): string {
    if ('brand' in glyph && words(server).length <= 2) {
        const brand = glyph.brand
        return BRAND_LABEL[brand] ?? brand.charAt(0).toUpperCase() + brand.slice(1)
    }
    return humanize(server)
}

function mcp(server: string, tool: string): CallKind {
    const glyph = guessGlyph(server, tool)
    const label = `${serverLabel(server, glyph)} (MCP)`
    return { key: `mcp:${server}`, label, glyph, phrase: `used ${label}` }
}

/** Where one call belongs, by its name. */
export function kindOf(name: string, ctx: ToolContext = EMPTY_CONTEXT): CallKind {
    const builtin = BUILTIN_TOOLS[name] ?? UNCATEGORISED_TOOLS[name] ?? (PROJECT.has(name) ? 'coding' : undefined)
    if (builtin) {
        const d = BUILTIN[builtin]
        const brand = BUILTIN_BRAND[builtin]
        return { key: builtin, label: d.label, glyph: brand ? { brand } : { icon: d.icon }, phrase: d.phrase }
    }
    const addon = ADDON_IDS.find((id) => name.startsWith(id + '_'))
    if (addon) return { key: `addon:${addon}`, ...ADDONS[addon] }
    // A function's name never shadows a tool the agent can call (first-wins at the server), so
    // a name in the agent's list that is not a built-in is the function.
    const category = ctx.functions.get(name)
    if (category != null) {
        // One group per function category, drawn from the category's name (or the function's),
        // and the functions glyph when neither says anything.
        const generic = category === '' || category === 'custom'
        const glyph = guessGlyph(generic ? '' : category, name)
        const label = generic ? 'Functions' : `${humanize(category)} functions`
        return {
            key: `functions:${generic ? 'custom' : category}`,
            label,
            glyph: 'letter' in glyph ? { icon: 'functions' } : glyph,
            detail: name,
            phrase: 'ran its functions',
        }
    }
    // The longest server key that prefixes the name: `my_github` before `my`.
    const server = ctx.mcpServers
        .map(pythonIdentifier)
        .filter((key) => name.startsWith(key + '_'))
        .sort((a, b) => b.length - a.length)[0]
    if (server) return mcp(server, name.slice(server.length + 1))
    // Not readable or gone since: the first word is the best guess at the server.
    const cut = name.indexOf('_')
    return cut > 0 ? mcp(name.slice(0, cut), name.slice(cut + 1)) : mcp(name, '')
}

/** A script's calls grouped by where they belong, most-used first (ties: first used first). */
export function callItems(report: ExecutionReport | null, ctx: ToolContext = EMPTY_CONTEXT): CallIconItem[] {
    if (!report) return []
    const groups = new Map<string, CallIconItem & { first: number; names: Map<string, number> }>()
    report.tool_calls.forEach((call, index) => {
        const kind = kindOf(call.name, ctx)
        let group = groups.get(kind.key)
        if (!group) {
            group = { key: kind.key, label: kind.label, glyph: kind.glyph, count: 0, first: index, names: new Map() }
            groups.set(kind.key, group)
        }
        group.count += 1
        if (!call.ok) group.failed = true
        if (kind.detail) group.names.set(kind.detail, (group.names.get(kind.detail) ?? 0) + 1)
    })
    return [...groups.values()]
        .sort((a, b) => b.count - a.count || a.first - b.first)
        .map(({ first: _first, names, ...item }) =>
            names.size ? { ...item, details: [...names].map(([label, count]) => ({ label, count })) } : item
        )
}

/** The phrases of a run's groups, in the order of `callItems`. */
function phrases(report: ExecutionReport, ctx: ToolContext): string[] {
    const seen = new Map<string, string>()
    for (const item of callItems(report, ctx)) {
        const call = report.tool_calls.find((c) => kindOf(c.name, ctx).key === item.key)
        if (call) seen.set(item.key, kindOf(call.name, ctx).phrase)
    }
    return [...new Set(seen.values())]
}

/**
 * A row's words when the agent stated no intent: what the run used, as a sentence ("Searched the
 * web and used its memory"). At most three groups are named.
 */
export function describeRun(report: ExecutionReport | null, ctx: ToolContext = EMPTY_CONTEXT): string {
    if (!report || report.tool_calls.length === 0) return report ? 'Ran a script' : 'Working…'
    const parts = phrases(report, ctx)
    const named = parts.slice(0, 3)
    const more = parts.length - named.length
    if (more > 0) named.push(`${more} more`)
    const text =
        named.length === 1 ? named[0] : `${named.slice(0, -1).join(', ')} and ${named[named.length - 1]}`
    return text.charAt(0).toUpperCase() + text.slice(1)
}

/** The words of a direct tool row (the model's own lookups), in place of its name. */
export function toolRowLabel(name: string, args: Record<string, unknown>, ctx: ToolContext = EMPTY_CONTEXT): string {
    const category = typeof args.category === 'string' ? args.category : null
    if (name === 'list_tool_functions') return category ? `Looked up its ${category} tools` : 'Looked up its tools'
    if (name === 'describe_tool_function') return "Read a tool's guide"
    const kind = kindOf(name, ctx)
    return kind.phrase.charAt(0).toUpperCase() + kind.phrase.slice(1)
}
