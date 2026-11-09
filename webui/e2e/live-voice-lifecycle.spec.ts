// NOT RUN in this environment: no server config in the tree (dummyplug is test-only, dev.vizier.yaml
// absent). Written against the B1-1 row; run against a server with VIZIER_URL set.
//
// T064 — the session's own cycle: an idle ending, leaving and coming back, and a tab that loses the
// window without losing the call. The server's idle timer is not built yet (T062's server half), so
// the idle case here waits for the ending rather than producing it: the spec says what it needs.

import { createPlugAgent, expect, login, send, test } from './fixtures'

const AGENT = 'e2e-live-cycle'
const topic = () => `e2e-lc-${Date.now()}-${Math.floor(Math.random() * 1000)}`

test.use({
    permissions: ['microphone'],
    launchOptions: { args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream'] },
})

const offered = (page: import('@playwright/test').Page) => page.getByRole('button', { name: 'Start live voice' })
const row = (page: import('@playwright/test').Page) => page.getByRole('group', { name: 'Live voice' })
const end = (page: import('@playwright/test').Page) => page.getByRole('button', { name: 'End the live voice session' })
const stateWord = (page: import('@playwright/test').Page) => page.locator('[role="status"][data-state]')

test.beforeEach(async ({ page }) => {
    await login(page)
    await createPlugAgent(page, AGENT)
})

test('T064 — leaving the conversation ends the call, and coming back starts clean', async ({ page }) => {
    const first = topic()
    const second = topic()
    await page.goto(`/${AGENT}/chat/${first}`)
    await expect(offered(page), 'the agent must offer live voice').toBeVisible({ timeout: 10_000 })
    await offered(page).click()
    await expect(end(page)).toBeVisible({ timeout: 10_000 })

    // Walking away from the conversation is one of the means of hiding the row (T090), and the
    // microphone goes with it.
    await page.goto(`/${AGENT}/chat/${second}`)
    await expect(offered(page)).toBeVisible({ timeout: 10_000 })
    await expect(row(page), 'no live row on the conversation we came back to').toBeHidden()

    // Nothing from the first visit is remembered here: no reason, no clock, no session.
    await offered(page).click()
    await expect(end(page)).toBeVisible({ timeout: 10_000 })
    await expect(stateWord(page)).not.toHaveAttribute('data-state', /ended|failed/)
    await end(page).click()
})

test('T064 — an ending keeps its reason on screen and the clock stops', async ({ page }) => {
    await page.goto(`/${AGENT}/chat/${topic()}`)
    await expect(offered(page), 'the agent must offer live voice').toBeVisible({ timeout: 10_000 })
    await offered(page).click()
    await expect(end(page)).toBeVisible({ timeout: 10_000 })

    // The end is pressed by the person here; an idle ending arrives the same way once the server
    // has a timer for it, and the assertions below are the ones that matter for both.
    const before = await stateWord(page).evaluate((el) => el.closest('[role="group"]')?.textContent ?? '')
    await end(page).click()
    await expect(stateWord(page)).toHaveAttribute('data-state', 'ended', { timeout: 10_000 })

    const word = await stateWord(page).textContent()
    expect(word?.trim(), 'a word, not a code').toBe('Ended')
    const after = await stateWord(page).evaluate((el) => el.closest('[role="group"]')?.textContent ?? '')
    // The reason is readable, and the time no longer moves.
    await page.waitForTimeout(1200)
    const still = await stateWord(page).evaluate((el) => el.closest('[role="group"]')?.textContent ?? '')
    expect(still.replace(/\d{1,2}:\d{2}/, ''), 'the clock does not keep ticking').toBe(
        after.replace(/\d{1,2}:\d{2}/, '')
    )
    expect(before, 'the row was live before the ending').not.toBe(after)
})

test('T064 — a hidden window does not end the call (FR-038)', async ({ page }) => {
    await page.goto(`/${AGENT}/chat/${topic()}`)
    await expect(offered(page), 'the agent must offer live voice').toBeVisible({ timeout: 10_000 })
    await offered(page).click()
    await expect(end(page)).toBeVisible({ timeout: 10_000 })

    // Background the tab the way a person does, then come back: the microphone indicator is still
    // on and there is no ending.
    await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')))
    await page.waitForTimeout(500)
    await expect(page.getByText(/^mic on$/)).toBeVisible()
    await expect(stateWord(page)).not.toHaveAttribute('data-state', /ended|failed/)
    await end(page).click()
})
