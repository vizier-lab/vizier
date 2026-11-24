import { test as base, expect, type Page } from '@playwright/test'

const USERNAME = process.env.VIZIER_USER ?? 'admin'
const PASSWORD = process.env.VIZIER_PASSWORD ?? 'admin'

/** `HOLD(n)` keeps a dummyplug turn open for n seconds (quickstart.md). */
export const HOLD = (n: number) =>
    JSON.stringify({
        tool: 'execute_python',
        arguments: { intent: 'hold', code: `shell_exec(commands="sleep ${n}")` },
    })

export async function login(page: Page) {
    await page.goto('/login')
    await page.getByLabel(/username/i).fill(USERNAME)
    await page.getByLabel(/password/i).fill(PASSWORD)
    await page.getByRole('button', { name: /sign in/i }).click()
    await expect(page).not.toHaveURL(/\/login/)
}

async function token(page: Page): Promise<string> {
    const res = await page.request.post('/api/v1/auth/login', {
        data: { username: USERNAME, password: PASSWORD },
    })
    const body = await res.json()
    return body.data?.token ?? body.token
}

/** Creates an agent on the offline dummyplug provider through the API. */
export async function createPlugAgent(page: Page, id: string) {
    const headers = { Authorization: `Bearer ${await token(page)}` }
    const res = await page.request.post('/api/v1/agents', {
        headers,
        data: {
            agent_id: id,
            name: id,
            description: 'e2e dummyplug agent',
            provider: 'dummyplug',
            model: 'dummyplug',
        },
    })
    // 400 when the agent already exists: reused across runs.
    expect(res.ok() || res.status() === 400 || res.status() === 409).toBeTruthy()
    // A local shell addon, so HOLD(n) can keep a turn open (the addons redesign: shell is an addon now).
    await page.request.post(`/api/v1/agents/${id}/integrations`, {
        headers,
        data: { kind: 'shell', name: 'Shell', settings: { path: process.env.VIZIER_SHELL_DIR ?? '/tmp' } },
    })
}

/** Types into the chat composer and sends. */
export async function send(page: Page, text: string) {
    const box = page.getByRole('textbox').first()
    await box.fill(text)
    await box.press('Enter')
}

/**
 * A public share posted straight to the API, so a spec that is not about creating links has one to
 * open. The message carries markdown headings, which is what the viewer's outline is checked on (T078).
 */
export async function createShare(page: Page, options: { title?: string; markdown?: string } = {}): Promise<string> {
    const title = options.title ?? `e2e share ${Date.now()}`
    const snapshot = {
        format_version: 1,
        generator: 'e2e',
        created_at: Date.now(),
        title,
        source: { agent_name: 'e2e' },
        include_trail: false,
        turns: [
            {
                id: 't1',
                role: 'agent',
                at: Date.now(),
                parts: [{ kind: 'text', markdown: options.markdown ?? '# The plan\n\nA short answer.\n\n## The steps\n\n1. one\n2. two' }],
            },
        ],
        media: {},
        rehydration: null,
    }
    const res = await page.request.post('/api/v1/shares', {
        headers: { Authorization: `Bearer ${await token(page)}` },
        data: {
            agent_id: 'e2e-plug',
            topic_id: 'a11y',
            title,
            audience: 'public',
            include_trail: false,
            removals: { credentials: 0, paths: 0, recall: 0, reasoning: 0, media: 0 },
            snapshot,
        },
    })
    expect(res.status(), await res.text()).toBe(201)
    return (await res.json()).data.share_id
}

export const test = base
export { expect }
