import { createPlugAgent, expect, login, test } from './fixtures'

const AGENT = 'e2e-plug'
const unique = (p: string) => `${p}-${Date.now().toString(36)}`

test.describe('Day-to-day pages (quickstart §5, §7)', () => {
    test.beforeEach(async ({ page }) => {
        await login(page)
        await createPlugAgent(page, AGENT)
    })

    test('Core: an edit saves as a new revision and survives a reload', async ({ page }) => {
        await page.goto(`/${AGENT}/core`)
        const editor = page.getByRole('textbox', { name: 'Document' })
        await expect(editor).toBeVisible()
        const stamp = unique('note')
        await editor.click()
        await page.keyboard.press('Control+End')
        await page.keyboard.type(`\n\nEdited ${stamp}`)
        await expect(page.getByText(/Unsaved/)).toBeVisible()
        await page.getByRole('button', { name: /^Save/ }).click()
        await expect(page.getByText('CORE saved')).toBeVisible()
        await page.reload()
        await expect(page.getByText(`Edited ${stamp}`)).toBeVisible()
        await expect(page.getByText(/last written by/)).toBeVisible()
    })

    test('Core: saving over a newer revision offers Reload theirs / Keep mine', async ({ page, context }) => {
        await page.goto(`/${AGENT}/core`)
        await expect(page.getByRole('textbox', { name: 'Document' })).toBeVisible()
        const other = await context.newPage()
        await other.goto(`/${AGENT}/core`)
        const otherEditor = other.getByRole('textbox', { name: 'Document' })
        await otherEditor.click()
        await other.keyboard.press('Control+End')
        await other.keyboard.type(`\n\ntheirs ${unique('t')}`)
        await other.getByRole('button', { name: /^Save/ }).click()
        await expect(other.getByText('CORE saved')).toBeVisible()

        const editor = page.getByRole('textbox', { name: 'Document' })
        await editor.click()
        await page.keyboard.press('Control+End')
        await page.keyboard.type(`\n\nmine ${unique('m')}`)
        await page.getByRole('button', { name: /^Save/ }).click()
        await expect(page.getByText('CORE changed while you were editing')).toBeVisible()
        await page.getByRole('button', { name: 'Reload theirs' }).click()
        await expect(page.getByText(/theirs t-/).last()).toBeVisible()
    })

    test('Skills: create, edit in the editor, and see the draft come back after a reload', async ({ page }) => {
        const name = unique('e2e-skill')
        await page.goto(`/${AGENT}/skills/new/edit?scope=agent`)
        await page.getByLabel('Name').fill(name)
        await page.getByLabel('Description').fill('A skill made by the e2e suite')
        await page.getByRole('button', { name: /^Save/ }).click()
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/skills\\?skill=${name}`))
        await expect(page.getByText(`/${name}`).first()).toBeVisible()

        await page.goto(`/${AGENT}/skills/${name}/edit?scope=agent`)
        const editor = page.getByRole('textbox', { name: 'Document' })
        await editor.click()
        await page.keyboard.press('Control+End')
        await page.keyboard.type('\n\nDraft text that was never saved')
        await expect(page.getByText(/Unsaved/)).toBeVisible()
        await page.reload()
        await expect(page.getByText(/unsaved draft/i)).toBeVisible()
        await page.getByRole('button', { name: 'Restore draft' }).click()
        await expect(page.getByText(/Draft text that was never saved/).first()).toBeVisible()
    })

    test('Tasks: a one-time task is created with the date field and listed', async ({ page }) => {
        const title = unique('E2E reminder')
        await page.goto(`/${AGENT}/tasks`)
        await page.getByRole('button', { name: 'New task' }).first().click()
        await page.getByLabel('Title').fill(title)
        await page.getByLabel('Instruction').fill('Say hello at the appointed time.')
        await page.getByRole('button', { name: 'Once' }).click()
        await page.getByLabel('Run at').click()
        // The grid opens on this month; pick a day a month on.
        await page.getByRole('button', { name: 'Next month' }).click()
        await page.getByRole('button', { name: /15, 20\d\d$/ }).first().click()
        await page.getByRole('button', { name: 'Create task' }).click()
        await expect(page.getByText('Task saved')).toBeVisible()
        await expect(page.getByText(title)).toBeVisible()
        const row = page.getByRole('row', { name: new RegExp(title) })
        await row.getByRole('switch').click()
        await expect(page.getByRole('row', { name: new RegExp(title) }).getByRole('switch')).toHaveAttribute('aria-checked', 'false')
    })

    test('Dreams: the night strip, the journal and Dream now are there', async ({ page }) => {
        await page.goto(`/${AGENT}/dream`)
        await expect(page.getByRole('heading', { name: 'Dreams' })).toBeVisible()
        await expect(page.getByText(/Last 30 nights/)).toBeVisible()
        await expect(page.getByRole('button', { name: /Dream now/ })).toBeVisible()
    })

    test('Memory: a document opens from its bundle, with its links', async ({ page }) => {
        await page.goto(`/${AGENT}/memory`)
        await expect(page.getByRole('heading', { name: 'Memory' })).toBeVisible()
        await page.getByRole('button', { name: 'New memory' }).click()
        const bundle = unique('e2e')
        const dialog = page.getByRole('dialog')
        await dialog.getByLabel('Bundle').fill(bundle)
        await dialog.getByLabel('Title').fill('First note')
        await dialog.getByLabel('Content').fill('Hello from the e2e suite.')
        await dialog.getByRole('button', { name: 'Create' }).click()
        await expect(page).toHaveURL(new RegExp(`/memory/${bundle}\\?doc=`))
        await expect(page.getByText('Hello from the e2e suite.')).toBeVisible()
    })

    test('Usage: the report loads', async ({ page }) => {
        await page.goto(`/${AGENT}/usage`)
        await expect(page.getByRole('heading', { name: 'Usage' })).toBeVisible()
        await expect(page.getByText('Estimated cost')).toBeVisible()
    })
})
