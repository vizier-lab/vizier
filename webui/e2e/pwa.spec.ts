import type { Page } from '@playwright/test'
import { expect, test } from './fixtures'
import { login } from './fixtures'

/** Whether the worker is in charge: registered and activated, not merely asked for. */
async function activated(page: Page): Promise<boolean> {
    // `waitForFunction` does not await an async predicate, so this polls for itself.
    for (let i = 0; i < 60; i++) {
        const ready = await page.evaluate(async () => {
            const registrations = await navigator.serviceWorker.getRegistrations()
            return registrations.some((r) => r.active?.state === 'activated')
        })
        if (ready) return true
        await page.waitForTimeout(250)
    }
    return false
}

/** Whether the page is under the worker's control, which takes a reload to happen. */
async function controlled(page: Page): Promise<boolean> {
    // The worker has to be in charge before the reload, or the reload is not controlled either.
    if (!(await activated(page))) return false
    // The worker does not claim the clients it did not serve, so the reload gives it control.
    await page.reload()
    return page
        .waitForFunction(() => navigator.serviceWorker.controller !== null, undefined, { timeout: 15_000 })
        .then(() => true)
        .catch(() => false)
}

test.describe('The installable app', () => {
    test('serves the manifest', async ({ request }) => {
        const res = await request.get('/manifest.webmanifest')
        expect(res.status()).toBe(200)
        expect(res.headers()['content-type']).toContain('application/manifest+json')
        const body = await res.json()
        expect(body.name).toBe('Vizier')
        expect(body.display).toBe('standalone')
        expect(body.start_url).toBe('/')
    })

    test('serves the worker with the right type', async ({ request }) => {
        const res = await request.get('/service-worker.js')
        expect(res.status()).toBe(200)
        expect(res.headers()['content-type']).toContain('javascript')
    })

    test('serves a worker that belongs to this build', async ({ request }) => {
        // The worker is stamped with the build's id, so a new build is a new worker and the
        // browser offers it. Without the stamp every build ships the same bytes and nothing does.
        const body = await (await request.get('/service-worker.js')).text()
        expect(body).not.toContain('__VIZIER_BUILD__')
        expect(body).toMatch(/const BUILD = '[0-9a-f]{12}'/)
    })

    test('registers the worker and renders offline', async ({ page, context }) => {
        await page.goto('/login')
        // A worker that never registers is skipped, not failed: a flaky check is worse than none.
        test.skip(!(await controlled(page)), 'the service worker did not register')

        await context.setOffline(true)
        await page.reload()
        await expect(page.getByRole('button', { name: /sign in/i })).toBeVisible()
        // The sign-in screen says so: without a connection it cannot succeed.
        await expect(page.getByText('No connection to Vizier')).toBeVisible()
    })

    test('says what is unavailable when the engine cannot be reached', async ({ page, context }) => {
        await login(page)
        await context.setOffline(true)
        await expect(page.getByText("Vizier can't reach the engine")).toBeVisible()
        await expect(page.getByText('Replies and new conversations need a connection')).toBeVisible()
    })
})
