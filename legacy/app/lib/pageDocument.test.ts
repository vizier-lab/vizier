// Unit tests for the interactive page documents (specs/019-client-tools R12, FR-026b).
//
//   cd webui && node --test app/lib/pageDocument.test.ts

import test from 'node:test'
import assert from 'node:assert/strict'

import { CSP, PRINTED_MESSAGE, baseStyle, fileName, printDocument, srcdoc, standaloneDocument } from './pageDocument.ts'

test('the drawn page puts the policy and bridge ahead of the agent markup', () => {
  const doc = srcdoc('<p>hi</p>', { n: 1 }, { dark: false, palette: {} })
  assert.ok(doc.indexOf(CSP) < doc.indexOf('window.vizier'))
  assert.ok(doc.indexOf('window.vizier') < doc.indexOf('<p>hi</p>'))
})

test('a download keeps the policy and the latest state, with a vizier that does nothing', () => {
  const doc = standaloneDocument('<p>hi</p>', 'Task <runs>', { n: 3 }, { dark: true, palette: { bg: '#101012' } })
  assert.ok(doc.startsWith('<!doctype html>'))
  assert.ok(doc.includes(`content="${CSP}"`))
  assert.ok(doc.includes('<title>Task &lt;runs></title>'))
  assert.ok(doc.includes('state:{"n":3}'))
  assert.ok(doc.includes('send:function(){}'))
  assert.ok(!doc.includes('postMessage'), 'no bridge to a parent that is not there')
  assert.ok(doc.endsWith('<p>hi</p>'))
  assert.ok(doc.includes('--bg:#101012') && doc.includes('html{background:var(--bg)'))
})

test('a background that could break out of the style falls back to the theme', () => {
  const doc = standaloneDocument('', 't', null, { dark: true, palette: { bg: 'red}</style><script>x()</script>' } })
  assert.ok(doc.includes('--bg:#0a0a0b'))
  assert.ok(!doc.includes('x()'))
})

test('state cannot close the script it is written into', () => {
  const doc = standaloneDocument('', 't', { s: '</script><script>alert(1)</script>' }, { dark: false, palette: {} })
  assert.equal(doc.split('</script>').length, 2)
})

test('the file is named after the title', () => {
  assert.equal(fileName('Task runs — Q3!'), 'task-runs-q3.html')
  assert.equal(fileName('  '), 'page.html')
})

test('the print document disables dialogs before the page runs, then prints once', () => {
  const doc = printDocument('<p>hi</p><script>alert(1)</script>', 'Report', { n: 2 }, { dark: true, palette: { bg: 'rgb(16, 16, 18)' } })
  const disable = doc.indexOf('window.alert=window.confirm=window.prompt=window.print=function(){}')
  assert.ok(disable > 0 && disable < doc.indexOf('<p>hi</p>'))
  assert.ok(doc.includes(`content="${CSP}"`), 'still no network')
  assert.ok(doc.includes('state:{"n":2}'))
  assert.ok(doc.includes('color-scheme:dark'), 'printed in the theme it was drawn in')
  assert.ok(doc.includes('--bg:rgb(16, 16, 18)'), 'on the background it was drawn on')
  assert.ok(doc.includes('print-color-adjust:exact'), 'backgrounds kept when printing')
  assert.ok(doc.includes(PRINTED_MESSAGE))
})

test('every page gets the theme colours as variables, with fallbacks', () => {
  const doc = srcdoc('', null, { dark: false, palette: { accent: ' #10B981 ', text: '' } })
  for (const key of ['bg', 'surface', 'border', 'text', 'muted', 'accent', 'error']) {
    assert.ok(doc.includes(`--${key}:`), key)
  }
  assert.ok(doc.includes('--accent:#10B981'))
  assert.ok(doc.includes('--text:#1a1a1a'), 'missing colours fall back to the light theme')
  assert.ok(doc.includes('color-scheme:light'))
})

test('every page gets the series colours and the derived tokens', () => {
  const doc = srcdoc('', null, { dark: true, palette: {} })
  for (let i = 1; i <= 6; i++) assert.ok(doc.includes(`--series-${i}:#`), `series ${i}`)
  for (const key of ['tint', 'accent-soft', 'accent-text', 'radius', 'shadow', 'font', 'mono']) {
    assert.ok(doc.includes(`--${key}:`), key)
  }
  assert.ok(doc.includes('--ok-bg:#10291f'), 'status colours fall back to the dark theme')
})

test('the kit never outweighs a rule of the page', () => {
  const css = baseStyle({ dark: false, palette: {} })
  const selectors = [...css.matchAll(/(?:^|})([^{}]+)\{/g)].map((m) => m[1].trim())
  const unwrapped = selectors.filter(
    (s) => s !== ':root' && s !== '*,*::before,*::after' && s !== 'html,body' && !s.startsWith(':where(')
  )
  assert.deepEqual(unwrapped, [])
  assert.ok(selectors.length > 40, 'the kit is there')
})
