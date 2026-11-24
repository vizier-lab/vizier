import { createPlugAgent, createShare, expect, login, send, test } from './fixtures'

const AGENT = 'e2e-plug'

/** Every route the app has that needs a signed-in person, at the phone width the project sets. */
const ROUTES = [
    '/',
    '/agents/new',
    `/${AGENT}/chat`,
    `/${AGENT}/details`,
    `/${AGENT}/core`,
    `/${AGENT}/core/edit`,
    `/${AGENT}/memory`,
    `/${AGENT}/tasks`,
    `/${AGENT}/tasks/new/edit`,
    `/${AGENT}/skills`,
    `/${AGENT}/skills/new/edit?scope=agent`,
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
    '/settings/providers/add',
    '/settings/users',
    '/settings/roles',
    '/settings/shares',
]

test.describe('Phone layout (quickstart §8, SC-008)', () => {
    test.beforeEach(async ({ page }, info) => {
        test.skip(!info.project.name.startsWith('mobile'), 'phone-sized projects only')
        await login(page)
        await createPlugAgent(page, AGENT)
    })

    for (const route of ROUTES) {
        test(`${route} does not scroll sideways`, async ({ page }) => {
            await page.goto(route)
            await page.waitForLoadState('networkidle')
            const { scroll, client } = await page.evaluate(() => ({
                scroll: document.documentElement.scrollWidth,
                client: document.documentElement.clientWidth,
            }))
            expect(scroll, `${route} is ${scroll}px wide in a ${client}px window`).toBeLessThanOrEqual(client)
        })
    }

    test('Home: the agents and the Workspaces as lists, and a drawer for the app', async ({ page }) => {
        await page.goto('/')
        await expect(page.getByRole('button', { name: 'Open menu' })).toBeVisible()
        await expect(page.getByRole('navigation', { name: 'Main' })).toBeHidden()
        const agents = page.getByRole('region', { name: 'Agents' })
        await expect(agents).toBeVisible()
        await expect(agents.getByRole('link', { name: new RegExp(AGENT) })).toBeVisible()
        await expect(agents.getByRole('button', { name: 'New agent' })).toBeVisible()
        // The phone's Home is the lists: no greeting, no activity panel.
        await expect(page.getByRole('region', { name: 'The court' })).toHaveCount(0)

        await page.getByRole('button', { name: 'Open menu' }).click()
        const nav = page.getByRole('dialog').getByRole('navigation', { name: 'Main' })
        await expect(nav).toBeVisible()
        await expect(nav.getByRole('link', { name: 'Home' })).toBeVisible()
        await page.getByRole('dialog').getByRole('link', { name: 'Settings' }).click()
        await expect(page).toHaveURL(/\/settings\/profile/)
        // Choosing a page closes the drawer.
        await expect(page.getByRole('dialog')).toBeHidden()
    })

    test('Chat: the agent switches from its name and opens its details from the side', async ({ page }) => {
        await page.goto(`/${AGENT}/chat`)
        await expect(page.getByRole('button', { name: 'Open menu' })).toBeVisible()
        await expect(page.getByRole('navigation', { name: 'Main' })).toBeHidden()

        await page.getByRole('button', { name: `${AGENT}, switch agent` }).click()
        const sheet = page.getByRole('dialog', { name: 'Switch agent' })
        await expect(sheet).toBeVisible()
        // No "New agent" in the switcher: Home has it.
        await expect(sheet.getByText('New agent')).toHaveCount(0)
        await sheet.getByRole('button', { name: `${AGENT} details` }).click()
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/details$`))
        await expect(page.getByRole('dialog', { name: 'Switch agent' })).toBeHidden()
    })

    test('Agent details: every page of the agent, and back up from one', async ({ page }) => {
        await page.goto(`/${AGENT}/chat`)
        await page.getByRole('button', { name: `${AGENT} details` }).click()
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/details$`))
        for (const group of ['The agent', 'Essentials', 'Connections', 'Running it']) {
            await expect(page.getByRole('region', { name: group })).toBeVisible()
        }
        await page.getByRole('link', { name: 'Memory' }).click()
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/memory`))
        await page.getByRole('button', { name: `Back to ${AGENT}` }).click()
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/details$`))
    })

    test('Chat: the topics are a sheet', async ({ page }) => {
        await page.goto(`/${AGENT}/chat`)
        // The bar's second row: a fresh chat's topic is untitled.
        await page.getByRole('button', { name: 'Untitled conversation' }).click()
        await expect(page.getByRole('dialog', { name: 'Topics' })).toBeVisible()
        await page.keyboard.press('Escape')
        await expect(page.getByRole('dialog', { name: 'Topics' })).toBeHidden()
    })

    test('Share: the dialog fits the phone, the people picker works from the keyboard, Escape closes it', async ({ page }) => {
        await page.goto(`/${AGENT}/chat`)
        await page.waitForLoadState('networkidle')
        await send(page, `sharing from a phone ${Date.now()}`)
        const trigger = page.getByRole('button', { name: /^Share this conversation/ }).first()
        await expect(trigger).toBeEnabled({ timeout: 30_000 })
        await trigger.click()
        const dialog = page.getByRole('dialog', { name: 'Share this conversation' })
        await expect(dialog).toBeVisible()
        await expect(dialog.getByText(/ of 5\.0 MiB/)).toBeVisible({ timeout: 20_000 })

        // The panel is inside the phone (the × is in its header, the panel wraps both) and nothing widened the page.
        const width = page.viewportSize()?.width ?? 390
        const panel = dialog.getByRole('heading', { name: 'Share this conversation' }).locator('..')
        const box = await panel.boundingBox()
        expect(box?.x ?? -1, 'the panel starts inside the window').toBeGreaterThanOrEqual(0)
        expect((box?.x ?? 0) + (box?.width ?? width + 1), 'the panel ends inside the window').toBeLessThanOrEqual(width + 1)
        expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(0)

        // The picker is one keyboard path on a phone too: field, arrows, Escape clearing it.
        await dialog.getByRole('radio', { name: 'Specific people' }).check()
        const search = dialog.getByRole('searchbox', { name: 'Add people by username' })
        await search.focus()
        await page.keyboard.type('adm')
        await expect(dialog.locator('span[role="status"]')).toHaveText(/match|No matches/i, { timeout: 10_000 })
        const matches = dialog.getByRole('list', { name: 'Matching users' })
        if ((await matches.count()) > 0) {
            await page.keyboard.press('ArrowDown')
            await expect(matches.getByRole('button').first()).toBeFocused()
            await page.keyboard.press('ArrowUp')
            await expect(search).toBeFocused()
        }
        await page.keyboard.press('Escape')
        await expect(search).toHaveValue('')
        await page.keyboard.press('Escape')
        await expect(dialog).toBeHidden()
    })

    test('Share: the page of a shared conversation fits the phone and names its landmarks', async ({ page }) => {
        const id = await createShare(page, { title: 'A plan on a phone' })
        await page.goto(`/share/${id}`)
        await expect(page.getByTestId('snapshot-view')).toBeVisible({ timeout: 15_000 })
        await expect(page.getByRole('main', { name: 'Shared conversation' })).toBeVisible()
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('A plan on a phone')
        // A copy is read only on a phone too: no composer to answer with.
        await expect(page.getByRole('textbox')).toHaveCount(0)
        const { scroll, client } = await page.evaluate(() => ({
            scroll: document.documentElement.scrollWidth,
            client: document.documentElement.clientWidth,
        }))
        expect(scroll, `the share page is ${scroll}px wide in a ${client}px window`).toBeLessThanOrEqual(client)
    })
})

test.describe('Theme (quickstart §8.2)', () => {
    test('the toggle switches without a reload and is remembered on this device', async ({ page }) => {
        await login(page)
        await page.goto('/')
        const before = await page.evaluate(() => getComputedStyle(document.body).backgroundColor)
        let navigated = false
        page.on('framenavigated', () => (navigated = true))
        // The desktop sidebar's Theme item, or the drawer's on a phone.
        if ((page.viewportSize()?.width ?? 1440) < 1024) await page.getByRole('button', { name: 'Open menu' }).click()
        await page.getByRole('link', { name: 'Theme' }).or(page.getByRole('button', { name: 'Theme' })).first().click()
        await expect.poll(() => page.evaluate(() => getComputedStyle(document.body).backgroundColor)).not.toBe(before)
        expect(navigated).toBe(false)
        const stored = await page.evaluate(() => localStorage.getItem('vizier.theme'))
        expect(['dark', 'light']).toContain(stored)

        // The stored theme is applied before first paint, so a reload doesn't flash the other one.
        await page.reload()
        const attr = await page.evaluate(() => document.documentElement.getAttribute('data-theme'))
        expect(attr).toBe(stored)
    })
})
