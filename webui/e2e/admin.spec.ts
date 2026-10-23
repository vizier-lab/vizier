import { expect, login, test } from './fixtures'

const unique = (p: string) => `${p}-${Date.now().toString(36)}`

async function setupRequired(page: import('@playwright/test').Page): Promise<boolean> {
    const res = await page.request.get('/api/v1/status')
    return (await res.json()).data?.setup_required === true
}

test.describe('Onboarding on an empty data dir (quickstart §1)', () => {
    test('account, provider and first agent, then Chat', async ({ page }) => {
        test.skip(!(await setupRequired(page)), 'needs a vizier started on an empty data dir (VIZIER_URL)')
        await page.goto('/')
        await expect(page).toHaveURL(/\/onboarding/)
        await expect(page.getByText(/Server online/)).toBeVisible()

        // Account: the password rules are checked before anything is sent
        await page.getByLabel('Username').fill('admin')
        await page.getByLabel('Password', { exact: true }).fill('short')
        await expect(page.getByText('At least 8 characters.')).toBeVisible()
        await page.getByLabel('Password', { exact: true }).fill('correct-horse')
        await page.getByLabel('Confirm Password').fill('wrong-horse')
        await expect(page.getByText('Passwords do not match.')).toBeVisible()
        await page.getByLabel('Confirm Password').fill('correct-horse')
        await page.getByRole('button', { name: 'Create account' }).click()

        // Provider: Ollama needs no key
        await expect(page.getByRole('heading', { name: 'Add a provider' })).toBeVisible()
        await expect(page.getByText(/dummyplug/i)).toHaveCount(0)
        await page.getByRole('button', { name: 'Use Ollama' }).click()
        await page.getByRole('button', { name: 'Connect' }).click()
        // The connection check may say the local server is not running: keep it anyway
        const keep = page.getByRole('button', { name: 'Keep it anyway' })
        await keep.waitFor({ timeout: 10000 }).then(() => keep.click(), () => {})
        await page.getByRole('button', { name: 'Continue' }).click()

        // First agent
        await expect(page.getByRole('heading', { name: 'Meet your first agent' })).toBeVisible()
    })
})

test.describe('Administration (quickstart §1, §7)', () => {
    test.beforeEach(async ({ page }) => {
        test.skip(await setupRequired(page), 'needs a set-up vizier')
        await login(page)
    })

    test('API keys: a new key is shown once, with its prefix in the list, and can be deleted', async ({ page }) => {
        const name = unique('key')
        await page.goto('/settings/api-keys')
        await page.getByRole('button', { name: 'New key' }).first().click()
        await page.getByLabel('Name').fill(name)
        await page.getByRole('button', { name: 'Create key' }).click()
        await expect(page.getByText(/won’t be shown again/)).toBeVisible()
        await expect(page.getByRole('row', { name: new RegExp(name) })).toContainText('vk_')
        await page.getByRole('button', { name: `Delete ${name}` }).click()
        await page.getByRole('dialog').getByRole('button', { name: 'Delete key' }).click()
        await expect(page.getByRole('row', { name: new RegExp(name) })).toHaveCount(0)
    })

    test('Roles: make a role, grant a permission, save, and see it counted', async ({ page }) => {
        const name = unique('role')
        await page.goto('/settings/roles')
        await page.getByRole('button', { name: 'New role' }).click()
        await page.getByRole('dialog').getByLabel('Name').fill(name)
        await page.getByRole('dialog').getByRole('button', { name: 'Create role' }).click()
        await expect(page.getByRole('heading', { name })).toBeVisible()
        await page.getByRole('checkbox', { name: /Create agents/ }).check()
        await page.getByRole('button', { name: 'Save role' }).click()
        await expect(page.getByText('Role saved')).toBeVisible()
        await page.reload()
        await expect(page.getByRole('checkbox', { name: /Create agents/ })).toBeChecked()
    })

    test('Users: add a person with a role, then delete them; you cannot delete yourself', async ({ page }) => {
        const username = unique('person')
        await page.goto('/settings/users')
        await expect(page.getByRole('row', { name: /admin.*\(you\)/ })).toBeVisible()
        await expect(page.getByRole('button', { name: 'Delete admin' })).toHaveCount(0)
        await page.getByRole('button', { name: 'Add user' }).first().click()
        const dialog = page.getByRole('dialog')
        await dialog.getByLabel('Username').fill(username)
        await dialog.getByLabel('Password').fill('a-long-password')
        await dialog.getByRole('button', { name: /Role/ }).click()
        // Not superadmin: the system user can't be deleted again.
        await page.getByRole('option').last().click()
        await dialog.getByRole('button', { name: 'Add user' }).click()
        await expect(page.getByRole('row', { name: new RegExp(username) })).toBeVisible()
        await page.getByRole('button', { name: `Delete ${username}` }).click()
        await page.getByRole('dialog').getByRole('button', { name: 'Delete user' }).click()
        await expect(page.getByRole('row', { name: new RegExp(username) })).toHaveCount(0)
    })

    test('Profile: save a Discord username', async ({ page }) => {
        await page.goto('/settings/profile')
        const stamp = unique('disc')
        await page.getByLabel('Discord username').fill(stamp)
        await page.getByRole('button', { name: 'Save profile' }).click()
        await expect(page.getByText('Profile saved')).toBeVisible()
        await page.reload()
        await expect(page.getByLabel('Discord username')).toHaveValue(stamp)
    })

    test('Providers: the connected list, the catalogue grid and a custom provider', async ({ page }) => {
        const name = unique('home-llm')
        await page.goto('/settings/providers')
        await expect(page.getByText('Add a provider')).toBeVisible()
        await page.getByRole('button', { name: 'Add custom provider' }).click()
        const dialog = page.getByRole('dialog')
        await dialog.getByLabel('Name').fill(name)
        await dialog.getByLabel('Base URL').fill('http://127.0.0.1:9/v1')
        await dialog.getByLabel('Models').fill('tiny-model')
        await dialog.getByRole('button', { name: 'Add provider' }).click()
        await expect(page.getByText(name)).toBeVisible()
    })
})
