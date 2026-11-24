// Syntax highlighting for fenced code (`specs`-less: see the CodeBlock wrap work). The design
// system knows tones, not grammars: `highlightCode` turns source into the `CodeLine[]` a
// `CodeBlock` draws, so the colour of a keyword is the design system's decision, not this file's.
//
// highlight.js is the grammar engine; `lowlight` is its hast wrapper, which is what lets a block
// be described as tokens instead of an HTML string (react-markdown must not receive markup here:
// `Markdown.tsx` reads a fence's children as text).
import type { CodeLine, CodeTone, CodeToken } from '@vizier/design'
import { createLowlight } from 'lowlight'
import bash from 'highlight.js/lib/languages/bash'
import css from 'highlight.js/lib/languages/css'
import diff from 'highlight.js/lib/languages/diff'
import ini from 'highlight.js/lib/languages/ini'
import javascript from 'highlight.js/lib/languages/javascript'
import json from 'highlight.js/lib/languages/json'
import markdown from 'highlight.js/lib/languages/markdown'
import python from 'highlight.js/lib/languages/python'
import rust from 'highlight.js/lib/languages/rust'
import sql from 'highlight.js/lib/languages/sql'
import typescript from 'highlight.js/lib/languages/typescript'
import xml from 'highlight.js/lib/languages/xml'
import yaml from 'highlight.js/lib/languages/yaml'

// Only the grammars the agent actually writes are bundled; an unregistered fence stays plain
// rather than pulling in all ~190 languages.
const lowlight = createLowlight({ bash, css, diff, ini, javascript, json, markdown, python, rust, sql, typescript, xml, yaml })

/** Fence names and file extensions that mean one of the registered grammars. */
const ALIASES: Record<string, string> = {
    py: 'python',
    python3: 'python',
    js: 'javascript',
    jsx: 'javascript',
    mjs: 'javascript',
    cjs: 'javascript',
    ts: 'typescript',
    tsx: 'typescript',
    sh: 'bash',
    shell: 'bash',
    zsh: 'bash',
    console: 'bash',
    yml: 'yaml',
    md: 'markdown',
    htm: 'xml',
    html: 'xml',
    svg: 'xml',
    toml: 'ini',
    rs: 'rust',
    jsonc: 'json',
    // The tool the agent writes most of its scripts for, used as a fence name by habit.
    execute_python: 'python',
    ipython: 'python',
}

/** A highlight.js class (`hljs-keyword`, `hljs-title function_`) as a design tone. */
const TONES: Record<string, CodeTone> = {
    keyword: 'keyword',
    'meta-keyword': 'keyword',
    built_in: 'keyword',
    literal: 'keyword',
    type: 'keyword',
    section: 'keyword',
    symbol: 'keyword',
    'selector-tag': 'keyword',
    string: 'string',
    regexp: 'string',
    'char.escape': 'string',
    subst: 'string',
    number: 'number',
    comment: 'comment',
    quote: 'comment',
    doctag: 'comment',
    title: 'function',
    'title.function': 'function',
    'title.class': 'function',
    'selector-id': 'function',
    operator: 'punctuation',
    punctuation: 'punctuation',
    tag: 'punctuation',
}

/** The parts of a hast node this file reads. */
type HastNode = {
    type: string
    value?: string
    properties?: { className?: unknown }
    children?: HastNode[]
}

/** The language a fence asked for, if one of the registered grammars is meant. */
export function resolveLanguage(language: string | undefined): string | undefined {
    const name = language?.trim().toLowerCase().split(/\s+/)[0]
    if (!name) return undefined
    const resolved = ALIASES[name] ?? name
    return lowlight.registered(resolved) ? resolved : undefined
}

/** The tone a highlighted node carries, from its `hljs-*` classes. A parent's tone is inherited. */
function toneOf(node: HastNode): CodeTone | undefined {
    const classes = Array.isArray(node.properties?.className) ? node.properties.className : []
    for (const name of classes) {
        if (typeof name !== 'string' || !name.startsWith('hljs-')) continue
        const tone = TONES[name.slice('hljs-'.length)]
        if (tone) return tone
    }
    return undefined
}

/** Source as one flat run per tone, which is then cut into lines. */
function flatten(nodes: HastNode[], inherited: CodeTone | undefined, out: CodeToken[]): void {
    for (const node of nodes) {
        const tone = toneOf(node) ?? inherited
        if (node.children) {
            flatten(node.children, tone, out)
            continue
        }
        if (typeof node.value !== 'string' || node.value === '') continue
        out.push(tone ? { text: node.value, tone } : { text: node.value })
    }
}

/**
 * `code` as lines of coloured tokens, or `undefined` when the fence names no language this
 * client highlights — the caller then draws the source plain, which is the old behaviour.
 *
 * Highlighting never throws: a grammar that trips over its input falls back to plain text
 * rather than taking the reply down with it.
 */
export function highlightCode(code: string, language: string | undefined): CodeLine[] | undefined {
    const resolved = resolveLanguage(language)
    if (!resolved) return undefined
    const source = code.replace(/\n$/, '')
    let flat: CodeToken[]
    try {
        flat = []
        flatten(lowlight.highlight(resolved, source).children as HastNode[], undefined, flat)
    } catch {
        return undefined
    }

    const lines: CodeLine[] = []
    let line: CodeToken[] = []
    for (const token of flat) {
        token.text.split('\n').forEach((part, i) => {
            if (i > 0) {
                lines.push(line)
                line = []
            }
            if (part !== '') line.push(token.tone ? { text: part, tone: token.tone } : { text: part })
        })
    }
    lines.push(line)
    return lines
}
