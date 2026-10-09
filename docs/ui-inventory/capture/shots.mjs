// Captures every WebUI page/state against a seeded instance.
import { chromium } from 'playwright'
import fs from 'fs'

const ORIGIN = process.env.ORIGIN || 'http://localhost:9911'
const OUT = process.env.OUT || '../screenshots'
const only = process.argv[2] ? new RegExp(process.argv[2]) : null
const token = fs.readFileSync('token', 'utf8').trim()
const userId = JSON.parse(Buffer.from(token.split('.')[1], 'base64url')).sub
fs.mkdirSync(OUT, { recursive: true })

const browser = await chromium.launch()

async function ctx({ scheme = 'light', width = 1440, height = 900, auth = true, agent = 'aria' } = {}) {
  const c = await browser.newContext({ viewport: { width, height }, colorScheme: scheme, deviceScaleFactor: 1 })
  c.setDefaultTimeout(6000)
  await c.addInitScript(([t, u, a, auth]) => {
    if (auth && !sessionStorage.getItem('__seeded')) {
      localStorage.setItem('auth_token', t)
      if (a) localStorage.setItem(`vizier_last_agent_${u}`, a)
      sessionStorage.setItem('__seeded', '1')
    }
  }, [token, userId, agent, auth])
  return c
}

const shots = []
function def(name, fn) { shots.push([name, fn]) }

async function go(page, path, wait = 1200) {
  await page.goto(ORIGIN + path)
  await page.waitForLoadState('networkidle').catch(() => {})
  await page.waitForTimeout(wait)
}
const snap = (page, name, opts = {}) => page.screenshot({ path: `${OUT}/${name}.png`, ...opts })
async function nodePoints(page) {
  return page.evaluate(() => {
    const c = document.querySelector('canvas'); const g = c.getContext('2d')
    const w = c.width, h = c.height; const d = g.getImageData(0, 0, w, h).data
    const sx = c.clientWidth / w, sy = c.clientHeight / h
    const cl = []
    for (let y = 0; y < h; y += 2) for (let x = 0; x < w; x += 2) {
      const i = (y * w + x) * 4
      const r = d[i], gg = d[i + 1], b = d[i + 2], a = d[i + 3]
      if (a > 200 && gg > 140 && r < 90 && b > 90 && b < 190) {
        const px = x * sx, py = y * sy
        const f = cl.find((k) => Math.hypot(k.x - px, k.y - py) < 14)
        if (f) { f.n++; f.tx += px; f.ty += py; f.x = f.tx / f.n; f.y = f.ty / f.n }
        else cl.push({ x: px, y: py, tx: px, ty: py, n: 1 })
      }
    }
    return cl.sort((a, b) => b.n - a.n).map((k) => [k.x, k.y])
  })
}
const click = (page, sel) => page.locator(sel).first().click()

// ---------- Auth ----------
def('02-login', async () => {
  const c = await ctx({ auth: false }); const p = await c.newPage()
  await go(p, '/login'); await snap(p, '02-login'); await c.close()
})

// ---------- Home ----------
def('03-home', async () => {
  const c = await ctx(); const p = await c.newPage()
  await go(p, '/'); await snap(p, '03-home-quick-chat')
  await p.getByText('Switch agent').click(); await p.waitForTimeout(500)
  await snap(p, '04-home-agent-picker')
  // agent switcher dropdown in sidebar
  await click(p, '.agent-card'); await p.waitForTimeout(400)
  await snap(p, '05-sidebar-agent-dropdown')
  await c.close()
  const c2 = await ctx({ agent: null }); const p2 = await c2.newPage()
  await go(p2, '/'); await snap(p2, '04b-home-no-agent-selected'); await c2.close()
})

// ---------- Create agent wizard ----------
def('06-agent-new', async () => {
  const c = await ctx({ height: 1400 }); const p = await c.newPage()
  await go(p, '/agents/new')
  await snap(p, '06-agent-new-1-config')
  await p.getByPlaceholder('my-agent').fill('researcher')
  await p.getByPlaceholder('My Agent').fill('Researcher')
  await p.getByPlaceholder('A helpful assistant').fill('Finds and summarises papers')
  for (const [i, tab] of [[2, 'Tools'], [3, 'System Prompt'], [4, 'Review']]) {
    await p.getByRole('button', { name: 'Next' }).last().click()
    await p.waitForTimeout(600)
    await snap(p, `06-agent-new-${i}-${tab.toLowerCase().replace(/ /g, '-')}`)
  }
  await c.close()
})

// ---------- Chat ----------
def('07-chat', async () => {
  const c = await ctx(); const p = await c.newPage()
  await go(p, '/aria/chat/General', 2500)
  await snap(p, '07-chat-conversation')
  // expand an activity trail if any
  const sum = p.locator('.activity-trail-summary')
  if (await sum.count()) { await sum.last().click(); await p.waitForTimeout(400); await snap(p, '08-chat-activity-trail-expanded') }
  // topic dropdown
  await click(p, '.session-selector'); await p.waitForTimeout(600)
  await snap(p, '09-chat-topic-dropdown')
  await p.getByText('Create New Topic').click(); await p.waitForTimeout(400)
  await p.locator('#new-session-id').fill('Release Notes Q4')
  await snap(p, '10-chat-new-topic-modal')
  await p.keyboard.press('Escape')
  // slash commands
  await go(p, '/aria/chat/General', 2000)
  await click(p, '.chat-mdx-editor [contenteditable="true"]')
  await p.keyboard.type('/')
  await p.waitForTimeout(500)
  await snap(p, '11-chat-slash-commands')
  await c.close()
})

def('12-chat-checkpoint', async () => {
  const c = await ctx(); const p = await c.newPage()
  await go(p, '/aria/chat/ui-rewrite', 2500)
  const cp = p.locator('.checkpoint-divider')
  if (await cp.count()) { await cp.first().click(); await p.waitForTimeout(400) }
  await snap(p, '12-chat-checkpoint-handover')
  await c.close()
})

// ---------- Core ----------
def('13-core', async () => {
  const c = await ctx(); const p = await c.newPage()
  await go(p, '/aria/core', 1500); await snap(p, '13-core-editor')
  await p.getByRole('button', { name: 'History' }).click(); await p.waitForTimeout(900)
  await snap(p, '14-core-history')
  await c.close()
})

// ---------- Memory ----------
def('15-memory', async () => {
  const c = await ctx(); const p = await c.newPage()
  await go(p, '/aria/memory', 3500); await snap(p, '15-memory-bundles-graph')
  await p.getByTitle('Force controls').click().catch(() => {}); await p.waitForTimeout(300)
  await snap(p, '16-memory-graph-force-controls')
  await p.getByTitle('Force controls').click().catch(() => {})
  await p.getByRole('button', { name: 'Import' }).click(); await p.waitForTimeout(500)
  await snap(p, '17-memory-import-bundle'); await c.close()

  // Inside a bundle: find nodes by scanning canvas pixels for the node colour
  const c2 = await ctx(); const p2 = await c2.newPage()
  await go(p2, '/aria/memory', 3500)
  const enterNode = async (want) => {
    for (let i = 0; i < 3; i++) { await p2.getByTitle('Zoom in').click(); await p2.waitForTimeout(150) }
    await p2.waitForTimeout(800)
    const pts = await nodePoints(p2)
    const canvas = p2.locator('canvas').first()
    for (const [x, y] of pts) {
      await canvas.click({ position: { x, y } })
      await p2.waitForTimeout(500)
      const open = p2.getByRole('button', { name: 'Open', exact: true })
      if (await open.count()) { await open.first().click(); await p2.waitForTimeout(900) }
      if (await want()) return true
    }
    return false
  }
  await enterNode(async () => (await p2.getByText('Delete Bundle').count()) > 0)
  await p2.waitForTimeout(2500)
  await snap(p2, '18-memory-bundle-graph')
  const opened = await enterNode(async () => (await p2.getByRole('button', { name: 'Edit' }).count()) > 0)
  await p2.waitForTimeout(600)
  await snap(p2, '19-memory-view')
  if (opened) {
    await p2.getByRole('button', { name: 'History' }).last().click(); await p2.waitForTimeout(800)
    await snap(p2, '20-memory-history')
    await p2.getByRole('button', { name: 'Back to memory' }).click(); await p2.waitForTimeout(300)
    await p2.getByRole('button', { name: 'Edit' }).click(); await p2.waitForTimeout(600)
    await snap(p2, '21-memory-edit')
  }
  await c2.close()

  const c3 = await ctx(); const p3 = await c3.newPage()
  await go(p3, '/aria/memory', 2500)
  await p3.getByRole('button', { name: 'New Memory' }).click(); await p3.waitForTimeout(600)
  await snap(p3, '22-memory-create'); await c3.close()
})

// ---------- Tasks ----------
def('23-tasks', async () => {
  const c = await ctx(); const p = await c.newPage()
  await go(p, '/aria/tasks', 1500); await snap(p, '23-tasks-list')
  await p.getByText('Check inbox once').first().click(); await p.waitForTimeout(1200)
  await snap(p, '24-task-detail-runs')
  const run = p.getByText('Past runs').locator('..').locator('button, [role=button]').first()
  await run.click().catch(() => {}); await p.waitForTimeout(1200)
  await snap(p, '25-task-run-expanded')
  await go(p, '/aria/tasks', 1200)
  await p.getByRole('button', { name: 'New Task' }).click(); await p.waitForTimeout(600)
  await snap(p, '26-task-create')
  await c.close()
})

// ---------- Skills ----------
def('27-skills', async () => {
  const c = await ctx(); const p = await c.newPage()
  await go(p, '/aria/skills', 1500); await snap(p, '27-skills-list')
  await p.getByText('paper-summary').first().click(); await p.waitForTimeout(800)
  await snap(p, '28-skill-detail')
  await go(p, '/aria/skills', 1200)
  await p.getByRole('button', { name: 'New Skill' }).click(); await p.waitForTimeout(600)
  await snap(p, '29-skill-create')
  await c.close()
})

// ---------- Dreams ----------
def('30-dream', async () => {
  const c = await ctx({ height: 1100 }); const p = await c.newPage()
  await go(p, '/aria/dream', 1500); await snap(p, '30-dream-journal')
  const btn = p.locator('.main-body button').first()
  if (await btn.count()) { await btn.click(); await p.waitForTimeout(400); const e = p.locator('.main-body button').nth(1); await e.click().catch(() => {}); await p.waitForTimeout(400) }
  await snap(p, '31-dream-cycle-expanded')
  await c.close()
})

// ---------- Usage ----------
def('32-usage', async () => {
  const c = await ctx({ height: 1300 }); const p = await c.newPage()
  await go(p, '/aria/usage', 2500); await snap(p, '32-usage-dashboard')
  await c.close()
})

// ---------- Agent config ----------
def('33-agent-settings', async () => {
  const c = await ctx({ height: 2000 }); const p = await c.newPage()
  await go(p, '/aria/settings', 1800)
  await snap(p, '33-agent-config-1-config')
  const tabs = [['System Prompt', '2-system-prompt'], ['Tools', '3-tools'], ['Sharing', '4-sharing'], ['Danger Zone', '5-danger-zone']]
  for (const [label, slug] of tabs) {
    await p.locator('.main-body .nav-item', { hasText: label }).first().click()
    await p.waitForTimeout(700)
    await snap(p, `33-agent-config-${slug}`)
  }
  await c.close()
  // Tools tab is very long; capture full content height
  const c2 = await ctx({ height: 5200 }); const p2 = await c2.newPage()
  await go(p2, '/aria/settings', 1800)
  await p2.locator('.main-body .nav-item', { hasText: 'Tools' }).first().click(); await p2.waitForTimeout(700)
  for (const lbl of ['Python sandbox', 'Enable Brave Search', 'Enable TTS', 'Enable STT', 'Use vision model for image reading', 'Enable Image Generation', 'Enable Shell']) {
    const cb = p2.getByLabel(lbl, { exact: false }).first()
    if (await cb.count()) await cb.check().catch(() => {}); else await p2.getByText(lbl).first().click().catch(() => {})
    await p2.waitForTimeout(200)
  }
  await p2.getByRole('button', { name: /Add Server/ }).click().catch(() => {}); await p2.waitForTimeout(400)
  const h = await p2.evaluate(() => Math.max(...[...document.querySelectorAll('.main-body, .main-body *')].map(e => e.scrollHeight)))
  await p2.setViewportSize({ width: 1440, height: Math.min(h + 120, 6000) }); await p2.waitForTimeout(500)
  await snap(p2, '33-agent-config-3-tools-all-expanded')
  await c2.close()
})

// ---------- Global settings ----------
def('34-settings', async () => {
  const c = await ctx({ height: 1100 }); const p = await c.newPage()
  await go(p, '/settings', 1500)
  const sections = [['Profile', '1-profile'], ['Password', '2-password'], ['API Keys', '3-api-keys'], ['Providers', '4-providers'], ['Users', '5-users'], ['Roles', '6-roles']]
  for (const [label, slug] of sections) {
    await p.locator('.nav-item:not(.nav-sidebar .nav-item)', { hasText: new RegExp('^' + label + '$') }).first().click()
    await p.waitForTimeout(900)
    await snap(p, `34-settings-${slug}`)
  }
  await c.close()
})

// ---------- Layout variations ----------
def('35-layout', async () => {
  const c = await ctx({ scheme: 'dark' }); const p = await c.newPage()
  await go(p, '/aria/chat/General', 2500); await snap(p, '35-dark-chat')
  await go(p, '/', 1200); await snap(p, '35-dark-home')
  await go(p, '/aria/memory', 3000); await snap(p, '35-dark-memory')
  await go(p, '/aria/settings', 1500); await snap(p, '35-dark-agent-config')
  await c.close()
  const c2 = await ctx(); const p2 = await c2.newPage()
  await go(p2, '/aria/tasks', 1200)
  await p2.locator('.sidebar-toggle').click(); await p2.waitForTimeout(500)
  await snap(p2, '36-sidebar-expanded'); await c2.close()
  const c3 = await ctx({ width: 390, height: 844 }); const p3 = await c3.newPage()
  await go(p3, '/aria/chat/General', 2500); await snap(p3, '37-mobile-chat')
  await p3.locator('.mobile-menu-btn').click(); await p3.waitForTimeout(500)
  await snap(p3, '38-mobile-drawer')
  await go(p3, '/aria/tasks', 1200); await snap(p3, '39-mobile-tasks')
  await c3.close()
})

for (const [name, fn] of shots) {
  if (only && !only.test(name)) continue
  try { await fn(); console.log('ok', name) } catch (e) { console.log('FAIL', name, e.message.split('\n')[0]) }
}
await browser.close()
