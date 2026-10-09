import { chromium } from 'playwright'
const b = await chromium.launch()
const p = await b.newPage({ viewport: { width: 1440, height: 900 }, colorScheme: 'light' })
await p.goto('http://localhost:9911/login'); await p.waitForURL('**/onboarding'); await p.waitForTimeout(800)
await p.screenshot({ path: '../screenshots/01-onboarding.png' })
await b.close()
