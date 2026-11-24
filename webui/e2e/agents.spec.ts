import { expect, login, test } from './fixtures'

const unique = (p: string) => `${p}-${Date.now().toString(36)}`

async function token(page: import('@playwright/test').Page): Promise<string> {
    return page.evaluate(() => localStorage.getItem('auth_token') ?? '')
}

async function agentExists(page: import('@playwright/test').Page, id: string): Promise<boolean> {
    const res = await page.request.get(`/api/v1/agents/${id}`, { headers: { Authorization: `Bearer ${await token(page)}` } })
    return res.ok()
}

test.describe('Agents (quickstart §7 steps 4–6)', () => {
    test.beforeEach(async ({ page }) => {
        await login(page)
    })

    test('New agent: five steps in a modal, nothing saved until Create, then Config rebuild warning and typed-ID delete', async ({ page }) => {
        const name = unique('wizard')
        await page.goto('/agents/new')
        const dialog = page.getByRole('dialog')

        // Identity
        await dialog.getByLabel('Name', { exact: true }).fill(name)
        await expect(dialog.getByLabel('Agent ID')).toHaveValue(name)
        await dialog.getByRole('button', { name: 'Continue' }).click()

        // Models
        await dialog.getByRole('button', { name: /Provider/ }).click()
        await page.getByRole('option', { name: /dummyplug/i }).click()
        await expect(dialog.getByRole('button', { name: /^Model/ })).toBeVisible()
        await dialog.getByRole('button', { name: /^Model/ }).click()
        await page.getByRole('option').first().click()
        await dialog.getByRole('button', { name: 'Continue' }).click()

        // Addons: skippable
        await dialog.getByRole('button', { name: 'Skip' }).click()

        // Prompt: skippable, but this one is filled
        await dialog.getByLabel('System prompt').fill('You are a test agent.')
        await dialog.getByRole('button', { name: 'Continue' }).click()

        // Review: the preview, and still nothing saved
        await expect(dialog.getByText(/nothing is saved until you create it/i)).toBeVisible()
        expect(await agentExists(page, name)).toBe(false)

        await dialog.getByRole('button', { name: 'Create agent' }).click()
        // The dialog closes at once: the agent's tile in the rail is busy until the server has it.
        await expect(dialog).toBeHidden()
        await expect(page.getByText(`${name} created`)).toBeVisible()
        await page.getByRole('navigation', { name: 'Agents' }).getByRole('link', { name }).click()
        await expect(page).toHaveURL(new RegExp(`/${name}/chat`))
        expect(await agentExists(page, name)).toBe(true)

        // Config: a change asks first, because saving rebuilds the agent
        await page.goto(`/${name}/config`)
        await page.getByLabel('Description').fill('changed by the e2e suite')
        await page.getByRole('button', { name: 'Save changes' }).click()
        await expect(page.getByRole('dialog').getByText(/Saving rebuilds .* and stops its running turns/)).toBeVisible()
        await page.getByRole('dialog').getByRole('button', { name: 'Save' }).click()
        await expect(page.getByText('Saved')).toBeVisible()

        // Danger zone: the delete needs the agent's ID typed. A save rebuilds the agent and the
        // server refuses a delete until that finishes, so the whole attempt is retried.
        await expect(async () => {
            await page.goto(`/${name}/config?tab=danger`)
            await page.getByRole('button', { name: 'Delete agent' }).click()
            const confirm = page.getByRole('dialog').getByRole('button', { name: `Delete ${name}` })
            await expect(confirm).toBeDisabled()
            await page.getByRole('dialog').getByRole('textbox').fill(name)
            await expect(confirm).toBeEnabled()
            await confirm.click()
            await expect(page).toHaveURL(/\/$/, { timeout: 3000 })
        }).toPass({ timeout: 25000 })
        expect(await agentExists(page, name)).toBe(false)
    })

    test('Integrations: add an MCP server, switch a grant on, and see it in Activity', async ({ page }) => {
        const agent = unique('integ')
        const res = await page.request.post('/api/v1/agents', {
            headers: { Authorization: `Bearer ${await token(page)}` },
            data: { agent_id: agent, name: agent, provider: 'dummyplug', model: 'dummyplug' },
        })
        expect(res.ok()).toBeTruthy()

        await page.goto(`/${agent}/integrations/mcp`)
        await page.getByRole('button', { name: 'Add server' }).first().click()
        const dialog = page
        await dialog.getByLabel('Name').fill('filesystem')
        await dialog.getByLabel('Command').fill('npx')
        await dialog.getByLabel('Arguments').fill('-y\n@modelcontextprotocol/server-filesystem\n/tmp')
        await page.getByRole('button', { name: 'Add server' }).last().click()
        await expect(page.getByRole('row', { name: /filesystem/ })).toBeVisible()

        await page.getByRole('switch', { name: /^MCP servers/ }).click()
        await expect(page.getByRole('switch', { name: /^MCP servers/ })).toBeChecked()
        await expect(page.getByText(/added filesystem/)).toBeVisible()
        await expect(page.getByText(/let the agent manage its own mcp integrations/)).toBeVisible()

        await page.request.delete(`/api/v1/agents/${agent}`, { headers: { Authorization: `Bearer ${await token(page)}` } })
    })
})
