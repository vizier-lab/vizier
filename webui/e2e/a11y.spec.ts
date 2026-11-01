import AxeBuilder from '@axe-core/playwright'
import { createPlugAgent, expect, login, test } from './fixtures'

const AGENT = 'e2e-plug'

const ROUTES = [
    '/',
    '/agents/new',
    `/${AGENT}/chat`,
    // The Core page is gone (spec 043): identity is reached at this document in Memory.
    `/${AGENT}/memory/core/soul`,
    `/${AGENT}/memory`,
    `/${AGENT}/tasks`,
    `/${AGENT}/skills`,
    `/${AGENT}/dream`,
    `/${AGENT}/usage`,
    `/${AGENT}/config`,
    `/${AGENT}/integrations/channels`,
    `/${AGENT}/integrations/addons`,
    `/${AGENT}/integrations/mcp`,
    '/settings/profile',
    '/settings/password',
    '/settings/api-keys',
    '/settings/providers',
    '/settings/users',
    '/settings/roles',
]

/** Serious or critical problems, named so a failure says where to look. */
async function violations(page: import('@playwright/test').Page) {
    const result = await new AxeBuilder({ page }).analyze()
    return result.violations
        .filter((v) => v.impact === 'serious' || v.impact === 'critical')
        .map((v) => `${v.id} (${v.impact}): ${v.nodes.slice(0, 3).map((n) => n.target.join(' ')).join(' | ')}`)
}

test.describe('Accessibility (quickstart §8.3, FR-020)', () => {
    test.beforeEach(async ({ page }, info) => {
        // One viewport is enough for axe; the colour scheme is the project's.
        test.skip(!info.project.name.startsWith('desktop'), 'checked at desktop width in both themes')
        await login(page)
        await createPlugAgent(page, AGENT)
    })

    for (const route of ROUTES) {
        test(`${route} has no serious or critical problems`, async ({ page }) => {
            await page.goto(route)
            await page.waitForLoadState('networkidle')
            expect(await violations(page)).toEqual([])
        })
    }

    test('Login has no serious or critical problems', async ({ browser }, info) => {
        test.skip(!info.project.name.startsWith('desktop'), 'desktop only')
        const context = await browser.newContext({ colorScheme: info.project.name.endsWith('dark') ? 'dark' : 'light' })
        const page = await context.newPage()
        await page.goto('/login')
        await page.waitForLoadState('networkidle')
        expect(await violations(page)).toEqual([])
        await context.close()
    })
})

test.describe('Keyboard only (quickstart §8.3)', () => {
    test.beforeEach(async ({}, info) => {
        test.skip(info.project.name !== 'desktop-dark', 'one pass is enough')
    })

    test('Login: tab through the fields and sign in with Enter', async ({ page }) => {
        await page.goto('/login')
        await page.keyboard.press('Tab')
        await expect(page.getByLabel(/username/i)).toBeFocused()
        await page.keyboard.type(process.env.VIZIER_USER ?? 'admin')
        await page.keyboard.press('Tab')
        await expect(page.getByLabel(/password/i)).toBeFocused()
        await page.keyboard.type(process.env.VIZIER_PASSWORD ?? 'admin')
        await page.keyboard.press('Enter')
        await expect(page).not.toHaveURL(/\/login/)
    })

    test('Chat: the composer takes focus from the keyboard and Enter sends', async ({ page }) => {
        await login(page)
        await createPlugAgent(page, AGENT)
        await page.goto(`/${AGENT}/chat`)
        const box = page.getByRole('textbox').first()
        await box.focus()
        const text = `hello from the keyboard ${Date.now()}`
        await page.keyboard.type(text)
        await page.keyboard.press('Enter')
        await expect(page.getByText(text).first()).toBeVisible()
    })

    test('Tasks: New task opens its page with the title focused, and Cancel goes back', async ({ page }) => {
        await login(page)
        await createPlugAgent(page, AGENT)
        await page.goto(`/${AGENT}/tasks`)
        await page.getByRole('button', { name: 'New task' }).first().focus()
        await page.keyboard.press('Enter')
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/tasks/new/edit$`))
        await expect(page.getByLabel('Title')).toBeFocused()
        await page.getByRole('button', { name: 'Cancel' }).focus()
        await page.keyboard.press('Enter')
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/tasks$`))
    })

    test('Roles: a role can be chosen and a permission toggled with the keyboard', async ({ page }) => {
        await login(page)
        await page.goto('/settings/roles')
        // The first role is built in, so its permissions can't be toggled: take the next one.
        const custom = page.getByRole('listitem').nth(1)
        await custom.focus()
        await page.keyboard.press('Enter')
        const box = page.getByRole('checkbox').first()
        await box.focus()
        await expect(box).toBeFocused()
        const before = await box.isChecked()
        await page.keyboard.press('Space')
        expect(await box.isChecked()).toBe(!before)
    })
})
