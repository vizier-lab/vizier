// Unit tests for the chat status strip's filters. Run with:
//
//   cd webui && npx vitest run app/lib/statusFilters.test.ts
//
// The strip's panels narrow one list at a time: a query, a filter tab, or both.
// The counts the tabs carry must agree with what the tab then shows.

import { test } from 'vitest'
import assert from 'node:assert/strict'

import type { DetachedScriptSnapshot, SessionFileRecord } from '../api/frames.ts'
import type { Recalled } from './recall.ts'
import {
  fileKinds,
  filterFiles,
  filterMemory,
  filterWork,
  workCounts,
  memoryBundles,
} from './statusFilters.ts'

const passage = (bundle: string, path: string, score = 0.8): Recalled => ({
  bundle,
  path,
  ordinal: 1,
  score,
  origin: 'auto',
  uid: `${bundle}-${path}`,
})

const script = (
  id: string,
  state: DetachedScriptSnapshot['state'],
  intent = 'do the thing',
  created_at = '2026-11-04T09:00:00Z',
): DetachedScriptSnapshot => ({
  id,
  intent,
  state,
  created_at,
  finished_at: state === 'running' ? null : '2026-11-04T09:01:00Z',
  timeout_secs: 600,
  parent_id: null,
  delivered: state !== 'running' && state !== 'cancelled',
  tool_call_count: 1,
})

const file = (id: string, filename: string): SessionFileRecord => ({
  id,
  session_slug: 'vizier-webui',
  agent_id: 'a',
  filename,
  mime_type: 'application/octet-stream',
  size: 1024,
  file_id: id,
  added_at: '2026-11-04T09:00:00Z',
})

test('memory bundles are counted and ranked by how many passages they hold', () => {
  const recalled = [passage('ops', 'a.md'), passage('ops', 'b.md'), passage('people', 'c.md')]
  assert.deepEqual(memoryBundles(recalled), [
    { value: 'ops', label: 'ops', count: 2 },
    { value: 'people', label: 'people', count: 1 },
  ])
})

test('the memory query matches the path, the bundle or the title', () => {
  const recalled = [passage('ops', 'on-call.md'), passage('people', 'mira.md')]
  const href = () => '/memory'
  assert.equal(filterMemory(recalled, 'on-call', null, href).length, 1)
  assert.equal(filterMemory(recalled, 'people', null, href).length, 1)
  assert.equal(filterMemory(recalled, 'nothing', null, href).length, 0)
})

test('the memory bundle tab keeps only that bundle', () => {
  const recalled = [passage('ops', 'a.md'), passage('people', 'b.md')]
  const href = () => '/memory'
  assert.equal(filterMemory(recalled, '', 'ops', href).length, 1)
  assert.equal(filterMemory(recalled, '', null, href).length, 2)
})

test('work counts split the states the tabs show', () => {
  const scripts = [
    script('d_1', 'running'),
    script('d_2', 'succeeded'),
    script('d_3', 'failed'),
    script('d_4', 'cancelled'),
    script('d_5', 'timed_out'),
    script('d_6', 'interrupted'),
  ]
  assert.deepEqual(workCounts(scripts), { running: 1, done: 2, stopped: 3, total: 6 })
})

test('the work tab keeps only its state, and the counts agree with it', () => {
  const scripts = [script('d_1', 'running'), script('d_2', 'succeeded'), script('d_3', 'interrupted')]
  const counts = workCounts(scripts)
  assert.equal(filterWork(scripts, '', 'running').length, counts.running)
  assert.equal(filterWork(scripts, '', 'done').length, counts.done)
  assert.equal(filterWork(scripts, '', 'stopped').length, counts.stopped)
})

test('the work query matches the intent or the handle, newest first', () => {
  const scripts = [
    script('d_1', 'running', 'index the docs', '2026-11-04T09:00:00Z'),
    script('d_2', 'running', 'email mira', '2026-11-04T09:05:00Z'),
  ]
  assert.equal(filterWork(scripts, 'email', 'running').length, 1)
  assert.equal(filterWork(scripts, 'd_1', 'running').length, 1)
  assert.deepEqual(filterWork(scripts, '', 'running').map((r) => r.key), ['d_2', 'd_1'])
})

test('file kinds count by extension, with images as one kind', () => {
  const files = [file('1', 'a.png'), file('2', 'b.jpg'), file('3', 'c.pdf'), file('4', 'd.log')]
  const kinds = fileKinds(files)
  assert.deepEqual(kinds[0], { value: 'images', label: 'images', count: 2 })
  assert.equal(kinds.length, 3)
})

test('the file query matches the name, the type or the sender', () => {
  const files = [file('1', 'report.pdf'), file('2', 'notes.md')]
  assert.equal(filterFiles(files, 'report', null).length, 1)
  assert.equal(filterFiles(files, 'md', null).length, 1)
  assert.equal(filterFiles(files, 'nothing', null).length, 0)
})

test('the file kind tab keeps only that kind', () => {
  const files = [file('1', 'a.png'), file('2', 'b.pdf')]
  assert.equal(filterFiles(files, '', 'images').length, 1)
  assert.equal(filterFiles(files, '', 'pdf').length, 1)
  assert.equal(filterFiles(files, '', null).length, 2)
})
