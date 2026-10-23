import { createPlugAgent, expect, login, test } from './fixtures'

const AGENT = 'e2e-plug'

/** Every route the app has that needs a signed-in person, at the phone width the project sets. */
const ROUTES = [
    '/',
    '/agents/new',
    `/${AGENT}/chat`,
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

    test('Home: a top bar with the wordmark, a drawer for the nav, and the court as a list', async ({ page }) => {
        await page.goto('/')
        await expect(page.getByRole('button', { name: 'Open menu' })).toBeVisible()
        await expect(page.getByRole('navigation', { name: 'Main' })).toBeHidden()
        await expect(page.getByRole('region', { name: 'The court' })).toBeVisible()
        await expect(page.getByRole('img', { name: 'vizier' })).toBeVisible()

        await page.getByRole('button', { name: 'Open menu' }).click()
        const nav = page.getByRole('dialog').getByRole('navigation', { name: 'Main' })
        await expect(nav).toBeVisible()
        await nav.getByRole('link', { name: 'Settings' }).click()
        await expect(page).toHaveURL(/\/settings\/profile/)
        // Choosing a page closes the drawer.
        await expect(page.getByRole('dialog')).toBeHidden()
    })

    test('Chat: the top bar names the topic and the agent, and the panel is a sheet', async ({ page }) => {
        await page.goto(`/${AGENT}/chat`)
        await expect(page.getByRole('button', { name: 'Open menu' })).toBeVisible()
        await expect(page.getByRole('navigation', { name: 'Main' })).toBeHidden()
        await expect(page.getByRole('complementary', { name: 'Conversation details' })).toHaveCount(0)

        await page.getByRole('button', { name: 'Conversation details' }).click()
        await expect(page.getByRole('dialog', { name: 'Conversation details' })).toBeVisible()
        await expect(page.getByText('Recalled memory')).toBeVisible()
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
