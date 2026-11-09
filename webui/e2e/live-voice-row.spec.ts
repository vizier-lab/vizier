// NOT RUN in this environment: no server config in the tree (dummyplug is test-only, dev.vizier.yaml
// absent). Written against the B1-1 row; run against a server with VIZIER_URL set.
//
// What the server must offer for these specs to be meaningful: the agent created below has to come
// back with `live_voice: true`. Without a recogniser and a voice in its config, `LiveVoiceRow` draws
// the disabled pill (a `span` with the reason in `title`, no button), and `expectOffered` fails on
// purpose rather than letting the file pass by never opening a session.
//
// The fake microphone is per-file, so `playwright.config.ts` stays as it is: Chromium is granted a
// device and permission automatically, and the session opens without a system prompt.

import type { Page } from '@playwright/test'
import { createPlugAgent, expect, login, send, test } from './fixtures'

const AGENT = 'e2e-live-voice'
const topic = () => `e2e-lv-${Date.now()}-${Math.floor(Math.random() * 1000)}`

test.use({
    permissions: ['microphone'],
    launchOptions: { args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream'] },
})

/** The idle control: a button, only when the agent can actually hold a session (FR-004). */
const offered = (page: Page) => page.getByRole('button', { name: 'Start live voice' })
/** The live row: one `role="group"` inside the composer's meta line (B1-1, FR-043). */
const row = (page: Page) => page.getByRole('group', { name: 'Live voice' })
const end = (page: Page) => page.getByRole('button', { name: 'End the live voice session' })
const stateWord = (page: Page) => page.locator('[role="status"][data-state]')

/** Fails loudly when the server offers no live voice, instead of quietly testing nothing. */
async function expectOffered(page: Page) {
    await expect(
        offered(page),
        'the agent must come back with live_voice: set a recogniser and a voice in its config'
    ).toBeVisible({ timeout: 10_000 })
}

test.beforeEach(async ({ page }) => {
    await login(page)
    await createPlugAgent(page, AGENT)
})

test('T091 — FR-046: three turns survive opening and ending the row, word for word', async ({ page }) => {
    await page.goto(`/${AGENT}/chat/${topic()}`)
    for (const text of ['one', 'two', 'three']) {
        await send(page, text)
        // The turn is over when Send is back.
        await expect(page.getByRole('button', { name: 'Send' })).toBeVisible({ timeout: 20_000 })
    }

    const before = await page.getByRole('listitem').evaluateAll((items) =>
        items.map((item) => ({ text: item.textContent ?? '', box: item.getBoundingClientRect().top }))
    )
    expect(before.length, 'three turns, with their replies').toBeGreaterThan(0)

    await expectOffered(page)
    await offered(page).click()
    await expect(row(page)).toBeVisible({ timeout: 10_000 })
    await end(page).click()
    await expect(row(page)).toBeHidden()

    const after = await page.getByRole('listitem').evaluateAll((items) =>
        items.map((item) => ({ text: item.textContent ?? '', box: item.getBoundingClientRect().top }))
    )
    // Same words, same order, same clock: the session leaves nothing behind in the conversation.
    expect(after).toEqual(before)

    // And the same three turns in a conversation that never had a session read the same. The clock
    // is compared inside one conversation only, because two conversations cannot share a minute.
    await page.goto(`/${AGENT}/chat/${topic()}`)
    for (const text of ['one', 'two', 'three']) {
        await send(page, text)
        await expect(page.getByRole('button', { name: 'Send' })).toBeVisible({ timeout: 20_000 })
    }
    const plain = await page.getByRole('listitem').evaluateAll((items) => items.map((item) => item.textContent ?? ''))
    expect(plain).toHaveLength(before.length)
    const stripClock = (text: string) => text.replace(/\b\d{1,2}:\d{2}\b/g, '')
    expect(plain.map(stripClock)).toEqual(before.map((item) => stripClock(item.text)))
})

test('T092 — SC-010: the first press shows an interactive row within 500 ms', async ({ page }) => {
    await page.goto(`/${AGENT}/chat/${topic()}`)
    await expectOffered(page)

    const started = await page.evaluate(() => performance.now())
    await offered(page).click()
    // Interactive, not merely present: the End control of a live row is what proves the session
    // opened rather than the row failing into `ended` or `failed`.
    await expect(end(page)).toBeVisible({ timeout: 10_000 })
    const elapsed = await page.evaluate((from) => performance.now() - from, started)

    expect(elapsed, `the row took ${Math.round(elapsed)} ms to become interactive`).toBeLessThan(500)
    await expect(stateWord(page)).toHaveAttribute('data-state', /listening|connecting|waiting/)
    await end(page).click()
})

test('T094 — FR-049: the row covers no message, adds no height, and surrenders in order', async ({ page }) => {
    await page.goto(`/${AGENT}/chat/${topic()}`)
    for (const text of ['one', 'two', 'three']) {
        await send(page, text)
        await expect(page.getByRole('button', { name: 'Send' })).toBeVisible({ timeout: 20_000 })
    }

    // The meta line the row borrows: the flex row the Send button sits in.
    const metaLine = page.getByRole('button', { name: 'Send' }).locator('..')
    const heightBefore = (await metaLine.boundingBox())?.height ?? 0

    await expectOffered(page)
    await offered(page).click()
    await expect(end(page)).toBeVisible({ timeout: 10_000 })

    // The composer keeps its height: the row borrows the meta line, it does not add one (B1-1).
    const heightAfter = (await metaLine.boundingBox())?.height ?? 0
    expect(Math.abs(heightAfter - heightBefore), 'the meta line does not grow').toBeLessThanOrEqual(1)

    // Nothing is covered: the row sits below every message, and the end of the conversation is
    // reachable with the row shown.
    const rowBox = await row(page).boundingBox()
    const lastMessage = await page.getByRole('listitem').last().boundingBox()
    if (rowBox && lastMessage) expect(lastMessage.y + lastMessage.height).toBeLessThanOrEqual(rowBox.y + 1)
    await page.mouse.wheel(0, 5000)
    await expect(page.getByRole('listitem').last()).toBeInViewport()

    // The order of surrender as the pane narrows: the waveform gives way first, then disappears;
    // the microphone indicator, the state word, the time and End stay readable and unwrapped.
    const widths = [1440, 1100, 900]
    let previousBars = Number.POSITIVE_INFINITY
    for (const width of widths) {
        await page.setViewportSize({ width, height: 900 })
        const word = stateWord(page)
        await expect(word, `the state word is still there at ${width} px`).toBeVisible()
        // No ellipsis in the state word: the text fits, it is not truncated.
        const clipped = await word.evaluate((el) => el.scrollWidth > el.clientWidth + 1)
        expect(clipped, `the state word is not clipped at ${width} px`).toBe(false)
        await expect(page.getByText(/^mic (on|off)$/)).toBeVisible()
        await expect(end(page)).toBeVisible()

        const bars = page.locator('[data-bars]')
        if (await bars.count()) {
            const count = Number(await bars.getAttribute('data-bars'))
            expect(count, 'the waveform never grows as the pane narrows').toBeLessThanOrEqual(previousBars)
            previousBars = count
        }
    }
    // The 240 px floor is the row's own floor, not the window's: the window stops at 900 px, so the
    // last word on the floor belongs to a container-level test, not to this one.
    await end(page).click()
})
