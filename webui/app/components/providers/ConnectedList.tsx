import { useEffect, useRef, useState } from 'react'
import { FaEllipsis } from 'react-icons/fa6'
import ProviderLogo from '../ProviderLogo'
import type { CatalogueProvider, CustomProviderResponse, ProviderResponse, UsedBy } from '../../interfaces/types'
import { badgeStyle, cardStyle, secondaryButton } from './styles'

/** A connected provider or a custom one, as one row. */
export type ConnectedRow =
  | { kind: 'provider'; provider: ProviderResponse; catalogue?: CatalogueProvider }
  | { kind: 'custom'; custom: CustomProviderResponse; catalogue?: CatalogueProvider }

interface ConnectedListProps {
  rows: ConnectedRow[]
  onViewModels: (row: ConnectedRow) => void
  onEdit: (row: ConnectedRow) => void
  onDuplicate: (row: ConnectedRow) => void
  onDelete: (row: ConnectedRow) => void
  onRefetch: (row: ConnectedRow) => void
}

const USED_LABELS: [keyof UsedBy, string][] = [
  ['chat', 'chat'],
  ['stt', 'STT'],
  ['tts', 'TTS'],
  ['image_gen', 'image gen'],
  ['embedding', 'embeddings'],
]

export function usedByText(used: UsedBy): string {
  const parts = USED_LABELS.filter(([k]) => used[k] > 0).map(([k, label]) =>
    k === 'chat' ? `chat (${used[k]} agent${used[k] === 1 ? '' : 's'})` : `${label} (${used[k]})`
  )
  return parts.length ? `Used by: ${parts.join(' · ')}` : 'Not used by any agent'
}

function sourceBadge(row: ConnectedRow) {
  if (row.kind === 'custom') return <span style={badgeStyle()}>custom</span>
  const p = row.provider
  return (
    <>
      {p.source === 'saved' && <span style={badgeStyle('ok')}>Saved key</span>}
      {p.source?.startsWith('env:') && <span style={badgeStyle('ok')}>From env {p.source.slice(4)}</span>}
      {p.source === null && <span style={badgeStyle()}>default address</span>}
      {p.migrated_from && <span style={badgeStyle()}>migrated from {p.migrated_from}</span>}
    </>
  )
}

function Menu({ items }: { items: [string, () => void][] }) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const close = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [])
  return (
    <div ref={ref} style={{ position: 'relative' }}>
      <button type="button" aria-label="More actions" style={{ ...secondaryButton, padding: '0.35rem 0.55rem' }} onClick={() => setOpen((o) => !o)}>
        <FaEllipsis size={12} />
      </button>
      {open && (
        <div style={{ position: 'absolute', right: 0, top: 'calc(100% + 4px)', zIndex: 20, ...cardStyle, padding: '0.25rem', minWidth: '9rem', boxShadow: '0 8px 24px rgba(0,0,0,0.15)' }}>
          {items.map(([label, run]) => (
            <button
              key={label}
              type="button"
              onClick={() => {
                setOpen(false)
                run()
              }}
              style={{
                display: 'block',
                width: '100%',
                textAlign: 'left',
                padding: '0.4rem 0.6rem',
                fontSize: '0.8rem',
                background: 'none',
                border: 'none',
                borderRadius: '0.25rem',
                color: label === 'Delete' ? 'var(--error, #ef4444)' : 'var(--text-primary)',
                cursor: 'pointer',
              }}
              onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
              onMouseLeave={(e) => (e.currentTarget.style.background = 'none')}
            >
              {label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

/** The providers vizier can use now: saved keys, keys in the environment, custom providers. */
export default function ConnectedList({ rows, onViewModels, onEdit, onDuplicate, onDelete, onRefetch }: ConnectedListProps) {
  if (!rows.length) {
    return <div style={{ ...cardStyle, fontSize: '0.85rem', color: 'var(--text-secondary)' }}>No provider is connected yet. Add one below.</div>
  }
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
      {rows.map((row) => {
        const id = row.kind === 'custom' ? row.custom.id : row.provider.id
        const name = row.kind === 'custom' ? row.custom.name : row.provider.name
        const modelCount = row.kind === 'custom' ? row.custom.models.length : row.provider.model_count
        const used = row.kind === 'custom' ? row.custom.used_by : row.provider.used_by
        const reachable = row.kind === 'custom' ? row.custom.reachable : row.provider.reachable
        const showReach = row.kind === 'custom' || row.provider.kind === 'local'
        const canDelete = row.kind === 'custom' || row.provider.source === 'saved'
        const menu: [string, () => void][] = []
        if (row.kind === 'custom') menu.push(['Re-fetch models', () => onRefetch(row)])
        menu.push(['Duplicate', () => onDuplicate(row)])
        if (canDelete) menu.push(['Delete', () => onDelete(row)])
        return (
          <div key={`${row.kind}:${id}`} style={{ ...cardStyle, display: 'flex', alignItems: 'center', gap: '0.75rem', flexWrap: 'wrap', padding: '0.75rem' }}>
            <ProviderLogo id={id} name={name} src={row.catalogue?.logo} fullColor={row.catalogue?.logo_full_color} size={28} />
            <div style={{ flex: '1 1 14rem', minWidth: 0 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '0.4rem', flexWrap: 'wrap' }}>
                <strong style={{ fontSize: '0.9rem' }}>{name}</strong>
                <span style={{ fontSize: '0.72rem', color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)' }}>{id}</span>
                {sourceBadge(row)}
                {showReach && reachable !== null && (
                  <span style={badgeStyle(reachable ? 'ok' : 'error')}>{reachable ? 'reachable' : 'unreachable'}</span>
                )}
              </div>
              <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', marginTop: '0.15rem' }}>
                {modelCount ? `${modelCount} models · ` : ''}
                {usedByText(used)}
              </div>
            </div>
            <div style={{ display: 'flex', gap: '0.4rem' }}>
              <button type="button" style={{ ...secondaryButton, padding: '0.35rem 0.7rem', fontSize: '0.8rem' }} onClick={() => onViewModels(row)}>
                View models
              </button>
              <button type="button" style={{ ...secondaryButton, padding: '0.35rem 0.7rem', fontSize: '0.8rem' }} onClick={() => onEdit(row)}>
                Edit
              </button>
              <Menu items={menu} />
            </div>
          </div>
        )
      })}
    </div>
  )
}
