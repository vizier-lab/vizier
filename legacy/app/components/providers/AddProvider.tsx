import { useState } from 'react'
import ProviderLogo from '../ProviderLogo'
import type { CatalogueProvider } from '../../interfaces/types'
import { badgeStyle, cardStyle, inputStyle, primaryButton } from './styles'

type Filter = 'all' | 'chat' | 'media' | 'local'

/** Media and embedding providers vizier's TTS/STT/image/embedding features use. */
const MEDIA = new Set([
  'openai', 'google', 'togetherai', 'github-copilot', 'groq', 'mistral', 'cohere',
  'openrouter', 'huggingface', 'xai', 'elevenlabs', 'voyageai', 'hyperbolic', 'ollama',
])

interface AddProviderProps {
  /** Providers not connected yet. */
  providers: CatalogueProvider[]
  onConnect: (provider: CatalogueProvider) => void
  /** Open the custom provider form, with a name when the search found nothing. */
  onCustom: (name?: string) => void
}

/** Search the catalogue for a provider to connect, or start a custom one. */
export default function AddProvider({ providers, onConnect, onCustom }: AddProviderProps) {
  const [q, setQ] = useState('')
  const [filter, setFilter] = useState<Filter>('all')
  const query = q.trim().toLowerCase()
  const shown = providers
    .filter((p) => p.origin !== 'custom' && p.id !== 'dummyplug')
    .filter((p) => {
      if (filter === 'chat') return p.dialect !== null
      if (filter === 'media') return p.kind === 'media' || MEDIA.has(p.id)
      if (filter === 'local') return p.kind === 'local'
      return true
    })
    .filter((p) => !query || p.name.toLowerCase().includes(query) || p.id.includes(query))
    .sort((a, b) => Number(b.popular) - Number(a.popular) || a.name.localeCompare(b.name))
  const popularCount = shown.filter((p) => p.popular).length

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem' }}>
      <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap', alignItems: 'center' }}>
        <input style={{ ...inputStyle, flex: '1 1 14rem' }} placeholder={`Search ${providers.length} providers`} value={q} onChange={(e) => setQ(e.target.value)} />
        <div style={{ display: 'flex', gap: '0.25rem' }}>
          {(['all', 'chat', 'media', 'local'] as Filter[]).map((f) => (
            <button
              key={f}
              type="button"
              onClick={() => setFilter(f)}
              style={{ ...badgeStyle(), cursor: 'pointer', padding: '0.25rem 0.65rem', background: filter === f ? 'var(--hover)' : 'transparent', color: 'var(--text-primary)' }}
            >
              {f === 'all' ? 'All' : f === 'chat' ? 'Chat' : f === 'media' ? 'Media & embeddings' : 'Local'}
            </button>
          ))}
        </div>
        <button type="button" style={primaryButton()} onClick={() => onCustom()}>
          + Custom provider
        </button>
      </div>

      <div style={{ ...cardStyle, padding: 0, maxHeight: '26rem', overflowY: 'auto' }}>
        {shown.map((p, i) => (
          <div key={p.id}>
            {i === 0 && popularCount > 0 && <div style={groupLabel}>POPULAR</div>}
            {i === popularCount && popularCount > 0 && <div style={groupLabel}>A–Z</div>}
            <div
              onClick={() => onConnect(p)}
              style={{ display: 'flex', alignItems: 'center', gap: '0.6rem', padding: '0.5rem 0.75rem', cursor: 'pointer', borderTop: '1px solid var(--border)' }}
              onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
              onMouseLeave={(e) => (e.currentTarget.style.background = '')}
            >
              <ProviderLogo id={p.id} name={p.name} src={p.logo} fullColor={p.logo_full_color} size={20} />
              <span style={{ flex: 1, fontSize: '0.85rem' }}>{p.name}</span>
              <span style={{ fontSize: '0.72rem', color: 'var(--text-tertiary)' }}>
                {p.model_count ? `${p.model_count} models` : p.kind}
              </span>
              <span style={{ fontSize: '0.75rem', color: 'var(--accent-primary)' }}>Connect</span>
            </div>
          </div>
        ))}
        {!shown.length && (
          <div style={{ padding: '1rem', fontSize: '0.85rem', color: 'var(--text-secondary)' }}>
            No provider matches “{q}”.{' '}
            <button type="button" className="btn btn-ghost" style={{ fontSize: '0.85rem', color: 'var(--accent-primary)' }} onClick={() => onCustom(q.trim())}>
              Create custom provider “{q.trim()}”
            </button>
          </div>
        )}
      </div>
    </div>
  )
}

const groupLabel: React.CSSProperties = {
  padding: '0.45rem 0.75rem 0.25rem',
  fontSize: '0.68rem',
  fontWeight: 600,
  letterSpacing: '0.04em',
  color: 'var(--text-tertiary)',
}
