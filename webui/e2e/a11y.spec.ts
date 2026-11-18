import AxeBuilder from '@axe-core/playwright'
import { createPlugAgent, createShare, expect, login, send, test } from './fixtures'

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

test.describe('Sharing (spec 063, T078)', () => {
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

    test('focus: opening the dialog moves focus in, it wraps inside, and closing returns it', async ({ page }) => {
        const trigger = page.getByRole('button', { name: /^Share this conversation/ }).first()
        await trigger.focus()
        await page.keyboard.press('Enter')
        const dialog = page.getByRole('dialog', { name: 'Share this conversation' })
        await expect(dialog).toBeVisible()
        // The native dialog places focus on its first control (the × in the header).
        await expect(dialog.getByRole('button', { name: 'Close' })).toBeFocused()
        // Backwards from the first control never reaches the page behind: the wrap ends on the last
        // control inside, or (at the very edge) on the browser's own UI, which reports as <body>.
        await page.keyboard.press('Shift+Tab')
        expect(await dialog.evaluate((d) => d.contains(document.activeElement) || document.activeElement === document.body)).toBe(true)
        for (let i = 0; i < 5; i++) {
            await page.keyboard.press('Shift+Tab')
            expect(await dialog.evaluate((d) => d.contains(document.activeElement) || document.activeElement === document.body), `Shift+Tab ${i + 2}`).toBe(true)
        }
        await page.keyboard.press('Escape')
        await expect(dialog).toBeHidden()
        await expect(trigger).toBeFocused()
    })

    test('labels: every control in the dialog has an accessible name, the icon-only ones included', async ({ page }) => {
        const dialog = await openDialog(page)
        await dialog.getByRole('radio', { name: 'Specific people' }).check()
        const search = dialog.getByRole('searchbox', { name: 'Add people by username' })
        await search.click()
        await page.keyboard.type('adm')
        // The live region is a span, so it is told apart from the Notices (which are also role=status).
        await expect(dialog.locator('span[role="status"]')).toHaveText(/match|No matches/i)
        const unnamed = await dialog.evaluate((d) =>
            Array.from(d.querySelectorAll<HTMLElement>('button, input, select, textarea, [role="switch"], [role="tab"]'))
                .filter((el) => {
                    const named =
                        el.getAttribute('aria-label')?.trim() ||
                        el.getAttribute('aria-labelledby') ||
                        (el.textContent ?? '').trim() ||
                        el.closest('label')?.textContent?.trim()
                    return !named
                })
                .map((el) => el.outerHTML.slice(0, 140))
        )
        expect(unnamed, 'controls without an accessible name').toEqual([])
        await expect(dialog.getByRole('button', { name: 'Close' })).toBeVisible()
        // The tab pair is one control: only the selected tab is in the tab order and the arrows move.
        const link = dialog.getByRole('tab', { name: 'Link' })
        await expect(link).toHaveAttribute('aria-selected', 'true')
        await expect(dialog.getByRole('tabpanel')).toBeVisible()
        await link.focus()
        await page.keyboard.press('ArrowRight')
        await expect(dialog.getByRole('tab', { name: 'Offline file' })).toBeFocused()
        await expect(dialog.getByRole('tab', { name: 'Offline file' })).toHaveAttribute('aria-selected', 'true')
    })

    test('keyboard: the people picker is walked with the arrow keys, picked with Enter and cleared with Escape', async ({ page }) => {
        const dialog = await openDialog(page)
        await dialog.getByRole('radio', { name: 'Specific people' }).check()
        const search = dialog.getByRole('searchbox', { name: 'Add people by username' })
        const matches = dialog.getByRole('list', { name: 'Matching users' })
        const status = dialog.locator('span[role="status"]')

        await search.click()
        await page.keyboard.type('adm')
        const first = matches.getByRole('button').first()
        await expect(first).toBeVisible({ timeout: 10_000 })
        // The count is announced, with the way to browse the results.
        await expect(status).toHaveText(/match.*down arrow/i)

        // Down from the field lands on the first result; the arrows walk the list and back to the field.
        await page.keyboard.press('ArrowDown')
        await expect(first).toBeFocused()
        if ((await matches.getByRole('button').count()) > 1) {
            await page.keyboard.press('ArrowDown')
            await expect(matches.getByRole('button').nth(1)).toBeFocused()
            await page.keyboard.press('ArrowUp')
            await expect(first).toBeFocused()
        }
        await page.keyboard.press('ArrowUp')
        await expect(search).toBeFocused()

        // Escape in the field clears the search instead of closing the dialog.
        await page.keyboard.press('Escape')
        await expect(search).toHaveValue('')
        await expect(matches).toHaveCount(0)
        await expect(dialog).toBeVisible()

        // Enter on a result picks that person; the field takes focus again and the count is announced.
        await page.keyboard.type('adm')
        await expect(first).toBeVisible({ timeout: 10_000 })
        await page.keyboard.press('ArrowDown')
        await page.keyboard.press('Enter')
        await expect(dialog.getByRole('button', { name: /^Remove / }).first()).toBeVisible()
        await expect(status).toHaveText(/person/i)

        // With nothing to clear, Escape closes the dialog (the native dialog's own cancel).
        await search.focus()
        await page.keyboard.press('Escape')
        await expect(dialog).toBeHidden()
    })

    test('the share page has named landmarks, one h1, ordered headings and no serious or critical problems', async ({ page }) => {
        const id = await createShare(page, { title: 'A shared plan' })
        await page.goto(`/share/${id}`)
        await expect(page.getByTestId('snapshot-view')).toBeVisible({ timeout: 15_000 })
        await expect(page.getByRole('main', { name: 'Shared conversation' })).toBeVisible()
        await expect(page.getByRole('banner')).toContainText('A shared plan')
        await expect(page.getByRole('contentinfo')).toContainText("This is a copy; the conversation can't continue here.")
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('A shared plan')

        // A message's markdown headings sit below the page's title: one h1, and no skipped level.
        const levels = await page.evaluate(() => Array.from(document.querySelectorAll('h1, h2, h3, h4, h5, h6')).map((h) => Number(h.tagName.slice(1))))
        expect(levels.filter((n) => n === 1), `headings ${levels}`).toHaveLength(1)
        expect(levels[0], `headings ${levels}`).toBe(1)
        for (let i = 1; i < levels.length; i++) expect(levels[i] - levels[i - 1], `heading ${i} of ${levels}`).toBeLessThanOrEqual(1)

        // Both projects run this: light and dark, where axe's colour-contrast rule is serious.
        expect(await violations(page)).toEqual([])
    })
})
