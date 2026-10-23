import type { AffectedAgent } from '../../interfaces/types'
import { cardStyle, primaryButton, secondaryButton } from './styles'

const FEATURE_LABELS: Record<string, string> = {
  chat: 'chat',
  stt: 'speech-to-text',
  tts: 'text-to-speech',
  image_gen: 'image generation',
  embedding: 'embeddings',
}

/** Lists the agents that use a provider before it is removed (FR-010b). */
export default function DeleteWarning({
  name,
  agents,
  onConfirm,
  onCancel,
  busy,
}: {
  name: string
  agents: AffectedAgent[]
  onConfirm: () => void
  onCancel: () => void
  busy?: boolean
}) {
  return (
    <div
      role="dialog"
      aria-modal
      style={{ position: 'fixed', inset: 0, zIndex: 1100, background: 'rgba(0,0,0,0.4)', display: 'flex', alignItems: 'center', justifyContent: 'center', padding: '16px' }}
      onClick={onCancel}
    >
      <div style={{ ...cardStyle, maxWidth: '28rem', width: '100%' }} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ marginTop: 0, fontSize: '1rem' }}>Remove {name}?</h3>
        {agents.length ? (
          <>
            <p style={{ fontSize: '0.85rem', color: 'var(--text-secondary)' }}>
              These agents use it and will stop working until they're moved to another provider:
            </p>
            <ul style={{ fontSize: '0.85rem', paddingLeft: '1.2rem' }}>
              {agents.map((a) => (
                <li key={a.id}>
                  <strong>{a.name}</strong> <span style={{ color: 'var(--text-tertiary)' }}>({a.features.map((f) => FEATURE_LABELS[f] ?? f).join(', ')})</span>
                </li>
              ))}
            </ul>
          </>
        ) : (
          <p style={{ fontSize: '0.85rem', color: 'var(--text-secondary)' }}>No agent uses it.</p>
        )}
        <div style={{ display: 'flex', gap: '0.5rem', justifyContent: 'flex-end' }}>
          <button type="button" style={secondaryButton} onClick={onCancel}>
            Cancel
          </button>
          <button type="button" style={{ ...primaryButton(busy), background: busy ? 'var(--border)' : 'var(--error, #ef4444)' }} disabled={busy} onClick={onConfirm}>
            {busy ? 'Removing…' : 'Remove'}
          </button>
        </div>
      </div>
    </div>
  )
}
