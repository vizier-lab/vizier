// Unit tests for the chat status strip's filters. Run with:
//
//   cd webui && npx vitest run app/lib/statusFilters.test.ts
//
// The strip's panels narrow one list at a time: a query, a filter tab, or both.
// The counts the tabs carry must agree with what the tab then shows.

import { test } from 'vitest'
import assert from 'node:assert/strict'

import type { BackgroundJobSnapshot, SessionFileRecord } from '../api/frames.ts'
import type { Recalled } from './recall.ts'
import {
  fileKinds,
  filterFiles,
  filterJobs,
  filterMemory,
  jobCounts,
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

const job = (id: string, state: BackgroundJobSnapshot['state'], prompt = 'do the thing'): BackgroundJobSnapshot => ({
  id,
  kind: 'batch',
  delegated_to: null,
  state,
  created_at: '2026-11-04T09:00:00Z',
  finished_at: null,
  timeout_secs: 600,
  pieces: [
    {
      ordinal: 0,
      prompt,
      state: state === 'running' ? 'running' : 'answered',
      reason: null,
      agent_id: 'a',
      topic: null,
      started_at: '2026-11-04T09:00:00Z',
      finished_at: null,
    },
  ],
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

test('job counts split the states the tabs show', () => {
  const jobs = [job('1', 'running'), job('2', 'reporting'), job('3', 'reported'), job('4', 'cancelled')]
  assert.deepEqual(jobCounts(jobs), { running: 2, done: 1, stopped: 1, total: 4 })
})

test('the job tab keeps only its state', () => {
  const jobs = [job('1', 'running'), job('2', 'reported'), job('3', 'interrupted')]
  const href = () => ''
  assert.equal(filterJobs(jobs, '', 'running').length, 1)
  assert.equal(filterJobs(jobs, '', 'done').length, 1)
  assert.equal(filterJobs(jobs, '', 'stopped').length, 1)
  assert.equal(filterJobs(jobs, '', 'all').length, 3)
})

test('the job query matches the kind, the delegate or the goal', () => {
  const jobs = [job('1', 'running', 'index the docs'), job('2', 'running', 'email mira')]
  assert.equal(filterJobs(jobs, 'email', 'all').length, 1)
  assert.equal(filterJobs(jobs, 'batch', 'all').length, 2)
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
