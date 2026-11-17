// The base of every `<vz-*>` element (specs/063-html-generative-ui tasks T052): a custom element
// whose inside is a React 19 root drawing a `@vizier/design` component.
//
// - Attributes are read against the element's spec (`../manifest.ts`). An invalid one draws an
//   inline problem box and is reported to the host as a `kit` problem, which the agent sees when
//   the version is checked, so it can fix the tag.
// - JSON attributes can also be set from script as properties (`table.rows = [...]`).
// - Children written inside the tag in HTML are kept: they are moved into `element.body`, which
//   the component draws where its content goes. Add content later by appending to `body`.
// - The element draws once the document has been parsed, so its children are all there.

import { type ReactNode, useLayoutEffect, useRef } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import type { ElementSpec } from '../manifest'
import { parseAttributes } from './parse'

export type RenderContext = {
    element: VzElement
    values: Record<string, unknown>
    /** Where the element's own children are drawn. */
    body: ReactNode
    /** Fire a DOM event from the element; it bubbles, and `detail` is what the spec says. */
    emit: (name: string, detail?: unknown) => void
}

export type Renderer = (ctx: RenderContext) => ReactNode

/** Report a problem to the host, like the frame runtime does for the page's own errors. */
export function reportKitProblem(message: string) {
    try {
        parent.postMessage({ vz: 1, type: 'problem', problem: { kind: 'kit', message: message.slice(0, 500), source: 'kit' } }, '*')
    } catch {
        // Not inside a frame (a test): nothing to tell.
    }
}

function Body({ node }: { node: HTMLElement }) {
    const ref = useRef<HTMLDivElement>(null)
    useLayoutEffect(() => {
        ref.current?.appendChild(node)
    }, [node])
    return <div ref={ref} className="contents" />
}

function ProblemBox({ tag, problems }: { tag: string; problems: string[] }) {
    return (
        <div
            role="alert"
            style={{
                border: '1px dashed var(--vz-failed-line)',
                background: 'var(--vz-failed-bg)',
                color: 'var(--vz-failed-text)',
                borderRadius: 'var(--vz-radius-md)',
                padding: '8px 10px',
                font: '12px/1.4 var(--vz-font-mono)',
            }}
        >
            <div style={{ fontWeight: 600 }}>&lt;{tag}&gt;</div>
            {problems.map((p) => (
                <div key={p}>{p}</div>
            ))}
        </div>
    )
}

export class VzElement extends HTMLElement {
    /** The element's own children, drawn where its content goes. */
    readonly body: HTMLElement = document.createElement('div')
    protected spec!: ElementSpec
    protected renderer!: Renderer
    private root: Root | null = null
    private props: Record<string, unknown> = {}
    private reported = new Set<string>()
    private scheduled = false

    constructor() {
        super()
        this.body.className = 'vz-body'
        this.body.style.display = 'contents'
        // The kit's base styles skip what an element draws but apply again to the page's own
        // content inside it (base.css, `@scope`).
        this.body.setAttribute('data-vz-body', '')
    }

    connectedCallback() {
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', () => this.mount(), { once: true })
        } else {
            this.mount()
        }
    }

    disconnectedCallback() {
        // Moved, not removed: React keeps the root; a removal unmounts on the next task.
        queueMicrotask(() => {
            if (!this.isConnected && this.root) {
                this.root.unmount()
                this.root = null
            }
        })
    }

    attributeChangedCallback() {
        this.update()
    }

    /** Set a JSON attribute's value from script. */
    protected setProp(name: string, value: unknown) {
        this.props[name] = value
        this.update()
    }

    protected getProp(name: string): unknown {
        if (this.props[name] !== undefined) return this.props[name]
        const raw = this.getAttribute(name)
        if (raw === null) return undefined
        try {
            return JSON.parse(raw)
        } catch {
            return undefined
        }
    }

    private mount() {
        if (this.root || !this.isConnected) return
        // What the page wrote inside the tag becomes the body.
        while (this.firstChild) this.body.appendChild(this.firstChild)
        this.setAttribute('data-vz-drawn', '')
        this.root = createRoot(this)
        this.draw()
    }

    private update() {
        if (!this.root || this.scheduled) return
        this.scheduled = true
        queueMicrotask(() => {
            this.scheduled = false
            this.draw()
        })
    }

    private draw() {
        if (!this.root) return
        const tag = this.spec.name
        const { values, problems } = parseAttributes(this.spec, (name) => this.getAttribute(name), this.props)
        for (const problem of problems) {
            if (this.reported.has(problem)) continue
            this.reported.add(problem)
            reportKitProblem(`<${tag}> ${problem}`)
        }
        const emit = (name: string, detail?: unknown) => this.dispatchEvent(new CustomEvent(name, { detail, bubbles: true, composed: true }))
        const body = <Body node={this.body} />
        this.root.render(
            <>
                {problems.length > 0 && <ProblemBox tag={tag} problems={problems} />}
                {this.renderer({ element: this, values, body, emit })}
            </>
        )
    }
}

/** Register `spec.name` as an element drawn by `render`, with properties for its JSON attributes. */
export function defineElement(spec: ElementSpec, render: Renderer, extend?: (cls: typeof VzElement) => void) {
    if (customElements.get(spec.name)) return
    const json = Object.entries(spec.attributes)
        .filter(([, a]) => a.type === 'json')
        .map(([n]) => n)
    class Element extends VzElement {
        static observedAttributes = Object.keys(spec.attributes)
        constructor() {
            super()
            this.spec = spec
            this.renderer = render
        }
    }
    for (const name of json) {
        Object.defineProperty(Element.prototype, name, {
            get(this: VzElement) {
                return (this as unknown as { getProp(n: string): unknown }).getProp(name)
            },
            set(this: VzElement, value: unknown) {
                ;(this as unknown as { setProp(n: string, v: unknown): void }).setProp(name, value)
            },
        })
    }
    extend?.(Element)
    customElements.define(spec.name, Element)
}
