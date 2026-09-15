import { useCallback, useEffect, useState } from 'react'
import { FaArrowLeft, FaClockRotateLeft, FaCodeCompare, FaEye } from 'react-icons/fa6'
import { useToastStore } from '../hooks/toastStore'
import type {
  HistoryRow,
  HistoryVersion,
  RevisionActor,
  RevisionDiff,
  RevisionTrigger,
  RollbackResponse,
} from '../interfaces/types'

// One presentational panel for both document kinds (CORE + memory). Each route hands it a
// small adapter bound to its own service functions, and maps its API rows into `HistoryRow`.
// specs/006-memory-version-history/contracts/webui.md
export interface VersionHistorySource {
  list: (offset: number, limit: number) => Promise<{ rows: HistoryRow[]; total: number }>
  get: (seq: number) => Promise<HistoryVersion>
  diff: (to: number, from?: number) => Promise<RevisionDiff>
  rollback: (seq: number) => Promise<RollbackResponse>
}

interface VersionHistoryProps {
  source: VersionHistorySource
  /** "CORE" or the memory title — used in copy only. */
  label: string
  /** Extra line shown inside the restore confirmation (e.g. "unsaved changes will be lost"). */
  extraWarning?: string
  onRolledBack?: (res: RollbackResponse) => void
}

const PAGE = 50

type Pane = { kind: 'list' } | { kind: 'version'; version: HistoryVersion } | { kind: 'diff'; diff: RevisionDiff }

function getErrorMessage(err: unknown): string {
  if (err && typeof err === 'object' && 'response' in err) {
    const resp = (err as { response?: { data?: { message?: string } } }).response
    return resp?.data?.message || 'An error occurred'
  }
  if (err instanceof Error) return err.message
  return 'An error occurred'
}

export function actorLabel(actor: RevisionActor): string {
  switch (actor.type) {
    case 'agent':
      return 'Agent'
    case 'user':
      return actor.username ? `@${actor.username}` : 'User'
    case 'system':
      return 'System'
  }
}

export function triggerLabel(trigger: RevisionTrigger): string {
  switch (trigger.type) {
    case 'conversation':
      return 'conversation'
    case 'dream':
      return 'dream cycle'
    case 'webui':
      return 'WebUI'
    case 'api':
      return 'API'
    case 'import':
      return 'import'
    case 'rollback':
      return `restored from v${trigger.restored_from}`
    case 'baseline':
      return 'baseline'
  }
}

function relativeTime(iso: string): string {
  const then = new Date(iso).getTime()
  const diff = Math.max(0, Date.now() - then)
  const s = Math.floor(diff / 1000)
  if (s < 60) return 'just now'
  const m = Math.floor(s / 60)
  if (m < 60) return `${m}m ago`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h}h ago`
  const d = Math.floor(h / 24)
  if (d < 30) return `${d}d ago`
  const mo = Math.floor(d / 30)
  if (mo < 12) return `${mo}mo ago`
  return `${Math.floor(mo / 12)}y ago`
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / (1024 * 1024)).toFixed(1)} MB`
}

export function DiffView({ diff }: { diff: RevisionDiff }) {
  if (diff.hunks.length === 0) {
    return (
      <p style={{ color: 'var(--text-tertiary)', fontSize: '13px' }}>
        No line changes between v{diff.from_seq} and v{diff.to_seq}.
      </p>
    )
  }
  return (
    <div className="diff-view">
      {diff.hunks.map((hunk, i) => (
        <div key={i}>
          <div className="diff-hunk-header">
            @@ -{hunk.old_start},{hunk.old_lines} +{hunk.new_start},{hunk.new_lines} @@
          </div>
          {hunk.lines.map((line, j) => (
            <div key={j} className={`diff-line ${line.op}`}>
              <span className="diff-gutter">{line.old_line ?? ''}</span>
              <span className="diff-gutter">{line.new_line ?? ''}</span>
              <span className="diff-sign">
                {line.op === 'insert' ? '+' : line.op === 'delete' ? '-' : ' '}
              </span>
              <span className="diff-text">{line.text || ' '}</span>
            </div>
          ))}
        </div>
      ))}
    </div>
  )
}

export default function VersionHistory({ source, label, extraWarning, onRolledBack }: VersionHistoryProps) {
  const addToast = useToastStore((s) => s.addToast)

  const [rows, setRows] = useState<HistoryRow[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState(false)
  const [pane, setPane] = useState<Pane>({ kind: 'list' })

  const [compareMode, setCompareMode] = useState(false)
  const [picked, setPicked] = useState<number[]>([])

  const [confirmSeq, setConfirmSeq] = useState<number | null>(null)

  const loadPage = useCallback(
    async (offset: number) => {
      const page = await source.list(offset, PAGE)
      setRows((prev) => (offset === 0 ? page.rows : [...prev, ...page.rows]))
      setTotal(page.total)
    },
    [source]
  )

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    source
      .list(0, PAGE)
      .then((page) => {
        if (cancelled) return
        setRows(page.rows)
        setTotal(page.total)
      })
      .catch((err) => {
        if (cancelled) return
        addToast('error', `Failed to load ${label} history`, getErrorMessage(err))
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
    // The adapter is rebuilt by the parent on every render; only refetch when the label
    // (i.e. the document) changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [label])

  const withBusy = async (fn: () => Promise<void>, failTitle: string) => {
    setBusy(true)
    try {
      await fn()
    } catch (err) {
      addToast('error', failTitle, getErrorMessage(err))
    } finally {
      setBusy(false)
    }
  }

  const openVersion = (seq: number) =>
    withBusy(async () => {
      const version = await source.get(seq)
      setPane({ kind: 'version', version })
    }, `Failed to load v${seq}`)

  const openChanges = (seq: number) =>
    withBusy(async () => {
      const diff = await source.diff(seq)
      setPane({ kind: 'diff', diff })
    }, `Failed to diff v${seq}`)

  const openCompare = () => {
    if (picked.length !== 2) return
    const [a, b] = picked
    const to = Math.max(a, b)
    const from = Math.min(a, b)
    void withBusy(async () => {
      const diff = await source.diff(to, from)
      setPane({ kind: 'diff', diff })
    }, `Failed to compare v${from} → v${to}`)
  }

  const togglePick = (seq: number) => {
    setPicked((prev) => {
      if (prev.includes(seq)) return prev.filter((s) => s !== seq)
      if (prev.length >= 2) return [prev[1], seq]
      return [...prev, seq]
    })
  }

  const loadMore = () => withBusy(() => loadPage(rows.length), 'Failed to load more versions')

  const confirmRestore = () => {
    if (confirmSeq === null) return
    const seq = confirmSeq
    setConfirmSeq(null)
    void withBusy(async () => {
      const res = await source.rollback(seq)
      if (res.no_change) {
        addToast('info', 'Already identical to current — nothing changed')
      } else {
        addToast('success', `Restored v${seq} as v${res.new_seq}`)
      }
      await loadPage(0)
      setPane({ kind: 'list' })
      setCompareMode(false)
      setPicked([])
      onRolledBack?.(res)
    }, `Failed to restore v${seq}`)
  }

  const current = rows.find((r) => r.is_current)
  const nextSeq = (current?.seq ?? rows[0]?.seq ?? 0) + 1
  const confirmRow = confirmSeq === null ? null : rows.find((r) => r.seq === confirmSeq) ?? null

  const canRestore = (row: HistoryRow) => !row.is_current && !row.deleted

  const RestoreButton = ({ row, primary }: { row: HistoryRow; primary?: boolean }) =>
    canRestore(row) ? (
      <button
        className={primary ? 'btn btn-primary' : 'btn btn-ghost'}
        onClick={() => setConfirmSeq(row.seq)}
        disabled={busy}
        title={`Restore v${row.seq} as a new version`}
      >
        <FaClockRotateLeft size={12} />
        Restore
      </button>
    ) : null

  const backToList = () => setPane({ kind: 'list' })

  const paneHeader = (title: string) => (
    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '12px' }}>
      <button className="btn btn-ghost" onClick={backToList} disabled={busy}>
        <FaArrowLeft size={12} />
        Back
      </button>
      <strong style={{ fontSize: '14px' }}>{title}</strong>
    </div>
  )

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', flex: 1, minHeight: 0 }}>
      {pane.kind === 'list' && (
        <>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
            <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
              {loading ? 'Loading…' : `${total} version${total === 1 ? '' : 's'} of ${label}`}
            </span>
            <div style={{ marginLeft: 'auto', display: 'flex', gap: '8px', alignItems: 'center' }}>
              {compareMode && (
                <button
                  className="btn btn-primary"
                  onClick={openCompare}
                  disabled={busy || picked.length !== 2}
                >
                  <FaCodeCompare size={12} />
                  {picked.length === 2
                    ? `Compare v${Math.min(...picked)} → v${Math.max(...picked)}`
                    : 'Pick two versions'}
                </button>
              )}
              <button
                className={compareMode ? 'btn btn-secondary' : 'btn btn-ghost'}
                onClick={() => {
                  setCompareMode((v) => !v)
                  setPicked([])
                }}
                disabled={busy || rows.length < 2}
              >
                {compareMode ? 'Cancel compare' : 'Compare two'}
              </button>
            </div>
          </div>

          {!loading && rows.length === 0 && (
            <p style={{ color: 'var(--text-tertiary)', fontSize: '13px' }}>No versions recorded yet.</p>
          )}

          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            {rows.map((row) => (
              <div key={row.seq} className={`history-row${row.is_current ? ' current' : ''}`}>
                {compareMode && (
                  <input
                    type="checkbox"
                    checked={picked.includes(row.seq)}
                    onChange={() => togglePick(row.seq)}
                    aria-label={`Select v${row.seq} for comparison`}
                  />
                )}
                <strong style={{ fontFamily: 'var(--font-mono)', minWidth: '3em' }}>v{row.seq}</strong>
                {row.is_current && <span className="history-badge current">Current</span>}
                {row.deleted && <span className="history-badge deleted">Deleted</span>}
                <span style={{ fontSize: '13px' }}>{actorLabel(row.actor)}</span>
                <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>{triggerLabel(row.trigger)}</span>
                <span
                  style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}
                  title={new Date(row.created_at).toLocaleString()}
                >
                  {relativeTime(row.created_at)}
                </span>
                {!row.deleted && (
                  <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>{formatBytes(row.size_bytes)}</span>
                )}
                <div style={{ marginLeft: 'auto', display: 'flex', gap: '4px' }}>
                  <button className="btn btn-ghost" onClick={() => openVersion(row.seq)} disabled={busy}>
                    <FaEye size={12} />
                    View
                  </button>
                  {row.seq > 1 && !row.deleted && (
                    <button className="btn btn-ghost" onClick={() => openChanges(row.seq)} disabled={busy}>
                      <FaCodeCompare size={12} />
                      Changes
                    </button>
                  )}
                  <RestoreButton row={row} />
                </div>
              </div>
            ))}
          </div>

          {rows.length < total && (
            <button className="btn btn-secondary" onClick={loadMore} disabled={busy} style={{ alignSelf: 'center' }}>
              Load more ({total - rows.length} older)
            </button>
          )}
        </>
      )}

      {pane.kind === 'version' && (
        <>
          {paneHeader(`v${pane.version.seq}`)}
          <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap', alignItems: 'center', fontSize: '12px', color: 'var(--text-tertiary)' }}>
            <span>{actorLabel(pane.version.actor)}</span>
            <span>{triggerLabel(pane.version.trigger)}</span>
            <span title={new Date(pane.version.created_at).toLocaleString()}>{relativeTime(pane.version.created_at)}</span>
            {pane.version.is_current && <span className="history-badge current">Current</span>}
            {pane.version.deleted && <span className="history-badge deleted">Deleted</span>}
            <div style={{ marginLeft: 'auto', display: 'flex', gap: '4px' }}>
              {pane.version.seq > 1 && !pane.version.deleted && (
                <button className="btn btn-ghost" onClick={() => openChanges(pane.version.seq)} disabled={busy}>
                  <FaCodeCompare size={12} />
                  Changes
                </button>
              )}
              <RestoreButton row={pane.version} primary />
            </div>
          </div>
          {pane.version.deleted ? (
            <p style={{ color: 'var(--text-secondary)', fontSize: '13px' }}>
              This version records a deletion — {label} did not exist after it. Restore an earlier
              content version to bring it back.
            </p>
          ) : (
            <>
              {(pane.version.title || (pane.version.tags && pane.version.tags.length > 0)) && (
                <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
                  {pane.version.title && <strong style={{ fontSize: '14px' }}>{pane.version.title}</strong>}
                  {pane.version.tags?.map((tag) => (
                    <span key={tag} className="history-badge">
                      {tag}
                    </span>
                  ))}
                </div>
              )}
              <div className="history-content">{pane.version.content ?? ''}</div>
            </>
          )}
        </>
      )}

      {pane.kind === 'diff' && (
        <>
          {paneHeader(
            pane.diff.from_seq >= 1
              ? `Changes v${pane.diff.from_seq} → v${pane.diff.to_seq}`
              : `v${pane.diff.to_seq} (first version)`
          )}
          <div style={{ display: 'flex', gap: '12px', alignItems: 'center', fontSize: '12px' }}>
            <span style={{ color: 'var(--accent-dark)' }}>+{pane.diff.additions}</span>
            <span style={{ color: '#ef4444' }}>−{pane.diff.deletions}</span>
            {(() => {
              const row = rows.find((r) => r.seq === pane.diff.to_seq)
              return row ? (
                <div style={{ marginLeft: 'auto' }}>
                  <RestoreButton row={row} primary />
                </div>
              ) : null
            })()}
          </div>
          <DiffView diff={pane.diff} />
        </>
      )}

      {confirmRow && (
        <div
          onClick={() => setConfirmSeq(null)}
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0, 0, 0, 0.4)',
            backdropFilter: 'blur(4px)',
            zIndex: 1100,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            padding: '16px',
          }}
        >
          <div
            role="dialog"
            aria-modal="true"
            onClick={(e) => e.stopPropagation()}
            style={{
              background: 'var(--background)',
              border: '1px solid var(--border)',
              borderRadius: '12px',
              boxShadow: 'var(--shadow-xl)',
              padding: '1.25rem',
              maxWidth: '440px',
              width: '100%',
              display: 'flex',
              flexDirection: 'column',
              gap: '12px',
            }}
          >
            <h3 style={{ margin: 0, fontSize: '1rem' }}>Restore v{confirmRow.seq} of {label}?</h3>
            <p style={{ margin: 0, fontSize: '13px', color: 'var(--text-secondary)' }}>
              v{confirmRow.seq} · {actorLabel(confirmRow.actor)} · {triggerLabel(confirmRow.trigger)} ·{' '}
              {new Date(confirmRow.created_at).toLocaleString()}
            </p>
            <p style={{ margin: 0, fontSize: '13px' }}>
              v{confirmRow.seq}'s content will be saved as a new version v{nextSeq}. Nothing is deleted —
              every existing version stays in the history.
            </p>
            {extraWarning && (
              <p style={{ margin: 0, fontSize: '13px', color: '#ef4444' }}>{extraWarning}</p>
            )}
            <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
              <button className="btn btn-secondary" onClick={() => setConfirmSeq(null)} disabled={busy}>
                Cancel
              </button>
              <button className="btn btn-primary" onClick={confirmRestore} disabled={busy}>
                <FaClockRotateLeft size={12} />
                Restore v{confirmRow.seq}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
