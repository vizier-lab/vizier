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
        await page.getByRole('button', { name: 'Edit' }).click()
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/core/edit$`))
        // The editor opens split: the source is typed into, the preview follows.
        const editor = page.getByRole('textbox', { name: 'Markdown source' })
        await expect(editor).toBeVisible()
        const stamp = unique('note')
        await editor.click()
        await page.keyboard.press('Control+End')
        await page.keyboard.type(`\n\nEdited ${stamp}`)
        await expect(page.getByText(/Unsaved/)).toBeVisible()
        await page.getByRole('button', { name: /^Save/ }).click()
        await expect(page.getByText('CORE saved')).toBeVisible()
        // Saving goes back to the preview, which shows the new text.
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/core$`))
        await page.reload()
        await expect(page.getByText(`Edited ${stamp}`)).toBeVisible()
        await expect(page.getByText(/last written by/)).toBeVisible()
    })

    test('Core: saving over a newer revision offers Reload theirs / Keep mine', async ({ page, context }) => {
        await page.goto(`/${AGENT}/core/edit`)
        await expect(page.getByRole('textbox', { name: 'Markdown source' })).toBeVisible()
        const other = await context.newPage()
        await other.goto(`/${AGENT}/core/edit`)
        const otherEditor = other.getByRole('textbox', { name: 'Markdown source' })
        await otherEditor.click()
        await other.keyboard.press('Control+End')
        await other.keyboard.type(`\n\ntheirs ${unique('t')}`)
        await other.getByRole('button', { name: /^Save/ }).click()
        await expect(other.getByText('CORE saved')).toBeVisible()

        const editor = page.getByRole('textbox', { name: 'Markdown source' })
        await editor.click()
        await page.keyboard.press('Control+End')
        await page.keyboard.type(`\n\nmine ${unique('m')}`)
        await page.getByRole('button', { name: /^Save/ }).click()
        await expect(page.getByText('CORE changed while you were editing')).toBeVisible()
        await page.getByRole('button', { name: 'Reload theirs' }).click()
        await expect(page.getByText(/theirs t-/).last()).toBeVisible()
        await other.close()
    })

    test('Skills: create, edit in the editor, and see the draft come back after a reload', async ({ page }) => {
        const name = unique('e2e-skill')
        await page.goto(`/${AGENT}/skills/new/edit?scope=agent`)
        await page.getByLabel('Name').fill(name)
        await page.getByLabel('Description').fill('A skill made by the e2e suite')
        await page.getByRole('button', { name: /^Save/ }).click()
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/skills/${name}\\?scope=agent`))
        await expect(page.getByText(`/${name}`).first()).toBeVisible()

        await page.goto(`/${AGENT}/skills/${name}/edit?scope=agent`)
        const editor = page.getByRole('textbox', { name: 'Markdown source' })
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
        await expect(page).toHaveURL(new RegExp(`/${AGENT}/tasks/new/edit$`))
        await page.getByLabel('Title').fill(title)
        await page.getByRole('textbox', { name: 'Markdown source' }).fill('Say hello at the appointed time.')
        await page.getByRole('button', { name: 'Once' }).click()
        await page.getByLabel('Run at').click()
        // The grid opens on this month; pick a day a month on.
        await page.getByRole('button', { name: 'Next month' }).click()
        await page.getByRole('button', { name: /15, 20\d\d$/ }).first().click()
        await page.getByRole('button', { name: 'Create task' }).click()
        await expect(page.getByText('Task created')).toBeVisible()
        // It opens on its own page; the list has it too.
        await expect(page.getByRole('heading', { name: title })).toBeVisible()
        await page.getByRole('link', { name: 'Back to Schedule' }).click()
        // The page opens on the calendar and on its Active tab; the table has the task, and All keeps it listed once it is paused.
        await page.getByRole('button', { name: 'Table' }).click()
        await page.getByRole('button', { name: /^All/ }).click()
        const row = page.getByRole('row', { name: new RegExp(title) })
        await row.getByRole('switch').click()
        await expect(page.getByRole('row', { name: new RegExp(title) }).getByRole('switch')).toHaveAttribute('aria-checked', 'false')
    })

    test('Schedule: the calendar opens a month, a day, and the table over the same tasks', async ({ page }) => {
        const title = unique('E2E daily')
        await page.goto(`/${AGENT}/tasks/new/edit`)
        await page.getByLabel('Title').fill(title)
        await page.getByRole('textbox', { name: 'Markdown source' }).fill('Say hello every day.')
        await page.getByRole('button', { name: 'Create task' }).click()
        await expect(page.getByText('Task created')).toBeVisible()
        await page.goto(`/${AGENT}/tasks`)
        await expect(page.getByRole('heading', { name: 'Schedule', exact: true })).toBeVisible()
        // Narrow to this task so earlier runs' tasks don't crowd it out of a cell.
        await page.getByRole('searchbox', { name: 'Search tasks' }).fill(title)
        // Calendar is the default view, on the month, and Active is the default tab.
        await expect(page.getByRole('group', { name: /^[A-Z][a-z]+ 20\d\d$/ })).toBeVisible()
        await expect(page.getByRole('button', { name: /^Active/ })).toHaveAttribute('aria-pressed', 'true')
        await expect(page.getByRole('button', { name: new RegExp(title) }).first()).toBeVisible()
        // A day opens the day view; the task is on its timeline or in its repeat card.
        await page.getByRole('button', { name: 'Day', exact: true }).click()
        await expect(page).toHaveURL(/range=day/)
        await expect(page.getByRole('button', { name: 'Previous day' })).toBeVisible()
        // The table is the same list.
        await page.getByRole('button', { name: 'Table' }).click()
        await expect(page).toHaveURL(/view=table/)
        await expect(page.getByRole('row', { name: new RegExp(title) })).toBeVisible()
    })

    test('Schedule: back from a task returns to the view you left (day, filter, search)', async ({ page }) => {
        const title = unique('E2E return')
        await page.goto(`/${AGENT}/tasks/new/edit`)
        await page.getByLabel('Title').fill(title)
        await page.getByRole('textbox', { name: 'Markdown source' }).fill('Say hello.')
        await page.getByRole('button', { name: 'Create task' }).click()
        await expect(page.getByText('Task created')).toBeVisible()
        const t = new Date(Date.now() + 86_400_000)
        const day = `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, '0')}-${String(t.getDate()).padStart(2, '0')}`
        const here = `/${AGENT}/tasks?range=day&date=${day}&filter=all&q=${encodeURIComponent(title)}`
        await page.goto(here)
        const open = page.getByRole('button', { name: new RegExp(title) }).or(page.getByRole('link', { name: new RegExp(title) })).first()
        await open.click()
        await expect(page.getByRole('heading', { name: title })).toBeVisible()
        // Editing and cancelling keeps the way back too.
        await page.getByRole('button', { name: 'Edit' }).click()
        await page.getByRole('button', { name: 'Cancel' }).click()
        await page.getByRole('link', { name: 'Back to Schedule' }).click()
        await expect(page).toHaveURL(new RegExp(`range=day&date=${day}&filter=all&q=`.replace(/[?]/g, '\\?')))
        await expect(page.getByRole('button', { name: 'Next day' })).toBeVisible()
    })

    test('Dreams: the night strip, the journal and Dream now are there', async ({ page }) => {
        await page.goto(`/${AGENT}/dream`)
        await expect(page.getByRole('heading', { name: 'Dreams' })).toBeVisible()
        await expect(page.getByText(/Last 30 nights/)).toBeVisible()
        await expect(page.getByRole('button', { name: /Dream now/ })).toBeVisible()
    })

    test('Memory: a new memory is written in its editor and found in the explorer', async ({ page }) => {
        await page.goto(`/${AGENT}/memory`)
        await expect(page.getByRole('heading', { name: 'Memory' })).toBeVisible()
        await page.getByRole('button', { name: 'New memory' }).first().click()
        const bundle = unique('e2e')
        const dialog = page.getByRole('dialog')
        await dialog.getByLabel('Bundle').fill(bundle)
        await dialog.getByLabel('Title').fill('First note')
        await dialog.getByRole('button', { name: 'Create and edit' }).click()
        await expect(page).toHaveURL(new RegExp(`/memory/${bundle}/first-note\\?edit=1$`))
        await page.getByRole('textbox', { name: 'Markdown source' }).click()
        await page.keyboard.press('Control+End')
        await page.keyboard.type('Hello from the e2e suite.')
        await page.getByRole('button', { name: /^Save/ }).click()
        await expect(page).toHaveURL(new RegExp(`/memory/${bundle}/first-note$`))
        await expect(page.getByRole('article').getByText('Hello from the e2e suite.')).toBeVisible()

        // Back in its bundle, the explorer lists it and opens it again.
        await page.getByRole('link', { name: `Back to ${bundle}` }).click()
        await page.getByRole('treeitem', { name: /First note/ }).click()
        await expect(page).toHaveURL(new RegExp(`/memory/${bundle}/first-note$`))
    })

    test('Usage: the report loads', async ({ page }) => {
        await page.goto(`/${AGENT}/usage`)
        await expect(page.getByRole('heading', { name: 'Usage' })).toBeVisible()
        await expect(page.getByText('Estimated cost')).toBeVisible()
    })
})
