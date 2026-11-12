import AxeBuilder from '@axe-core/playwright'
import { createPlugAgent, expect, login, send, test } from './fixtures'

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
    '/settings/shares',
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

test.describe('Sharing (spec 056, T078)', () => {
    test.beforeEach(async ({ page }, info) => {
        test.skip(!info.project.name.startsWith('desktop'), 'checked at desktop width in both themes')
        await login(page)
        await createPlugAgent(page, AGENT)
        await page.goto(`/${AGENT}/chat`)
        // A message typed before the page has hydrated is lost; wait, then send.
        await page.waitForLoadState('networkidle')
        await send(page, `something to share ${Date.now()}`)
        await expect(page.getByRole('button', { name: /^Share this conversation$/ }).first()).toBeEnabled({ timeout: 30_000 })
    })

    const openDialog = async (page: import('@playwright/test').Page) => {
        await page.getByRole('button', { name: /^Share this conversation/ }).first().click()
        const dialog = page.getByRole('dialog', { name: 'Share this conversation' })
        await expect(dialog).toBeVisible()
        await expect(dialog.getByText(/ of 5\.0 MiB/)).toBeVisible({ timeout: 20_000 })
        return dialog
    }

    test('the Share dialog has no serious or critical problems, with the people picker open', async ({ page }) => {
        const dialog = await openDialog(page)
        await dialog.getByRole('radio', { name: 'Specific people' }).check()
        await expect(dialog.getByRole('searchbox', { name: 'Add people by username' })).toBeVisible()
        expect(await violations(page)).toEqual([])
    })

    test('keyboard: Tab stays inside the dialog, the people picker is reachable and Escape closes it', async ({ page }) => {
        const dialog = await openDialog(page)
        // focus moved in, and a long run of Tab never reaches the page behind
        for (let i = 0; i < 30; i++) {
            await page.keyboard.press('Tab')
            // A native modal <dialog> makes the page behind inert; after its last control Tab hands focus to the browser's own UI (activeElement is <body>), never to the page.
            const inside = await dialog.evaluate((d) => d.contains(document.activeElement) || document.activeElement === document.body)
            expect(inside, `Tab ${i + 1}: ${await page.evaluate(() => { const a = document.activeElement; return a ? `${a.tagName} ${a.getAttribute('aria-label') ?? a.textContent?.slice(0, 40)}` : 'none' })}`).toBe(true)
        }
        // the audience radios and the picker are on the keyboard path
        await dialog.getByRole('radio', { name: 'Specific people' }).focus()
        await page.keyboard.press('Space')
        const search = dialog.getByRole('searchbox', { name: 'Add people by username' })
        await search.focus()
        await expect(search).toBeFocused()
        await page.keyboard.type(process.env.VIZIER_USER ?? 'admin')
        // a match is a button: Tab to it, Enter adds the person, and the chip can be removed by keyboard
        const match = dialog.getByRole('list', { name: 'Matching users' }).getByRole('button').first()
        if (await match.isVisible({ timeout: 3000 }).catch(() => false)) {
            await match.focus()
            await page.keyboard.press('Enter')
            const remove = dialog.getByRole('button', { name: /^Remove / })
            await expect(remove).toBeVisible()
            await remove.focus()
            await page.keyboard.press('Enter')
            await expect(remove).toHaveCount(0)
        }
        await page.keyboard.press('Escape')
        // In a search box the first Escape clears the typed text (the browser's own behaviour); the next one closes.
        if (await dialog.isVisible()) await page.keyboard.press('Escape')
        await expect(dialog).toBeHidden()
    })

    test('a public share is not created until the confirmation is ticked (by keyboard)', async ({ page }) => {
        const dialog = await openDialog(page)
        const create = dialog.getByRole('button', { name: 'Create link' })
        await expect(create).toBeDisabled() // nothing chosen yet (FR-011)
        await dialog.getByRole('radio', { name: /Anyone with the link/ }).check()
        await expect(create).toBeDisabled() // chosen, not confirmed (FR-053)
        const box = dialog.getByRole('checkbox', { name: /anyone who has this link can read/i })
        await box.focus()
        await page.keyboard.press('Space')
        await expect(create).toBeEnabled()
    })
})
