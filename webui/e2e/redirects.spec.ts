import { createPlugAgent, expect, login, test } from './fixtures'

const AGENT = 'e2e-plug'

test.describe('Legacy links (quickstart §9, FR-016)', () => {
    test.beforeEach(async ({ page }) => {
        await login(page)
        await createPlugAgent(page, AGENT)
    })

    test('an agent’s old settings page is its Config', async ({ page }) => {
        await page.goto(`/${AGENT}/settings`)
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/config`))
    })

    test('the old settings root is the profile', async ({ page }) => {
        await page.goto('/settings')
        await expect(page).toHaveURL(/\/settings\/profile/)
    })

    test('an old chat topic link opens that topic', async ({ page }) => {
        await page.goto(`/${AGENT}/chat/legacy-topic`)
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/chat/legacy-topic`))
        await expect(page.getByRole('textbox').first()).toBeVisible()
    })

    test('an unknown address goes home', async ({ page }) => {
        await page.goto('/nope')
        await expect(page).toHaveURL(/\/$/)
    })
})
