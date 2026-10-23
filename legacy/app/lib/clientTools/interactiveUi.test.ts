// cd webui && node --test app/lib/clientTools/interactiveUi.test.ts

import test from 'node:test'
import assert from 'node:assert/strict'

import { RENDER_INTERACTIVE_UI_DESCRIPTION } from './interactiveUi.ts'

test('the description fits the server limit on a declaration', () => {
  assert.ok(new TextEncoder().encode(RENDER_INTERACTIVE_UI_DESCRIPTION).length <= 2048)
})
