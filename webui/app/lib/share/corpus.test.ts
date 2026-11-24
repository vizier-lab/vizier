import { describe, expect, it } from 'vitest'
import type { ChatMessage } from '../../api/frames'
import { loadConversation, pngDeps } from './testing'
import { serializeEntries } from './serialize'

// SC-003: over a seeded corpus, nothing planted survives in the serialized document.
const fx = loadConversation()

function rng(seed: number) {
    return () => {
        seed |= 0
        seed = (seed + 0x6d2b79f5) | 0
        let t = Math.imul(seed ^ (seed >>> 15), 1 | seed)
        t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
        return ((t ^ (t >>> 14)) >>> 0) / 4294967296
    }
}

const ALNUM = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
const UP = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'

function make(seed: number) {
    const r = rng(seed)
    const pick = (chars: string, n: number) => Array.from({ length: n }, () => chars[Math.floor(r() * chars.length)]).join('') + '7'
    const secretMakers: (() => string)[] = [
        () => `sk-proj-${pick(ALNUM, 40)}`,
        () => `sk-ant-api03-${pick(ALNUM, 50)}`,
        () => `AIza${pick(ALNUM, 34).slice(0, 35)}`,
        () => `AKIA${pick(UP, 15)}`,
        () => `ghp_${pick(ALNUM, 36)}`,
        () => `github_pat_${pick(ALNUM, 40)}`,
        () => `xoxb-${pick('0123456789', 11)}-${pick(ALNUM, 20)}`,
        () => `sk_live_${pick(ALNUM, 24)}`,
        () => `eyJ${pick(ALNUM, 20)}.eyJ${pick(ALNUM, 24)}.${pick(ALNUM, 30)}`,
    ]
    const secrets: string[] = []
    const secret = () => {
        const s = secretMakers[Math.floor(r() * secretMakers.length)]()
        secrets.push(s)
        return s
    }
    const pem = () => {
        const body = pick(ALNUM, 60)
        secrets.push(body)
        return `-----BEGIN PRIVATE KEY-----\n${body}\n${pick(ALNUM, 60)}\n-----END PRIVATE KEY-----`
    }
    const password = () => {
        const pw = pick(ALNUM, 12)
        secrets.push(pw)
        return `password=${pw}`
    }
    const dirs: string[] = []
    const path = () => {
        const dir = `/home/user${pick('abcdefgh', 6)}/proj${pick('xyz', 4)}`
        dirs.push(dir)
        const win = `C:\\Users\\win${pick('abcdefgh', 6)}\\Docs${pick('xyz', 3)}`
        dirs.push(win)
        return r() < 0.5 ? `${dir}/${pick('abc', 5)}.txt` : `${win}\\${pick('abc', 5)}.txt`
    }
    const markers: string[] = []
    const marker = (k: string) => {
        const m = `${k}-MARK-${pick('QWERTYUIOP', 12)}`
        markers.push(m)
        return m
    }
    const S = fx.entries[0].vizier_session
    let n = 0
    const e = (content: object): ChatMessage => ({ uid: `u${++n}`, seq: n, timestamp: new Date(1_700_000_000_000 + n * 1000).toISOString(), vizier_session: S, content } as ChatMessage)
    const sprinkle = () => [secret, pem, password, path, path][Math.floor(r() * 5)]()
    const entries: ChatMessage[] = []
    const turns = 2 + Math.floor(r() * 4)
    for (let i = 0; i < turns; i++) {
        entries.push(e({ Request: { timestamp: 'x', user: 'u', content: { chat: `hello ${sprinkle()} and ${sprinkle()}` } } }))
        entries.push(e({ Recall: { origin: 'auto', passages: [{ text: marker('RECALL') }] } }))
        entries.push(e({ Response: { timestamp: 'x', content: { thinking: marker('THINK') } } }))
        entries.push(e({ SensitiveAsk: { ask_id: 'a', purpose: marker('ASK'), state: 'answered' } }))
        entries.push(e({ ToolCall: { call_id: `c${i}`, name: 'shell_exec', arguments: { command: `run ${sprinkle()}`, env: { note: sprinkle() } } } }))
        entries.push(e({ ToolResult: { call_id: `c${i}`, content: `out ${sprinkle()}\n${sprinkle()}` } }))
        entries.push(e({ QuestionSet: { set_id: `q${i}`, title: `T ${sprinkle()}`, questions: [{ id: 'a', prompt: `Use ${sprinkle()}?`, type: 'text' }], state: 'answered', answers: [{ question_id: 'a', state: 'answered', value: sprinkle() }], answerer: marker('WHO') } }))
        entries.push(e({ ClientToolCall: { name: 'ui_render', category: 'ui', tool: 'render', arguments: { title: `P ${sprinkle()}`, spec: { root: 'a', elements: { a: { type: 'Text', props: { text: sprinkle() } } } } }, state: { v: sprinkle() } } }))
        entries.push(e({ Checkpoint: marker('CKPT') }))
        entries.push(e({ Command: marker('CMD') }))
        entries.push(e({ Response: { timestamp: 'x', content: { message: { content: `done ${sprinkle()}` } } } }))
    }
    return { entries, secrets, dirs, markers, title: `Chat ${secret()} ${path()}` }
}

describe('corpus: no planted seed survives (SC-003)', () => {
    for (const includeTrail of [false, true]) {
        it(`50 seeded conversations, trail ${includeTrail ? 'on' : 'off'}`, async () => {
            for (let seed = 1; seed <= 50; seed++) {
                const c = make(seed)
                const { snapshot } = await serializeEntries(c.entries, { agentName: 'Ada', title: c.title, includeTrail, media: pngDeps(fx.pngBase64), now: 1, turnInProgress: false })
                const doc = JSON.stringify(snapshot)
                for (const s of [...c.secrets, ...c.dirs, ...c.markers]) {
                    if (doc.includes(s) || doc.includes(JSON.stringify(s).slice(1, -1))) throw new Error(`seed ${seed}, trail ${includeTrail}: "${s.slice(0, 30)}" is in the document`)
                }
                expect(snapshot.title).not.toContain('/home/')
            }
        })
    }
})
