// Reading a `<vz-*>` element's attributes against its spec (`../manifest.ts`), every problem at
// once, so the element can draw what is valid and say what is not.

import type { AttrSpec, ElementSpec } from '../manifest'

export type Parsed = { values: Record<string, unknown>; problems: string[] }

const clip = (text: string, max = 80) => (text.length > max ? `${text.slice(0, max)}…` : text)

function parseOne(name: string, spec: AttrSpec, raw: string): { value?: unknown; problem?: string } {
    switch (spec.type) {
        case 'string':
            return { value: raw }
        case 'number': {
            const n = Number(raw.trim())
            if (raw.trim() === '' || !Number.isFinite(n)) return { problem: `${name}: expected a number, got "${clip(raw)}"` }
            return { value: n }
        }
        case 'boolean':
            // The HTML way: present is true, unless it says "false".
            return { value: raw.trim().toLowerCase() !== 'false' }
        case 'enum':
            if (!spec.values?.includes(raw)) return { problem: `${name}: "${clip(raw)}" is not one of ${spec.values?.join(', ')}` }
            return { value: raw }
        case 'json': {
            let value: unknown
            try {
                value = JSON.parse(raw)
            } catch {
                return { problem: `${name}: not valid JSON (${clip(raw)})` }
            }
            return checkJson(name, spec, value)
        }
    }
}

function checkJson(name: string, spec: AttrSpec, value: unknown): { value?: unknown; problem?: string } {
    if (!spec.schema) return { value }
    const result = spec.schema.safeParse(value)
    if (result.success) return { value: result.data }
    const issue = result.error.issues[0]
    const at = issue.path.length ? `[${issue.path.join('.')}]` : ''
    return { problem: `${name}${at}: ${issue.message}` }
}

/**
 * The element's attributes as values. `props` holds JSON values set from script as properties
 * (`table.rows = [...]`), which win over the attribute of the same name.
 */
export function parseAttributes(spec: ElementSpec, read: (name: string) => string | null, props: Record<string, unknown> = {}): Parsed {
    const values: Record<string, unknown> = {}
    const problems: string[] = []
    for (const [name, attr] of Object.entries(spec.attributes)) {
        let result: { value?: unknown; problem?: string }
        if (attr.type === 'json' && props[name] !== undefined) {
            result = checkJson(name, attr, props[name])
        } else {
            const raw = read(name)
            if (raw === null) {
                if (attr.required) problems.push(`${name} is required`)
                continue
            }
            result = parseOne(name, attr, raw)
        }
        if (result.problem) problems.push(result.problem)
        else values[name] = result.value
    }
    return { values, problems }
}
