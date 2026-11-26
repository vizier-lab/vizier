import { describe, expect, it } from 'vitest'
import type { ExecutionReport } from '../api/frames'
import { callItems, describeRun, guessGlyph, kindOf, toolRowLabel, words, type ToolContext } from './toolIcons'

const report = (calls: [string, boolean?][]): ExecutionReport => ({
    ok: true,
    result: null,
    stdout: '',
    duration_ms: 10,
    tool_calls: calls.map(([name, ok = true], i) => ({ seq: i + 1, name, ok, duration_ms: 1 })),
})

const ctx: ToolContext = {
    functions: new Map([
        ['weekly_digest', 'reports'],
        ['tidy_tags', 'custom'],
    ]),
    mcpServers: ['github', 'my_github', 'acme-internal', 'weather'],
}

describe('kindOf', () => {
    it('knows built-in tools exactly', () => {
        expect(kindOf('memory_search')).toMatchObject({ key: 'memory', label: 'Memory', glyph: { icon: 'memory' } })
        expect(kindOf('fetch')).toMatchObject({ key: 'web', glyph: { icon: 'globe' } })
        expect(kindOf('read_file')).toMatchObject({ key: 'coding' })
        expect(kindOf('discord_send_message')).toMatchObject({ glyph: { brand: 'discord' } })
        expect(kindOf('wake_session')).toMatchObject({ key: 'sessions', label: 'Conversations', glyph: { icon: 'activity' } })
        expect(kindOf('list_tool_functions')).toMatchObject({ key: 'lookup' })
    })

    it('knows addons by their id prefix', () => {
        expect(kindOf('brave_search_web')).toMatchObject({ key: 'addon:brave_search', label: 'Brave Search', glyph: { brand: 'brave' } })
        expect(kindOf('office_pdf_read_as_md')).toMatchObject({ key: 'addon:office' })
    })

    it('knows functions from the agent list, by their category', () => {
        expect(kindOf('weekly_digest', ctx)).toMatchObject({
            key: 'functions:reports',
            label: 'Reports functions',
            glyph: { icon: 'chart-column' },
            detail: 'weekly_digest',
        })
        // The default category, with nothing to go on: the functions glyph.
        expect(kindOf('tidy_tags', ctx)).toMatchObject({ key: 'functions:custom', label: 'Functions', glyph: { icon: 'functions' } })
    })

    it('matches the longest MCP server key that prefixes the name', () => {
        expect(kindOf('my_github_list_issues', ctx)).toMatchObject({ key: 'mcp:my_github', label: 'GitHub (MCP)', glyph: { brand: 'github' } })
        expect(kindOf('github_list_issues', ctx)).toMatchObject({ key: 'mcp:github' })
        // A key with a dash is matched as the script names it.
        expect(kindOf('acme_internal_sync', ctx)).toMatchObject({ key: 'mcp:acme_internal', glyph: { letter: 'a' } })
    })

    it('guesses from the name when the servers cannot be read', () => {
        expect(kindOf('linear_create_issue')).toMatchObject({ key: 'mcp:linear', glyph: { brand: 'linear' } })
        expect(kindOf('weather_get_forecast')).toMatchObject({ glyph: { icon: 'cloud-sun' } })
        expect(kindOf('zz_frobnicate')).toMatchObject({ glyph: { letter: 'z' } })
    })

    it('prefers a built-in over a lookalike prefix', () => {
        // `memory_search` is reserved before any MCP server or function could take it.
        expect(kindOf('memory_search', { functions: new Map([['memory_search', 'x']]), mcpServers: ['memory'] }).key).toBe('memory')
    })
})

describe('guessGlyph', () => {
    it('matches whole words only', () => {
        expect(guessGlyph('digital_signage')).toEqual({ letter: 'd' })
        expect(guessGlyph('git_tools')).toEqual({ icon: 'git-branch' })
    })

    it('puts brands before topics', () => {
        expect(guessGlyph('github')).toEqual({ brand: 'github' })
        expect(guessGlyph('notes_obsidian')).toEqual({ brand: 'obsidian' })
    })

    it('joins neighbouring words for a brand', () => {
        expect(guessGlyph('google_drive')).toEqual({ brand: 'googledrive' })
        expect(guessGlyph('GoogleCalendar')).toEqual({ brand: 'googlecalendar' })
    })

    it('never decides on words that could mean anything', () => {
        expect(guessGlyph('home_server')).toEqual({ letter: 'h' })
        expect(guessGlyph('my_query')).toEqual({ letter: 'm' })
    })

    it('reads the tool name when the group says nothing', () => {
        expect(guessGlyph('acme', 'get_forecast')).toEqual({ icon: 'cloud-sun' })
    })
})

describe('words', () => {
    it('splits on separators and case changes', () => {
        expect(words('getForecast-v2.now')).toEqual(['get', 'forecast', 'v2', 'now'])
    })
})

describe('callItems', () => {
    it('groups by category, most-used first, ties by first use', () => {
        const items = callItems(
            report([['fetch'], ['memory_search'], ['memory_read'], ['write_tmp'], ['fetch'], ['current_time']]),
            ctx
        )
        expect(items.map((i) => [i.key, i.count])).toEqual([
            ['web', 2],
            ['memory', 2],
            ['scratchpad', 1],
            ['time', 1],
        ])
    })

    it('marks a group failed when one of its calls failed', () => {
        const items = callItems(report([['memory_search'], ['github_list_issues', false]]), ctx)
        expect(items.find((i) => i.key === 'mcp:github')?.failed).toBe(true)
        expect(items.find((i) => i.key === 'memory')?.failed).toBeUndefined()
    })

    it('lists functions by name under their category', () => {
        const items = callItems(report([['weekly_digest'], ['tidy_tags'], ['weekly_digest']]), ctx)
        expect(items).toEqual([
            {
                key: 'functions:reports',
                label: 'Reports functions',
                glyph: { icon: 'chart-column' },
                count: 2,
                details: [{ label: 'weekly_digest', count: 2 }],
            },
            {
                key: 'functions:custom',
                label: 'Functions',
                glyph: { icon: 'functions' },
                count: 1,
                details: [{ label: 'tidy_tags', count: 1 }],
            },
        ])
    })

    it('is empty with no report or no calls', () => {
        expect(callItems(null)).toEqual([])
        expect(callItems(report([]))).toEqual([])
    })
})

describe('describeRun', () => {
    it('makes a sentence of what the run used', () => {
        expect(describeRun(report([['fetch'], ['memory_search']]))).toBe('Browsed the web and used its memory')
        expect(describeRun(report([['fetch']]))).toBe('Browsed the web')
    })

    it('names three and counts the rest', () => {
        expect(
            describeRun(report([['fetch'], ['memory_search'], ['write_tmp'], ['current_time'], ['shell_exec']]))
        ).toBe('Browsed the web, used its memory, took notes and 2 more')
    })

    it('says something for a run with no calls, and one still running', () => {
        expect(describeRun(report([]))).toBe('Ran a script')
        expect(describeRun(null)).toBe('Working…')
    })
})

describe('toolRowLabel', () => {
    it('words the sandbox lookups', () => {
        expect(toolRowLabel('list_tool_functions', { category: 'memory' })).toBe('Looked up its memory tools')
        expect(toolRowLabel('list_tool_functions', {})).toBe('Looked up its tools')
        expect(toolRowLabel('describe_tool_function', { name: 'fetch' })).toBe("Read a tool's guide")
    })
})
