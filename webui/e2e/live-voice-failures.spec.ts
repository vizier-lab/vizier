// NOT RUN in this environment: no server config in the tree (dummyplug is test-only, dev.vizier.yaml
// absent). Written against the B1-1 row; run against a server with VIZIER_URL set.
//
// T072 — the failures someone meets, and what they are offered afterwards. Every case here ends
// with the same question: can the person try again without reloading the page?
//
// One case is deliberately absent: a voice provider that fails mid-reply. `dummyplug` has no knob
// to make its voice refuse, so a spec here would assert its own precondition and pass for the wrong
// reason. That behaviour is covered where the decision is made — `speak_reply` in
// `crates/vizier-core/src/live_voice/session.rs` (FR-032: the error is published, the state returns
// to `listening`, the session is not ended) — rather than pretended at this level.

import { createPlugAgent, expect, login, test } from './fixtures'

const AGENT = 'e2e-live-fail'
const topic = () => `e2e-lf-${Date.now()}-${Math.floor(Math.random() * 1000)}`

const offered = (page: import('@playwright/test').Page) => page.getByRole('button', { name: 'Start live voice' })
const end = (page: import('@playwright/test').Page) => page.getByRole('button', { name: 'End the live voice session' })
const stateWord = (page: import('@playwright/test').Page) => page.locator('[role="status"][data-state]')

test.beforeEach(async ({ page }) => {
    await login(page)
    await createPlugAgent(page, AGENT)
})

test('T072 — a refused microphone is explained, and Try again works without a reload', async ({ page, context }) => {
    // The permission is refused through the browser, not by the app: this is the real path.
    await context.clearPermissions()
    await page.goto(`/${AGENT}/chat/${topic()}`)
    await expect(offered(page), 'the agent must offer live voice').toBeVisible({ timeout: 10_000 })

    // Playwright cannot dismiss a system prompt, so the refusal is injected where the browser would
    // report it. The message the row shows is the app's own, and that is what is asserted.
    await page.evaluate(() => {
        navigator.mediaDevices.getUserMedia = () =>
            Promise.reject(Object.assign(new Error('permission denied'), { name: 'NotAllowedError' }))
    })
    await offered(page).click()
    await expect(stateWord(page)).toHaveAttribute('data-state', 'failed', { timeout: 10_000 })
    const notice = await page.getByText(/microphone|permission|no device/i).first().textContent()
    expect(notice, 'the failure is a sentence a person can act on').toBeTruthy()

    // And the way out is on screen, not a page reload.
    const again = page.getByRole('button', { name: 'Try again' })
    await expect(again).toBeVisible()
    await page.evaluate(() => {
        // Restore permission so the second attempt can really open a device.
        delete (navigator.mediaDevices as unknown as Record<string, unknown>).getUserMedia
    })
    await again.click()
    await expect(end(page)).toBeVisible({ timeout: 10_000 })
    await end(page).click()
})

test('T072 — a session that ends can be started again, and the old reason does not follow', async ({ page }) => {
    await page.goto(`/${AGENT}/chat/${topic()}`)
    await expect(offered(page), 'the agent must offer live voice').toBeVisible({ timeout: 10_000 })
    await offered(page).click()
    await expect(end(page)).toBeVisible({ timeout: 10_000 })
    await end(page).click()
    await expect(stateWord(page)).toHaveAttribute('data-state', 'ended', { timeout: 10_000 })

    const again = page.getByRole('button', { name: 'Start again' })
    await expect(again).toBeVisible()
    await again.click()
    await expect(end(page)).toBeVisible({ timeout: 10_000 })
    await expect(stateWord(page)).not.toHaveAttribute('data-state', 'ended')
    await end(page).click()
})
