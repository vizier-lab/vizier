// The WebUI names the calls on an activity trail from their tool names alone, with its own copy of
// which built-in tool is in which category (webui/app/lib/toolCategories.ts). This keeps that copy
// in step with the engine: the `every_tool_is_in_its_category` test in
// crates/vizier-core/src/agents/tools/mod.rs (its EXPECTED, ADDON and UNCATEGORISED tables) and
// the project coding tools in crates/vizier-core/src/projects/tools/. A tool added to core without
// the WebUI entry fails here and would otherwise be drawn as an unknown MCP server's tool.
//
//   node --test scripts/trail-tool-categories.test.mjs

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readdirSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const toolsMod = join(root, 'crates/vizier-core/src/agents/tools/mod.rs')
const projectTools = join(root, 'crates/vizier-core/src/projects/tools')
const projectShell = join(root, 'crates/vizier-core/src/projects/shell.rs')

/** `CustomTools` → `custom_tools`. */
const snake = (name) => name.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase()

/** The body of `const <name>: … = &[ … ];` inside the category test. */
function table(source, name) {
    const test = source.slice(source.indexOf('fn every_tool_is_in_its_category'))
    const match = test.match(new RegExp(`const ${name}: [^=]+= &\\[([\\s\\S]*?)\\n\\s*\\];`))
    assert.ok(match, `${name} table not found in ${toolsMod}`)
    return match[1]
}

/** The engine's tables, as the WebUI's shapes. */
export function engineTables(source) {
    const expected = Object.fromEntries(
        [...table(source, 'EXPECTED').matchAll(/\("([a-z0-9_]+)",\s*([A-Za-z]+)\)/g)].map((m) => [m[1], snake(m[2])])
    )
    const addons = [...new Set([...table(source, 'ADDON').matchAll(/\("[a-z0-9_]+",\s*"([a-z0-9_]+)"\)/g)].map((m) => m[1]))]
    const uncategorised = [...table(source, 'UNCATEGORISED').matchAll(/"([a-z0-9_]+)"/g)].map((m) => m[1])
    return { expected, addons, uncategorised }
}

/** Each coding tool's `fn name() -> String { "…" }`, and the project shell. */
function engineProjectTools() {
    const names = []
    for (const file of readdirSync(projectTools).filter((f) => f.endsWith('.rs'))) {
        const text = readFileSync(join(projectTools, file), 'utf8')
        const m = text.match(/fn name\(\) -> String \{\s*"([a-z0-9_]+)"/)
        if (m) names.push(m[1])
    }
    const shell = readFileSync(projectShell, 'utf8').match(/pub const NAME: &str = "([a-z0-9_]+)"/)
    if (shell) names.push(shell[1])
    return names.sort()
}

const webui = await import(pathToFileURL(join(root, 'webui/app/lib/toolCategories.ts')).href)
const engine = engineTables(readFileSync(toolsMod, 'utf8'))

test('every built-in tool has the same category in the WebUI', () => {
    assert.deepEqual(webui.BUILTIN_TOOLS, engine.expected)
})

test('the addon ids match', () => {
    assert.deepEqual([...webui.ADDON_IDS].sort(), [...engine.addons].sort())
})

test('every uncategorised tool is an addon tool, a sandbox tool the trail draws apart, or named', () => {
    const drawnApart = new Set(['think', 'execute_python'])
    const named = new Set(Object.keys(webui.UNCATEGORISED_TOOLS))
    const loose = engine.uncategorised.filter(
        (name) => !drawnApart.has(name) && !named.has(name) && !engine.addons.some((id) => name.startsWith(id + '_'))
    )
    assert.deepEqual(loose, [])
    for (const name of named) assert.ok(engine.uncategorised.includes(name), `${name} is no longer an uncategorised tool`)
})

test('the project coding tools match', () => {
    assert.deepEqual([...webui.PROJECT_TOOLS].sort(), engineProjectTools())
})
