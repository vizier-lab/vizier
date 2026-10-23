import { createPlugAgent, expect, HOLD, login, send, test } from './fixtures'

const AGENT = 'e2e-plug'
const topic = () => `e2e-${Date.now()}-${Math.floor(Math.random() * 1000)}`

test.describe('Chat (quickstart §2)', () => {
    test.beforeEach(async ({ page }) => {
        await login(page)
        await createPlugAgent(page, AGENT)
    })

    test('a reply lands with its footer; Send turns into Stop while it works', async ({ page }) => {
        await page.goto(`/${AGENT}/chat/${topic()}`)
        await send(page, 'tools')
        await expect(page.getByText('Available tools')).toBeVisible()
        await expect(page.getByText(/dummyplug\/dummyplug/)).toBeVisible()
        await expect(page.getByRole('button', { name: 'Send' })).toBeVisible()
    })

    test('the live trail shows a running call, then folds into a summary', async ({ page }) => {
        await page.goto(`/${AGENT}/chat/${topic()}`)
        await send(page, HOLD(4))
        await expect(page.getByRole('button', { name: 'Stop' })).toBeVisible()
        await expect(page.getByText('Working…')).toBeVisible()
        await expect(page.getByText('running…')).toBeVisible()
        await expect(page.getByText(/^Worked/)).toBeVisible({ timeout: 15_000 })
        await expect(page.getByText(/1 python run/)).toBeVisible()
    })

    test('a message sent while busy is queued, then sent when the turn ends', async ({ page }) => {
        await page.goto(`/${AGENT}/chat/${topic()}`)
        await send(page, HOLD(4))
        await expect(page.getByRole('button', { name: 'Stop' })).toBeVisible()
        await send(page, 'second')
        await expect(page.getByText(/Queued/)).toBeVisible()
        // It goes out once the first turn ends.
        await expect(page.getByText(/Queued/)).toBeHidden({ timeout: 20_000 })
        await expect(page.getByText('second').first()).toBeVisible()
    })

    test('Stop ends the turn and drops the queue', async ({ page }) => {
        await page.goto(`/${AGENT}/chat/${topic()}`)
        await send(page, HOLD(30))
        await expect(page.getByRole('button', { name: 'Stop' })).toBeVisible()
        await send(page, 'never sent')
        await page.getByRole('button', { name: 'Stop' }).click()
        await expect(page.getByText(/Stopped by you/)).toBeVisible({ timeout: 10_000 })
        await expect(page.getByText(/Queued/)).toBeHidden()
    })

    test('reloading mid-turn shows the turn so far and the rest of it', async ({ page }) => {
        const url = `/${AGENT}/chat/${topic()}`
        await page.goto(url)
        await send(page, HOLD(8))
        await expect(page.getByText('running…')).toBeVisible()
        await page.reload()
        await expect(page.getByText(/Working…/)).toBeVisible({ timeout: 5_000 })
        await expect(page.getByText(/^Worked/)).toBeVisible({ timeout: 20_000 })
        // One turn, one trail: no step is drawn twice.
        await expect(page.getByText(/^Worked/)).toHaveCount(1)
        await expect(page.getByText(/1 python run/)).toHaveCount(1)
    })

    test('typing / opens the skill menu with built-in commands', async ({ page }) => {
        await page.goto(`/${AGENT}/chat/${topic()}`)
        await page.getByRole('textbox').first().fill('/')
        await expect(page.getByText('checkpoint')).toBeVisible()
        await expect(page.getByText('lobotomy')).toBeVisible()
    })
})
