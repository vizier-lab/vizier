import AxeBuilder from '@axe-core/playwright'
import { createPlugAgent, expect, login, test } from './fixtures'

/** A unique agent per run, so its notifications are the only ones it can create. */
function uniqueAgent(prefix: string) {
    return `${prefix}-${Date.now().toString(36)}`
}

async function token(page: import('@playwright/test').Page): Promise<string> {
    const username = process.env.VIZIER_USER ?? 'admin'
    const password = process.env.VIZIER_PASSWORD ?? 'admin'
    const res = await page.request.post('/api/v1/auth/login', {
        data: { username, password },
    })
    const body = await res.json()
    return body.data?.token ?? body.token
}

/** An MCP server that never connects: creating it is enough for one notification. */
async function addIntegration(page: import('@playwright/test').Page, agent: string) {
    const res = await page.request.post(`/api/v1/agents/${agent}/integrations`, {
        headers: { Authorization: `Bearer ${await token(page)}` },
        data: {
            kind: 'mcp',
            name: 'e2e-mcp',
            settings: { transport: 'http', uri: 'http://127.0.0.1:1/mcp' },
        },
    })
    expect(res.ok(), `integration create: ${res.status()}`).toBeTruthy()
}

const USERNAME = process.env.VIZIER_USER ?? 'admin'

test.describe('Notifications (quickstart §10)', () => {
    test('the entry, the panel and a row all work from any screen', async ({ page }, info) => {
        test.skip(info.project.name.startsWith('mobile'), 'the desktop sidebar entry')
        const agent = uniqueAgent('e2e-notify')
        await login(page)
        await createPlugAgent(page, agent)
        await addIntegration(page, agent)

        await page.goto(`/${agent}/core`)
        const entry = page.getByRole('button', { name: /Notifications/ })
        await expect(entry).toBeVisible()
        await expect(entry).toContainText('1')

        await entry.click()
        const dialog = page.getByRole('dialog', { name: 'Notifications' })
        await expect(dialog).toBeVisible()
        await expect(dialog.getByText(`${USERNAME} added e2e-mcp.`)).toBeVisible()

        // The row is a destination: it opens the integrations page and marks itself read.
        await dialog.getByRole('button', { name: /added e2e-mcp/ }).click()
        await expect(page).toHaveURL(new RegExp(`/${agent}/integrations/mcp`))

        // Home never shows the removed away panel.
        await page.goto('/')
        await expect(page.getByText('Since you were away')).toHaveCount(0)
    })

    test('filters, per-row read state and mark-all stick across a reload', async ({ page }, info) => {
        test.skip(info.project.name.startsWith('mobile'), 'the desktop sidebar entry')
        const agent = uniqueAgent('e2e-notify-read')
        await login(page)
        await createPlugAgent(page, agent)
        await addIntegration(page, agent)

        await page.goto(`/${agent}/chat`)
        await page.getByRole('button', { name: /Notifications/ }).click()
        const dialog = page.getByRole('dialog', { name: 'Notifications' })
        const filters = dialog.getByRole('group', { name: 'Filter notifications' })

        // Opening the panel marks nothing read.
        await expect(dialog.getByRole('button', { name: /Unread/ })).toContainText('1')
        await dialog.getByRole('button', { name: 'Close' }).click()
        await page.getByRole('button', { name: /Notifications/ }).click()
        await expect(dialog.getByRole('button', { name: /Unread/ })).toContainText('1')

        // Unread holds the row; marking it read moves it to Read.
        await filters.getByRole('button', { name: /^Unread/ }).click()
        const row = dialog.getByRole('button', { name: /added e2e-mcp/ })
        await expect(row).toBeVisible()
        await dialog.getByRole('button', { name: 'Mark as read' }).click()
        await expect(row).toHaveCount(0)
        await filters.getByRole('button', { name: /^Read/ }).click()
        await expect(dialog.getByRole('button', { name: /added e2e-mcp/ })).toBeVisible()
        await dialog.getByRole('button', { name: 'Mark as unread' }).click()
        await expect(dialog.getByRole('button', { name: /added e2e-mcp/ })).toHaveCount(0)

        // The read state is the server's: a reload shows the same thing.
        await page.reload()
        await page.getByRole('button', { name: /Notifications/ }).click()
        const again = page.getByRole('dialog', { name: 'Notifications' })
        await expect(again.getByRole('button', { name: /Unread/ })).toContainText('1')
        await again.getByRole('button', { name: 'Mark all read' }).click()
        await expect(again.getByRole('button', { name: /Unread/ })).toContainText('0')
        await expect(again.getByText('You’re all caught up.')).toBeVisible()
    })

    test('the phone drawer opens a full-screen sheet without sideways scroll', async ({
        page,
    }, info) => {
        test.skip(!info.project.name.startsWith('mobile'), 'phone-sized projects only')
        const agent = uniqueAgent('e2e-notify-phone')
        await login(page)
        await createPlugAgent(page, agent)
        await addIntegration(page, agent)

        await page.goto(`/${agent}/chat`)
        await page.getByRole('button', { name: 'Open menu' }).click()
        await page.getByRole('dialog').getByRole('button', { name: /Notifications/ }).click()

        const dialog = page.getByRole('dialog', { name: 'Notifications' })
        await expect(dialog).toBeVisible()
        const box = await dialog.boundingBox()
        const viewport = page.viewportSize()
        expect(box?.width).toBe(viewport?.width)

        const { scroll, client } = await page.evaluate(() => ({
            scroll: document.documentElement.scrollWidth,
            client: document.documentElement.clientWidth,
        }))
        expect(scroll).toBeLessThanOrEqual(client)

        await dialog.getByRole('button', { name: 'Close' }).click()
        await expect(dialog).toBeHidden()
    })

    test('the panel has no serious accessibility violations', async ({ page }, info) => {
        test.skip(info.project.name.startsWith('mobile'), 'the desktop sidebar entry')
        const agent = uniqueAgent('e2e-notify-a11y')
        await login(page)
        await createPlugAgent(page, agent)
        await addIntegration(page, agent)
        await page.goto(`/${agent}/chat`)
        await page.getByRole('button', { name: /Notifications/ }).click()
        await expect(page.getByRole('dialog', { name: 'Notifications' })).toBeVisible()

        const results = await new AxeBuilder({ page })
            .include('[role="dialog"]')
            .analyze()
        const serious = results.violations.filter(
            (v) => v.impact === 'serious' || v.impact === 'critical'
        )
        expect(serious).toEqual([])
    })
})

test.describe('Notifications freshness', () => {
    test('a new notification and a read change reach a second open tab', async ({
        page,
    }, info) => {
        test.skip(info.project.name.startsWith('mobile'), 'the desktop sidebar entry')
        const agent = uniqueAgent('e2e-notify-tabs')
        await login(page)
        await createPlugAgent(page, agent)

        const other = await page.context().newPage()
        await page.goto(`/${agent}/chat`)
        await other.goto(`/${agent}/chat`)

        // Created while both tabs are open: the 15s count poll must pick it up.
        await addIntegration(page, agent)
        await expect(page.getByRole('button', { name: /Notifications/ })).toContainText(
            '1',
            { timeout: 20_000 }
        )
        await expect(other.getByRole('button', { name: /Notifications/ })).toContainText(
            '1',
            { timeout: 20_000 }
        )

        // Reading in one tab converges in the other without a reload.
        await page.getByRole('button', { name: /Notifications/ }).click()
        await other.getByRole('button', { name: /Notifications/ }).click()
        await page.getByRole('button', { name: 'Mark as read' }).click()
        await expect(
            other.getByRole('button', { name: /Notifications/ })
        ).not.toContainText('1', { timeout: 20_000 })
        await other.close()
    })
})
