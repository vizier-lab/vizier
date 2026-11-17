// The agent's guide to pages must name exactly the kit's elements, attributes, events and libraries
// (specs/063-html-generative-ui T057): an element the guide leaves out is one the agent never uses,
// and one it invents draws a red box. The guide is split in two: REFERENCE.md (what `ui_render`'s
// guide returns: the libraries and a one-line index of the elements) and ELEMENTS.md (what
// `ui_describe_elements` returns: each element's attributes and events).
//
// The kit's own list is webui/app/kit/manifest.ts (read directly; Node strips its types), or the
// built webui/build/client/ui-kit/manifest.json when the WebUI's packages are not installed.
//
//   node --test scripts/ui-kit-reference.test.mjs

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const referencePath = join(root, 'crates/vizier-core/assets/ui-kit/REFERENCE.md')
const elementsPath = join(root, 'crates/vizier-core/assets/ui-kit/ELEMENTS.md')

async function kit() {
    try {
        const { manifestData } = await import(pathToFileURL(join(root, 'webui/app/kit/manifest.ts')).href)
        return manifestData()
    } catch {
        const built = join(root, 'webui/build/client/ui-kit/manifest.json')
        if (!existsSync(built)) return null
        return JSON.parse(readFileSync(built, 'utf8')).kit ?? null
    }
}

/** What the guide documents: `### \`<vz-x>\`` sections, their attribute rows and events, and libraries. */
export function documented(markdown) {
    const elements = new Map()
    const sections = markdown.split(/^### `<(vz-[a-z]+)>`$/m)
    for (let i = 1; i < sections.length; i += 2) {
        const body = sections[i + 1].split(/^#{2,3} /m)[0]
        const attributes = [...body.matchAll(/^\| `([a-z-]+)`( \*)? \|/gm)].map((m) => `${m[1]}${m[2] ? '*' : ''}`)
        const events = [...body.matchAll(/^- Event `([a-z]+)`/gm)].map((m) => m[1])
        elements.set(sections[i], { attributes: attributes.sort(), events: events.sort() })
    }
    const libraries = markdown.split(/^## Libraries$/m)[1]?.split(/^## /m)[0] ?? ''
    const libs = [...libraries.matchAll(/^\| `([a-z0-9@/._-]+)` \|/gm)].map((m) => m[1])
    return { elements, libraries: libs.sort() }
}

test('REFERENCE.md and ELEMENTS.md document exactly the kit', async (t) => {
    const manifest = await kit()
    if (!manifest) {
        t.skip('neither webui/app/kit/manifest.ts (with its packages) nor a built manifest is available')
        return
    }
    const guide = readFileSync(referencePath, 'utf8')
    const doc = documented(readFileSync(elementsPath, 'utf8'))
    const index = [...(guide.split(/^## Elements$/m)[1]?.split(/^## /m)[0] ?? '').matchAll(/^\| `(vz-[a-z]+)` \|/gm)].map((m) => m[1])

    const names = manifest.elements.map((e) => e.name).sort()
    assert.deepEqual([...doc.elements.keys()].sort(), names, 'the elements in ELEMENTS.md and the kit differ')
    assert.deepEqual(index.sort(), names, "the element index in REFERENCE.md and the kit differ")

    for (const element of manifest.elements) {
        const want = Object.entries(element.attributes)
            .map(([name, a]) => `${name}${a.required ? '*' : ''}`)
            .sort()
        const got = doc.elements.get(element.name)
        assert.deepEqual(got.attributes, want, `<${element.name}> attributes (a * marks required)`)
        assert.deepEqual(got.events, Object.keys(element.events).sort(), `<${element.name}> events`)
    }

    assert.deepEqual(documented(guide).libraries, [...manifest.libraries].sort(), 'the libraries in REFERENCE.md and the kit differ')
})

test('the parser reads a section the way it is written', () => {
    const doc = documented('## Elements\n\n### `<vz-a>`\n\n| Attribute | Type | |\n|---|---|---|\n| `x` * | string | y |\n| `y-z` | number | w |\n\n- Event `change`: e.\n\n### `<vz-b>`\n\nNone.\n\n## Libraries\n\n| Name | What |\n|---|---|\n| `three` | 3D |\n')
    assert.deepEqual(doc.elements.get('vz-a'), { attributes: ['x*', 'y-z'], events: ['change'] })
    assert.deepEqual(doc.elements.get('vz-b'), { attributes: [], events: [] })
    assert.deepEqual(doc.libraries, ['three'])
})
