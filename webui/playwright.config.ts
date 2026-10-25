import { defineConfig, devices } from '@playwright/test'

const baseURL = process.env.VIZIER_URL ?? 'http://localhost:9999'

export default defineConfig({
    testDir: 'e2e',
    use: { baseURL },
    projects: (['light', 'dark'] as const).flatMap((colorScheme) => [
        {
            name: `desktop-${colorScheme}`,
            use: { ...devices['Desktop Chrome'], viewport: { width: 1440, height: 900 }, colorScheme },
        },
        {
            name: `mobile-${colorScheme}`,
            // The functional specs drive the desktop layout; the phone projects check the phone one.
            testMatch: /(mobile|redirects|notifications)\.spec\.ts/,
            use: { ...devices['Desktop Chrome'], viewport: { width: 390, height: 844 }, colorScheme },
        },
    ]),
})
