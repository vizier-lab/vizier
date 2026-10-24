// Checks what the agent sent to `ui_render` and, when it is wrong, says everything that is
// wrong in one message, with JSON paths, the valid options and the nearest names (contracts/
// generative-ui-tools.md §5). Runs in the tool handler, where the catalogue lives (research R4).
import { autoFixSpec, validateSpec, type Spec } from '@json-render/core'
import { ACTION_NAMES, catalog, COMPONENT_NAMES, type ComponentDef } from './catalog/index'
import { FUNCTION_NAMES } from './functions'
import { LIMITS } from './limits'
import { markdownLinks, parseHref, ALLOWED_SCHEMES } from './links'
import { availableFiles, fileReferences } from './media'
import { nearest } from './suggest'

export type UiSpec = Spec & { state?: Record<string, unknown>; logic?: Record<string, { params: string[]; body: string }>; format?: number }
export type Issue = { path: string; message: string }

/** What the validator may need to know about the conversation. */
export type ValidateContext = {
    /** The conversation's session files, for `file` references. `undefined` skips the check. */
    files?: { name: string; mime: string }[]
}

export type Validated = { ok: true; spec: UiSpec; title: string; fixed: string[] } | { ok: false; issues: Issue[] }

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)

/** An expression in place of a literal: `{ "$state": … }`, `{ "$computed": … }` and the rest. */
export const isExpression = (value: unknown): boolean => isObject(value) && Object.keys(value).some((key) => key.startsWith('$'))

const byteLength = (text: string) => new TextEncoder().encode(text).length

function quoted(values: readonly unknown[]): string {
    return values.map((v) => (typeof v === 'string' ? v : JSON.stringify(v))).join(', ')
}

/** The first `$item`, `$index` or `$bindItem` in a value, which only mean something inside a repeat. */
function itemExpression(value: unknown, depth = 0): string | null {
    if (depth > 24 || value === null || typeof value !== 'object') return null
    if (Array.isArray(value)) return value.map((v) => itemExpression(v, depth + 1)).find(Boolean) ?? null
    for (const key of ['$item', '$index', '$bindItem']) if (key in (value as object)) return key
    return Object.values(value as object).map((v) => itemExpression(v, depth + 1)).find(Boolean) ?? null
}

/** Every `$computed` and path in a value, checked structurally. */
function checkExpressions(value: unknown, path: string, issues: Issue[], depth = 0): void {
    if (depth > 24 || value === null || typeof value !== 'object') return
    if (Array.isArray(value)) {
        value.forEach((item, i) => checkExpressions(item, `${path}[${i}]`, issues, depth + 1))
        return
    }
    const object = value as Record<string, unknown>
    if ('$computed' in object) {
        const name = object.$computed
        if (typeof name !== 'string' || !FUNCTION_NAMES.includes(name)) {
            const hint = typeof name === 'string' ? nearest(name, FUNCTION_NAMES) : []
            issues.push({
                path: `${path}.$computed`,
                message: `${JSON.stringify(name)} is not a function. Valid: ${FUNCTION_NAMES.join(', ')}.${hint.length ? ` Did you mean "${hint[0]}"?` : ''}`,
            })
        }
    }
    for (const key of ['$state', '$bindState'] as const) {
        if (key in object && (typeof object[key] !== 'string' || !(object[key] as string).startsWith('/'))) {
            issues.push({ path: `${path}.${key}`, message: `must be a state path starting with "/", like "/form/name"` })
        }
    }
    for (const [key, child] of Object.entries(object)) checkExpressions(child, `${path}.${key}`, issues, depth + 1)
}

/** The props of one element against its component's schema. */
function checkProps(key: string, def: ComponentDef, props: unknown, issues: Issue[], ctx: ValidateContext): void {
    const base = `elements.${key}.props`
    if (props !== undefined && !isObject(props)) {
        issues.push({ path: base, message: 'props must be an object' })
        return
    }
    const given = (props ?? {}) as Record<string, unknown>
    const shape = def.props.shape
    const known = Object.keys(shape)

    for (const name of Object.keys(given)) {
        if (!(name in shape)) {
            const near = nearest(name, known)
            issues.push({
                path: `${base}.${name}`,
                message: `${def.name} has no prop "${name}". Props: ${known.join(', ') || '(none)'}.${near.length ? ` Did you mean "${near[0]}"?` : ''}`,
            })
        }
    }

    for (const [name, schema] of Object.entries(shape)) {
        const value = given[name]
        const field = schema as import('zod').ZodType
        if (value === undefined) {
            if (!field.safeParse(undefined).success) issues.push({ path: `${base}.${name}`, message: 'is required' })
            continue
        }
        checkExpressions(value, `${base}.${name}`, issues)
        if (isExpression(value)) continue
        const parsed = field.safeParse(value)
        if (parsed.success) continue
        for (const issue of parsed.error.issues.slice(0, 3)) {
            const sub = issue.path.length ? `${base}.${name}${issue.path.map((p) => (typeof p === 'number' ? `[${p}]` : `.${String(p)}`)).join('')}` : `${base}.${name}`
            if (issue.code === 'invalid_value' && 'values' in issue) {
                const shown = JSON.stringify(typeof value === 'string' ? value : value)
                issues.push({ path: sub, message: `${shown} is not allowed. Valid: ${quoted(issue.values as unknown[])}.` })
            } else if (issue.code === 'invalid_type') {
                issues.push({ path: sub, message: `must be ${('expected' in issue ? String(issue.expected) : 'a different type')}${issue.input === undefined ? '' : `, got ${Array.isArray(issue.input) ? 'a list' : typeof issue.input}`}.` })
            } else {
                issues.push({ path: sub, message: issue.message })
            }
        }
    }
    void ctx
}

type Binding = { action?: unknown; params?: unknown }

function bindingsOf(value: unknown): Binding[] {
    return (Array.isArray(value) ? value : [value]).filter(isObject) as Binding[]
}

function checkBinding(path: string, binding: Binding, logic: Set<string>, issues: Issue[]): void {
    const action = binding.action
    if (typeof action !== 'string' || !ACTION_NAMES.includes(action)) {
        const near = typeof action === 'string' ? nearest(action, ACTION_NAMES) : []
        issues.push({
            path: `${path}.action`,
            message: `${JSON.stringify(action)} is not an action. Valid: ${ACTION_NAMES.join(', ')}.${near.length ? ` Did you mean "${near[0]}"?` : ''}`,
        })
        return
    }
    const params = isObject(binding.params) ? binding.params : {}
    checkExpressions(params, `${path}.params`, issues)
    if (action === 'run') {
        const fn = params.fn
        if (typeof fn !== 'string') issues.push({ path: `${path}.params.fn`, message: 'is required: the name of a function in "logic"' })
        else if (!logic.has(fn)) {
            const near = nearest(fn, [...logic])
            issues.push({
                path: `${path}.params.fn`,
                message: `"${fn}" is not in "logic". ${logic.size ? `Defined: ${[...logic].join(', ')}.` : 'logic is empty.'}${near.length ? ` Did you mean "${near[0]}"?` : ''}`,
            })
        }
    }
    if (action === 'sendMessage' && typeof params.text !== 'string' && !isExpression(params.text)) {
        issues.push({ path: `${path}.params.text`, message: 'is required: the message to send' })
    }
    if (action === 'setState' && typeof params.statePath !== 'string') {
        issues.push({ path: `${path}.params.statePath`, message: 'is required: a state path like "/tab"' })
    }
}

function checkLogic(logic: unknown, issues: Issue[]): Set<string> {
    const names = new Set<string>()
    if (logic === undefined) return names
    if (!isObject(logic)) {
        issues.push({ path: 'spec.logic', message: 'must be an object of {"name": {"params": [...], "body": "..."}}' })
        return names
    }
    const entries = Object.entries(logic)
    if (entries.length > LIMITS.logicFunctions) issues.push({ path: 'spec.logic', message: `has ${entries.length} functions; the limit is ${LIMITS.logicFunctions}` })
    let bytes = 0
    for (const [name, fn] of entries) {
        names.add(name)
        const path = `spec.logic.${name}`
        if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) issues.push({ path, message: 'name must be a plain identifier' })
        if (!isObject(fn) || typeof fn.body !== 'string') {
            issues.push({ path, message: 'must be {"params": ["a"], "body": "return …"}' })
            continue
        }
        bytes += byteLength(fn.body)
        const params = fn.params
        if (!Array.isArray(params) || !params.every((p) => typeof p === 'string' && /^[A-Za-z_][A-Za-z0-9_]*$/.test(p))) {
            issues.push({ path: `${path}.params`, message: 'must be a list of identifiers' })
        } else if (params.length > LIMITS.logicParams) {
            issues.push({ path: `${path}.params`, message: `has ${params.length} params; the limit is ${LIMITS.logicParams}` })
        }
    }
    if (bytes > LIMITS.logicBytes) issues.push({ path: 'spec.logic', message: `source is ${bytes} bytes; the limit is ${LIMITS.logicBytes}` })
    return names
}

function depthOf(elements: Record<string, { children?: string[]; slots?: Record<string, string[]> }>, root: string): number {
    let deepest = 0
    const seen = new Set<string>()
    const walk = (key: string, depth: number) => {
        if (seen.has(`${key}:${depth}`) || depth > LIMITS.depth + 1) return
        seen.add(`${key}:${depth}`)
        deepest = Math.max(deepest, depth)
        const element = elements[key]
        if (!element) return
        for (const child of [...(element.children ?? []), ...Object.values(element.slots ?? {}).flat()]) walk(child, depth + 1)
    }
    walk(root, 1)
    return deepest
}

const SCHEMES = ALLOWED_SCHEMES.join(', ')

/** Links and files: addresses must follow the link rules, files must be files of this conversation. */
function checkMedia(elements: Record<string, { type?: string; props?: Record<string, unknown> }>, issues: Issue[], ctx: ValidateContext): void {
    for (const [key, element] of Object.entries(elements)) {
        const props = isObject(element?.props) ? element.props : {}
        const at = (prop: string) => `elements.${key}.props.${prop}`
        if (element?.type === 'Link') {
            if (typeof props.href === 'string' && !parseHref(props.href)) {
                issues.push({ path: at('href'), message: `${JSON.stringify(props.href)} is not allowed. Links take ${SCHEMES} addresses only (no credentials in the address).` })
            } else if (props.href === undefined && props.file === undefined && !isExpression(props.href)) {
                issues.push({ path: `elements.${key}.props`, message: 'Link needs href (an https, http or mailto address) or file (a conversation file to download)' })
            }
        }
        if (element?.type === 'Image') {
            // (An external `src` is refused by the prop's own rule.)
            if (props.src === undefined && props.file === undefined) {
                issues.push({ path: `elements.${key}.props`, message: 'Image needs file (a conversation file) or src (a data:image/ URL)' })
            }
        }
        if ((element?.type === 'Text' || element?.type === 'Callout') && typeof props.text === 'string') {
            for (const href of markdownLinks(props.text)) {
                if (!parseHref(href)) issues.push({ path: at('text'), message: `the link ${JSON.stringify(href)} is not allowed. Links take ${SCHEMES} addresses only.` })
            }
        }
    }

    const refs = fileReferences(elements)
    const distinct = new Set(refs.map((ref) => ref.name))
    if (distinct.size > LIMITS.distinctFiles) issues.push({ path: 'spec.elements', message: `refers to ${distinct.size} different files; the limit is ${LIMITS.distinctFiles}` })
    if (ctx.files) {
        const known = new Set(ctx.files.map((f) => f.name))
        const all = ctx.files.map((f) => ({ filename: f.name, mime_type: f.mime }))
        for (const ref of refs) {
            if (known.has(ref.name)) continue
            issues.push({
                path: ref.path,
                message: `${JSON.stringify(ref.name)} is not a file of this conversation. ${availableFiles(all as never)} Make the file first (see list_session_files).`,
            })
        }
    }
}

/** Check a `ui_render` call's arguments. */
export function validateUi(args: unknown, ctx: ValidateContext = {}): Validated {
    const issues: Issue[] = []
    if (!isObject(args)) return { ok: false, issues: [{ path: '', message: 'arguments must be an object {title, spec}' }] }

    const title = args.title
    if (typeof title !== 'string' || title.trim() === '') issues.push({ path: 'title', message: 'is required: a short title for screen readers' })
    else if (title.length > LIMITS.titleLength) issues.push({ path: 'title', message: `is ${title.length} characters; the limit is ${LIMITS.titleLength}` })

    const rawSpec = args.spec
    if (!isObject(rawSpec)) {
        issues.push({ path: 'spec', message: 'is required: {root, elements, state?, logic?}' })
        return { ok: false, issues }
    }
    const size = byteLength(JSON.stringify(rawSpec))
    if (size > LIMITS.specBytes) {
        return { ok: false, issues: [{ path: 'spec', message: `is ${Math.round(size / 1024)} KB; the limit is ${LIMITS.specBytes / 1024} KB. Make the interface smaller (fewer elements, shorter text, move rules into "logic").` }] }
    }

    const { spec: fixedSpec, fixDetails } = autoFixSpec(rawSpec as unknown as Spec, { lossy: false })
    // The fixer knows json-render's fields only; `logic` and `format` are ours and stay as sent.
    const spec = { ...rawSpec, ...fixedSpec } as UiSpec
    const fixed = fixDetails.filter((f) => !f.lossy).map((f) => f.message)

    for (const issue of validateSpec(spec).issues) {
        if (issue.severity !== 'error') continue
        issues.push({ path: issue.elementKey ? `elements.${issue.elementKey}` : 'spec', message: issue.message })
    }

    const elements = isObject(spec.elements) ? (spec.elements as unknown as Record<string, Record<string, unknown>>) : {}
    const keys = Object.keys(elements)
    if (keys.length > LIMITS.elements) issues.push({ path: 'spec.elements', message: `has ${keys.length} elements; the limit is ${LIMITS.elements}` })
    if (typeof spec.root === 'string' && elements[spec.root] && depthOf(elements as never, spec.root) > LIMITS.depth) {
        issues.push({ path: 'spec.elements', message: `nests deeper than ${LIMITS.depth} levels` })
    }

    const logic = checkLogic(spec.logic, issues)
    if (spec.state !== undefined && !isObject(spec.state)) issues.push({ path: 'spec.state', message: 'must be an object' })

    // Who each element's parent is, for the rule that 3D parts live inside a Scene3D.
    const parentOf = new Map<string, string>()
    for (const [key, element] of Object.entries(elements)) {
        if (!isObject(element)) continue
        const kids = [...((element.children as string[] | undefined) ?? []), ...Object.values((element.slots as Record<string, string[]> | undefined) ?? {}).flat()]
        for (const kid of kids) if (typeof kid === 'string') parentOf.set(kid, key)
    }
    const insideScene = (key: string): boolean => {
        for (let at = parentOf.get(key), hops = 0; at !== undefined && hops < LIMITS.depth + 2; at = parentOf.get(at), hops++) {
            const type = (elements[at] as { type?: string } | undefined)?.type
            if (type === 'Scene3D') return true
            if (type !== 'Group3D') return false
        }
        return false
    }

    // An element nothing reaches is never drawn, so a slider, a watch or an Effect on it does nothing.
    if (typeof spec.root === 'string' && elements[spec.root]) {
        const reached = new Set<string>()
        const visit = (key: string) => {
            if (reached.has(key) || !isObject(elements[key])) return
            reached.add(key)
            const el = elements[key] as Record<string, unknown>
            for (const kid of [...((el.children as string[] | undefined) ?? []), ...Object.values((el.slots as Record<string, string[]> | undefined) ?? {}).flat()]) if (typeof kid === 'string') visit(kid)
        }
        visit(spec.root)
        for (const key of keys) {
            if (!reached.has(key) && isObject(elements[key])) {
                issues.push({ path: `elements.${key}`, message: `is not reachable from root "${spec.root}", so it is never drawn and does nothing. Add "${key}" to a parent's children (an Effect or a watch only runs if its element is drawn).` })
            }
        }
    }
    // $item, $index and $bindItem need a repeat above them.
    const inRepeat = (key: string): boolean => {
        for (let at = parentOf.get(key), hops = 0; at !== undefined && hops < LIMITS.depth + 2; at = parentOf.get(at), hops++) {
            if (isObject((elements[at] as { repeat?: unknown } | undefined)?.repeat)) return true
        }
        return false
    }

    for (const key of keys) {
        const element = elements[key]
        const path = `elements.${key}`
        if (!isObject(element)) {
            issues.push({ path, message: 'must be an object {type, props, children}' })
            continue
        }
        const type = element.type
        const def = typeof type === 'string' ? catalog[type] : undefined
        if (!def) {
            const near = typeof type === 'string' ? nearest(type, COMPONENT_NAMES) : []
            issues.push({
                path: `${path}.type`,
                message: `${JSON.stringify(type)} is not a component. Valid: ${COMPONENT_NAMES.join(', ')}.${near.length ? ` Did you mean "${near[0]}"?` : ''}`,
            })
            continue
        }
        checkProps(key, def, element.props, issues, ctx)

        if (!inRepeat(key)) {
            // A repeat element's own `visible` filters its items, so it may name $item.
            const own = isObject(element.repeat) ? { props: element.props, on: element.on, watch: element.watch } : { props: element.props, on: element.on, watch: element.watch, visible: element.visible }
            for (const [where, value] of Object.entries(own)) {
                const used = itemExpression(value)
                if (used) issues.push({ path: `${path}.${where}`, message: `uses ${used}, which only works inside a repeat. Put "${key}" under an element that has "repeat", or use a literal value or a {"$state": …} path instead.` })
            }
        }
        if (def.parent && !insideScene(key)) issues.push({ path: `${path}.type`, message: `${def.name} is a 3D part: put it inside a Scene3D (or a Group3D inside one)` })
        if (def.children === 'three_d') {
            for (const child of (element.children as string[] | undefined) ?? []) {
                const childDef = catalog[(elements[child] as { type?: string } | undefined)?.type ?? '']
                if (childDef && !childDef.parent) issues.push({ path: `${path}.children`, message: `${def.name} holds 3D parts only; "${child}" is a ${childDef.name}` })
            }
        }
        if (def.name === 'Heightfield' && isObject(element.props)) {
            const { resolution, heights } = element.props
            if (typeof resolution === 'number' && Array.isArray(heights) && heights.length !== resolution * resolution) {
                issues.push({ path: `${path}.props.heights`, message: `has ${heights.length} numbers; resolution ${resolution} needs ${resolution * resolution} (resolution², row by row)` })
            }
        }
        if (isObject(element.repeat) && typeof element.repeat.statePath === 'string' && isObject(spec.state)) {
            const list = element.repeat.statePath.split('/').filter(Boolean).reduce<unknown>((at, part) => (isObject(at) || Array.isArray(at) ? (at as Record<string, unknown>)[part] : undefined), spec.state)
            if (Array.isArray(list) && list.length > LIMITS.repeatItems) issues.push({ path: `${path}.repeat`, message: `repeats ${list.length} items; the limit is ${LIMITS.repeatItems}` })
        }

        const children = [...((element.children as string[] | undefined) ?? []), ...Object.values((element.slots as Record<string, string[]> | undefined) ?? {}).flat()]
        if (def.children === 'none' && children.length > 0) issues.push({ path: `${path}.children`, message: `${def.name} cannot have children` })

        for (const [event, binding] of Object.entries(isObject(element.on) ? element.on : {})) {
            if (!def.events.includes(event)) {
                const near = nearest(event, def.events)
                issues.push({
                    path: `${path}.on.${event}`,
                    message: `${def.name} does not emit "${event}". ${def.events.length ? `Events: ${def.events.join(', ')}.` : 'It emits no events.'}${near.length ? ` Did you mean "${near[0]}"?` : ''}`,
                })
            }
            bindingsOf(binding).forEach((b, i) => checkBinding(`${path}.on.${event}[${i}]`, b, logic, issues))
        }
        for (const [watched, binding] of Object.entries(isObject(element.watch) ? element.watch : {})) {
            if (!watched.startsWith('/')) issues.push({ path: `${path}.watch.${watched}`, message: 'the key must be a state path starting with "/"' })
            bindingsOf(binding).forEach((b, i) => checkBinding(`${path}.watch.${watched}[${i}]`, b, logic, issues))
        }
        if (element.visible !== undefined) checkExpressions(element.visible, `${path}.visible`, issues)
    }

    checkMedia(elements as never, issues, ctx)

    if (issues.length > 0) return { ok: false, issues }
    return { ok: true, spec, title: String(title), fixed }
}

/** The error the agent reads (contracts/generative-ui-tools.md §5): every problem, capped, never the spec. */
export function formatIssues(issues: Issue[]): string {
    const shown = issues.slice(0, LIMITS.maxIssuesShown)
    const lines = shown.map((issue, i) => `${i + 1}. ${issue.path ? `${issue.path}: ` : ''}${issue.message}`)
    if (issues.length > shown.length) lines.push(`…and ${issues.length - shown.length} more.`)
    const heads = [...new Set(shown.map((issue) => /^elements\.([^.]+)\./.exec(issue.path)?.[1]).filter(Boolean))]
        .map((key) => key as string)
    void heads
    return `ui_render refused the interface (${issues.length} problem${issues.length === 1 ? '' : 's'}):\n${lines.join('\n')}\nFix these and call ui_render again. ui_components(names=[…]) shows exact props.`
}
