import { useCallback, useEffect, useRef, useState } from 'react'
import { useParams } from 'react-router'
import {
  triggerDream,
  getDreamStatus,
  listDreamEntries,
} from '../services/vizier'
import { Skeleton } from '../components/Skeleton'
import { useToastStore } from '../hooks/toastStore'
import type { DreamJournalEntry, DreamStatusResponse } from '../interfaces/types'

function getErrorMessage(err: unknown): string {
  if (err && typeof err === 'object' && 'response' in err) {
    const resp = (err as { response?: { data?: { message?: string } } }).response
    return resp?.data?.message || 'An error occurred'
  }
  return 'An error occurred'
}

interface DreamCycle {
  cycle_id: string
  extractions: DreamJournalEntry[]
  consolidation: DreamJournalEntry | null
  timestamp: string
}

export default function AgentDream() {
  const { agentId } = useParams()
  const addToast = useToastStore((s) => s.addToast)
  const [status, setStatus] = useState<DreamStatusResponse | null>(null)
  const [entries, setEntries] = useState<DreamJournalEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [expandedCycles, setExpandedCycles] = useState<Set<string>>(new Set())
  const [expandedEntries, setExpandedEntries] = useState<Set<string>>(new Set())
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const isDreaming =
    status?.status === 'extracting' || status?.status === 'consolidating'

  const loadData = useCallback(async () => {
    if (!agentId) return
    try {
      const [statusRes, entriesRes] = await Promise.all([
        getDreamStatus(agentId),
        listDreamEntries(agentId, 50),
      ])
      setStatus(statusRes.data)
      setEntries(entriesRes.data || [])
    } catch (error) {
      console.error('Failed to load dream data:', error)
    } finally {
      setLoading(false)
    }
  }, [agentId])

  useEffect(() => {
    loadData()
  }, [loadData])

  // Poll: 5s when dreaming, 30s when idle
  useEffect(() => {
    if (pollRef.current) clearInterval(pollRef.current)
    const interval = isDreaming ? 5000 : 30000
    pollRef.current = setInterval(loadData, interval)
    return () => {
      if (pollRef.current) clearInterval(pollRef.current)
    }
  }, [isDreaming, loadData])

  const handleTriggerDream = async () => {
    if (!agentId) return
    try {
      await triggerDream(agentId)
      addToast('success', 'Dream cycle started')
      setTimeout(loadData, 1000)
    } catch (error) {
      addToast('error', getErrorMessage(error))
    }
  }

  const toggleCycle = (cycleId: string) => {
    setExpandedCycles((prev) => {
      const next = new Set(prev)
      if (next.has(cycleId)) next.delete(cycleId)
      else next.add(cycleId)
      return next
    })
  }

  const toggleEntry = (entryId: string) => {
    setExpandedEntries((prev) => {
      const next = new Set(prev)
      if (next.has(entryId)) next.delete(entryId)
      else next.add(entryId)
      return next
    })
  }

  // Group entries by cycle_id
  const cycles: DreamCycle[] = []
  const cycleMap = new Map<string, DreamCycle>()
  for (const entry of entries) {
    let cycle = cycleMap.get(entry.dream_cycle_id)
    if (!cycle) {
      cycle = {
        cycle_id: entry.dream_cycle_id,
        extractions: [],
        consolidation: null,
        timestamp: entry.timestamp,
      }
      cycleMap.set(entry.dream_cycle_id, cycle)
      cycles.push(cycle)
    }
    if (entry.stage === 'extraction') {
      cycle.extractions.push(entry)
    } else {
      cycle.consolidation = entry
    }
  }

  if (loading) {
    return (
      <div style={{ padding: '2rem' }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
          {Array.from({ length: 5 }).map((_, i) => (
            <Skeleton key={i} variant="rounded" height={48} />
          ))}
        </div>
      </div>
    )
  }

  const statusColor =
    status?.status === 'idle'
      ? '#22c55e'
      : status?.status === 'extracting'
        ? '#f59e0b'
        : '#3b82f6'

  return (
    <div style={{ padding: '2rem', maxWidth: '900px', margin: '0 auto' }}>
      {/* Header */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '1.5rem',
        }}
      >
        <h1 style={{ fontSize: '1.5rem', fontWeight: 700 }}>Dream Journal</h1>
        <button
          onClick={handleTriggerDream}
          disabled={isDreaming}
          style={{
            padding: '0.5rem 1rem',
            borderRadius: '0.5rem',
            border: 'none',
            background: isDreaming ? '#6b7280' : '#8b5cf6',
            color: 'white',
            fontWeight: 600,
            cursor: isDreaming ? 'not-allowed' : 'pointer',
            opacity: isDreaming ? 0.6 : 1,
          }}
        >
          {isDreaming ? 'Dreaming...' : 'Dream Now'}
        </button>
      </div>

      {/* Status Bar */}
      <div
        style={{
          display: 'flex',
          gap: '2rem',
          padding: '1rem 1.25rem',
          borderRadius: '0.75rem',
          background: 'var(--color-surface, #1a1a2e)',
          marginBottom: '1.5rem',
          flexWrap: 'wrap',
        }}
      >
        <div>
          <div
            style={{
              fontSize: '0.75rem',
              color: '#9ca3af',
              marginBottom: '0.25rem',
            }}
          >
            Status
          </div>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '0.5rem',
              fontWeight: 600,
            }}
          >
            <span
              style={{
                width: '0.5rem',
                height: '0.5rem',
                borderRadius: '50%',
                background: statusColor,
                display: 'inline-block',
              }}
            />
            {status?.status === 'idle'
              ? 'Idle'
              : status?.status === 'extracting'
                ? `Extracting (${status.completed_sessions ?? 0}/${status.total_sessions ?? 0})`
                : 'Consolidating'}
          </div>
        </div>
        <div>
          <div
            style={{
              fontSize: '0.75rem',
              color: '#9ca3af',
              marginBottom: '0.25rem',
            }}
          >
            Last Dream
          </div>
          <div style={{ fontWeight: 600 }}>
            {status?.last_dream
              ? new Date(status.last_dream).toLocaleString()
              : 'Never'}
          </div>
        </div>
        <div>
          <div
            style={{
              fontSize: '0.75rem',
              color: '#9ca3af',
              marginBottom: '0.25rem',
            }}
          >
            Next Dream
          </div>
          <div style={{ fontWeight: 600 }}>
            {status?.next_dream
              ? new Date(status.next_dream).toLocaleString()
              : 'Not scheduled'}
          </div>
        </div>
        {status?.dream_model && (
          <div>
            <div
              style={{
                fontSize: '0.75rem',
                color: '#9ca3af',
                marginBottom: '0.25rem',
              }}
            >
              Model
            </div>
            <div style={{ fontWeight: 600 }}>
              {status.dream_provider
                ? `${status.dream_provider}/${status.dream_model}`
                : status.dream_model}
            </div>
          </div>
        )}
      </div>

      {/* Timeline */}
      {cycles.length === 0 ? (
        <div
          style={{
            textAlign: 'center',
            color: '#6b7280',
            padding: '3rem 0',
          }}
        >
          No dream entries yet. Enable dreaming in agent settings and wait for
          the first cycle, or click "Dream Now".
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
          {cycles.map((cycle) => {
            const isExpanded = expandedCycles.has(cycle.cycle_id)
            const isComplete = !!cycle.consolidation
            const totalDuration = cycle.extractions.reduce(
              (sum, e) => sum + (e.duration_ms || 0),
              0
            ) + (cycle.consolidation?.duration_ms || 0)

            return (
              <div
                key={cycle.cycle_id}
                style={{
                  borderRadius: '0.75rem',
                  border: '1px solid var(--color-border, #2a2a3e)',
                  overflow: 'hidden',
                }}
              >
                {/* Cycle Header */}
                <button
                  onClick={() => toggleCycle(cycle.cycle_id)}
                  style={{
                    width: '100%',
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    padding: '0.75rem 1rem',
                    background: 'var(--color-surface, #1a1a2e)',
                    border: 'none',
                    cursor: 'pointer',
                    color: 'inherit',
                    textAlign: 'left',
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                    <span style={{ fontSize: '0.75rem', color: '#9ca3af' }}>
                      {isExpanded ? '▼' : '▶'}
                    </span>
                    <span style={{ fontWeight: 600 }}>
                      {new Date(cycle.timestamp).toLocaleDateString('en-US', {
                        month: 'short',
                        day: 'numeric',
                        year: 'numeric',
                        hour: '2-digit',
                        minute: '2-digit',
                      })}
                    </span>
                    <span style={{ fontSize: '0.85rem', color: '#9ca3af' }}>
                      {cycle.extractions.length} session{cycle.extractions.length !== 1 ? 's' : ''}
                      {totalDuration > 0 && ` · ${(totalDuration / 1000).toFixed(1)}s`}
                    </span>
                  </div>
                  <span
                    style={{
                      fontSize: '0.75rem',
                      padding: '0.2rem 0.6rem',
                      borderRadius: '1rem',
                      background: isComplete ? '#16a34a20' : '#f59e0b20',
                      color: isComplete ? '#22c55e' : '#f59e0b',
                      fontWeight: 600,
                    }}
                  >
                    {isComplete ? 'Complete' : 'In Progress'}
                  </span>
                </button>

                {/* Cycle Body */}
                {isExpanded && (
                  <div style={{ padding: '0.75rem 1rem' }}>
                    {/* Extractions */}
                    <div style={{ marginBottom: '0.75rem' }}>
                      <div
                        style={{
                          fontSize: '0.75rem',
                          fontWeight: 600,
                          color: '#f59e0b',
                          marginBottom: '0.5rem',
                          textTransform: 'uppercase',
                          letterSpacing: '0.05em',
                        }}
                      >
                        Extraction ({cycle.extractions.length})
                      </div>
                      {cycle.extractions.map((entry) => (
                        <EntryCard
                          key={entry.id}
                          entry={entry}
                          isExpanded={expandedEntries.has(entry.id)}
                          onToggle={() => toggleEntry(entry.id)}
                        />
                      ))}
                    </div>

                    {/* Consolidation */}
                    {cycle.consolidation && (
                      <div>
                        <div
                          style={{
                            fontSize: '0.75rem',
                            fontWeight: 600,
                            color: '#3b82f6',
                            marginBottom: '0.5rem',
                            textTransform: 'uppercase',
                            letterSpacing: '0.05em',
                          }}
                        >
                          Consolidation
                        </div>
                        <EntryCard
                          entry={cycle.consolidation}
                          isExpanded={expandedEntries.has(cycle.consolidation.id)}
                          onToggle={() => toggleEntry(cycle.consolidation!.id)}
                        />
                      </div>
                    )}
                  </div>
                )}
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}

function EntryCard({
  entry,
  isExpanded,
  onToggle,
}: {
  entry: DreamJournalEntry
  isExpanded: boolean
  onToggle: () => void
}) {
  return (
    <div
      style={{
        borderRadius: '0.5rem',
        border: '1px solid var(--color-border, #2a2a3e)',
        marginBottom: '0.5rem',
        overflow: 'hidden',
      }}
    >
      <button
        onClick={onToggle}
        style={{
          width: '100%',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          padding: '0.5rem 0.75rem',
          background: 'transparent',
          border: 'none',
          cursor: 'pointer',
          color: 'inherit',
          textAlign: 'left',
          fontSize: '0.875rem',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
          <span style={{ fontSize: '0.7rem', color: '#6b7280' }}>
            {isExpanded ? '▼' : '▶'}
          </span>
          <span style={{ fontWeight: 500 }}>
            {entry.session_context || entry.stage}
          </span>
          {entry.duration_ms != null && (
            <span style={{ fontSize: '0.75rem', color: '#6b7280' }}>
              {(entry.duration_ms / 1000).toFixed(1)}s
            </span>
          )}
        </div>
        <span style={{ fontSize: '0.75rem', color: '#6b7280' }}>
          {new Date(entry.timestamp).toLocaleTimeString()}
        </span>
      </button>
      {isExpanded && (
        <div
          style={{
            padding: '0.75rem',
            borderTop: '1px solid var(--color-border, #2a2a3e)',
            fontSize: '0.85rem',
            lineHeight: 1.6,
            whiteSpace: 'pre-wrap',
            color: 'var(--color-text-secondary, #d1d5db)',
          }}
        >
          {entry.content}
        </div>
      )}
    </div>
  )
}
