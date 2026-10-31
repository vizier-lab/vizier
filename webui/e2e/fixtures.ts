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

export const test = base
export { expect }
