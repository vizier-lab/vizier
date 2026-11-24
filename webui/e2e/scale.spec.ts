import type { Page } from '@playwright/test'
import { expect, login, test } from './fixtures'

/**
 * Lists at scale (quickstart §6, SC-004): run against a vizier seeded by `scripts/seed-scale.mjs`
 * (50 agents, 300 tasks, 1,000 bundles, 600 conversations, 1,000 roles). Every list page shows its
 * first content in under 2 s, and no response carries more than one page.
 */
const FIRST_CONTENT_MS = 2000
const PAGE_MAX = 100

type Listing = { url: string; items: number; bytes: number }

/** Records every list response the page fetches, so the page size can be checked afterwards. */
function watchLists(page: Page): Listing[] {
    const seen: Listing[] = []
    page.on('response', async (res: import('@playwright/test').Response) => {
        const url = res.url()
        if (!url.includes('/api/v1/') || res.request().method() !== 'GET') return
        const body = await res.body().catch(() => undefined)
        if (!body) return
        let json: { data?: { items?: unknown[] } | unknown[] } | undefined
        try {
            json = JSON.parse(body.toString())
        } catch {
            return
        }
        const data = json?.data
        const items = Array.isArray(data) ? undefined : data?.items
        // A plain array is only a list when it is one of the unpaged endpoints we know are small.
        if (items) seen.push({ url, items: items.length, bytes: body.length })
    })
    return seen
}

async function firstContent(page: Page, path: string, ready: () => Promise<unknown>): Promise<number> {
    const started = Date.now()
    await page.goto(path)
    await ready()
    return Date.now() - started
}

test.describe('Lists at scale (quickstart §6)', () => {
    test.beforeEach(async ({ page }) => {
        const status = await (await page.request.get('/api/v1/status')).json()
        test.skip(status.data?.setup_required, 'needs a set-up vizier')
        await login(page)
        const token = await page.evaluate(() => localStorage.getItem('auth_token') ?? '')
        const agents = await (await page.request.get('/api/v1/agents?limit=1', { headers: { Authorization: `Bearer ${token}` } })).json()
        test.skip((agents.data?.counts?.all ?? 0) < 50, 'needs the seeded data: node scripts/seed-scale.mjs')
    })

    const pages: { name: string; path: string; ready: (page: Page) => Promise<unknown> }[] = [
        { name: 'Home', path: '/', ready: (p) => expect(p.getByRole('region', { name: 'The court' }).getByRole('link').first()).toBeVisible() },
        { name: 'Tasks', path: '/seed-000/tasks', ready: (p) => expect(p.getByRole('row', { name: /Seed task/ }).first()).toBeVisible() },
        { name: 'Memory bundles', path: '/seed-000/memory', ready: (p) => expect(p.getByText(/seed-bundle-/).first()).toBeVisible() },
        { name: 'Roles', path: '/settings/roles', ready: (p) => expect(p.getByText(/seed-role-/).first()).toBeVisible() },
        { name: 'Users', path: '/settings/users', ready: (p) => expect(p.getByRole('row', { name: /admin/ }).first()).toBeVisible() },
    ]

    for (const { name, path, ready } of pages) {
        test(`${name}: first content under 2 s, one page per response`, async ({ page }) => {
            const lists = watchLists(page)
            const ms = await firstContent(page, path, () => ready(page))
            expect(ms, `${name} took ${ms} ms`).toBeLessThan(FIRST_CONTENT_MS)
            await page.waitForLoadState('networkidle')
            expect(lists.length, 'the page asked for no list').toBeGreaterThan(0)
            for (const l of lists) expect(l.items, `${l.url} returned ${l.items} items`).toBeLessThanOrEqual(PAGE_MAX)
        })
    }

    test('The conversation picker pages 600 conversations 25 at a time', async ({ page }) => {
        await page.goto('/seed-000/chat')
        const asked = page.waitForResponse((r) => r.url().includes('/conversations') && r.request().method() === 'GET')
        await page.getByRole('button', { name: /^.*topic.*$/i }).first().click()
        const started = Date.now()
        const body = await (await asked).json()
        await expect(page.getByRole('option').first()).toBeVisible()
        expect(Date.now() - started).toBeLessThan(FIRST_CONTENT_MS)
        expect(body.data.items.length).toBeLessThanOrEqual(25)
        expect(body.data.total).toBeGreaterThanOrEqual(600)
        await expect(page.getByText(/more · scroll to load/)).toBeVisible()
    })

    test('The conversation picker searches the server, not the page', async ({ page }) => {
        await page.goto('/seed-000/chat')
        await page.getByRole('button', { name: /^.*topic.*$/i }).first().click()
        await page.getByPlaceholder('Search conversations').fill('seed')
        const res = page.waitForResponse((r) => r.url().includes('/conversations') && r.url().includes('q=seed'))
        expect((await res).ok()).toBeTruthy()
    })
})
