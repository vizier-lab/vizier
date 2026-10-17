// Unit tests for the client tool registry's display rules (specs/019-client-tools R19).
//
//   cd webui && node --test app/lib/clientTools.test.ts

import test from 'node:test'
import assert from 'node:assert/strict'

import { declaredClientTools, registerClientTool, visibleClientCalls } from './clientTools.ts'
import type { ClientCallView } from '../interfaces/types.ts'

const definition = (name: string, category: string) => ({
  name,
  category,
  description: 'd',
  input_schema: { type: 'object' as const },
})

registerClientTool({ definition: definition('page', 'ui'), handle: () => null, renderAfterTurn: true, latestPerTurn: true })
registerClientTool({ definition: definition('pick', 'palette'), handle: () => null })

const call = (callId: string, category: string, tool: string): ClientCallView => ({
  callId,
  name: `${category}_${tool}`,
  category,
  tool,
  args: {},
  live: false,
})

test('a renderAfterTurn tool is hidden while its turn runs', () => {
  const calls = [call('a', 'ui', 'page'), call('b', 'palette', 'pick')]
  assert.deepEqual(visibleClientCalls(calls, true).map((c) => c.callId), ['b'])
  assert.deepEqual(visibleClientCalls(calls, false).map((c) => c.callId), ['a', 'b'])
})

test('a latestPerTurn tool shows only its last call', () => {
  const calls = [call('a', 'ui', 'page'), call('b', 'palette', 'pick'), call('c', 'ui', 'page'), call('d', 'palette', 'pick')]
  assert.deepEqual(visibleClientCalls(calls, false).map((c) => c.callId), ['b', 'c', 'd'])
})

test('unregistered tools are always shown', () => {
  const calls = [call('a', 'x', 'y'), call('b', 'x', 'y')]
  assert.deepEqual(visibleClientCalls(calls, true).map((c) => c.callId), ['a', 'b'])
})

test('declarations carry their options to the server', () => {
  registerClientTool({ definition: { ...definition('later', 'ui'), deferrable: true }, handle: () => null })
  const later = declaredClientTools().client_tools.find((t) => t.name === 'later')
  assert.equal(later?.deferrable, true)
})
