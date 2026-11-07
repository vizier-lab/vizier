import type { BackgroundJobSnapshot, SessionFileRecord } from '../api/frames'
import type { Recalled } from './recall'

/** The sections the chat's status strip holds. */
export type StatusSection = 'memory' | 'jobs' | 'files'

/** A row in a section's list, flattened for the shared row renderer. */
export type StatusRow =
  | { kind: 'memory'; key: string; passage: Recalled; href: string }
  | { kind: 'job'; key: string; job: BackgroundJobSnapshot }
  | { kind: 'file'; key: string; file: SessionFileRecord }

/** How many rows a section shows at once; the panel scrolls for the rest. */
export const PAGE = 25

const matches = (query: string, ...parts: (string | undefined | null)[]) => {
  const q = query.trim().toLowerCase()
  if (!q) return true
  return parts.some((p) => (p ?? '').toLowerCase().includes(q))
}

/** The bundles a memory section holds, most passages first, for the filter tabs. */
export function memoryBundles(recalled: Recalled[]): { value: string; label: string; count: number }[] {
  const counts = new Map<string, number>()
  for (const p of recalled) counts.set(p.bundle, (counts.get(p.bundle) ?? 0) + 1)
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([bundle, count]) => ({ value: bundle, label: bundle, count }))
}

/** The passages a memory section shows: the query and the bundle tab narrow them. */
export function filterMemory(
  recalled: Recalled[],
  query: string,
  bundle: string | null,
  href: (p: Recalled) => string,
): StatusRow[] {
  const rows: StatusRow[] = []
  for (const p of recalled) {
    if (bundle && p.bundle !== bundle) continue
    if (!matches(query, p.path, p.bundle, p.title)) continue
    rows.push({ kind: 'memory', key: `${p.uid}-${p.bundle}-${p.path}-${p.ordinal}`, passage: p, href: href(p) })
  }
  return rows
}

/** The job states the jobs tabs count. */
export function jobCounts(jobs: BackgroundJobSnapshot[]) {
  const running = jobs.filter((j) => j.state === 'running' || j.state === 'reporting')
  const done = jobs.filter((j) => j.state === 'reported' || j.state === 'undelivered')
  const stopped = jobs.filter((j) => j.state === 'cancelled' || j.state === 'interrupted')
  return { running: running.length, done: done.length, stopped: stopped.length, total: jobs.length }
}

/** The jobs a section shows: the query narrows them, the tab picks a state. */
export function filterJobs(
  jobs: BackgroundJobSnapshot[],
  query: string,
  tab: 'all' | 'running' | 'done' | 'stopped',
): StatusRow[] {
  const rows: StatusRow[] = []
  for (const j of jobs) {
    if (tab === 'running' && !(j.state === 'running' || j.state === 'reporting')) continue
    if (tab === 'done' && !(j.state === 'reported' || j.state === 'undelivered')) continue
    if (tab === 'stopped' && !(j.state === 'cancelled' || j.state === 'interrupted')) continue
    if (!matches(query, j.kind, j.delegated_to, j.pieces[0]?.prompt)) continue
    rows.push({ kind: 'job', key: j.id, job: j })
  }
  return rows
}

/** The file kinds the files tabs count, by extension. */
export function fileKinds(files: SessionFileRecord[]): { value: string; label: string; count: number }[] {
  const counts = new Map<string, number>()
  for (const f of files) {
    const ext = (f.filename.split('.').pop() ?? '').toLowerCase()
    const kind = ext === 'png' || ext === 'jpg' || ext === 'jpeg' || ext === 'gif' || ext === 'webp' ? 'images' : ext
    counts.set(kind, (counts.get(kind) ?? 0) + 1)
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([value, count]) => ({ value, label: value, count }))
}

/** The files a section shows: the query narrows them, the tab picks a kind. */
export function filterFiles(
  files: SessionFileRecord[],
  query: string,
  kind: string | null,
): StatusRow[] {
  const rows: StatusRow[] = []
  for (const f of files) {
    const ext = (f.filename.split('.').pop() ?? '').toLowerCase()
    const fileKind = ext === 'png' || ext === 'jpg' || ext === 'jpeg' || ext === 'gif' || ext === 'webp' ? 'images' : ext
    if (kind && fileKind !== kind) continue
    if (!matches(query, f.filename, f.mime_type, f.sender)) continue
    rows.push({ kind: 'file', key: f.id, file: f })
  }
  return rows
}