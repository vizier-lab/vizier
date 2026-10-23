import { memo, useMemo } from 'react'
import { FaStop } from 'react-icons/fa6'
import ActivityTrail from './ActivityTrail'
import type { TrailEvent } from '../interfaces/types'

const THINKING_WORDS = [
  'thinking',
  'pondering',
  'reasoning',
  'considering',
  'processing',
  'analyzing',
  'contemplating',
  'deliberating',
  'reflecting',
  'musing',
  'figuring out',
  'working on it',
  'brewing ideas',
  'cooking up a response',
  'consulting the archives',
  'doing some brain yoga',
  'fetching wisdom',
  'gathering thoughts',
  'having a think',
  'let me think',
  'loading brain cells',
  'mapping it out',
  'one moment',
  'putting pieces together',
  'reading between the lines',
  'rewiring neurons',
  'rubbing my chin',
  'searching the void',
  'stirring the pot',
  'taking a moment',
  'thinking really hard',
  'tracing the logic',
  'unpacking this',
  'warming up the brain',
  'weighing options',
  'wrestling with ideas',
  'consulting the grand vizier',
  'deciphering royal scrolls',
  'distilling ancient wisdom',
  'drawing from the treasury of knowledge',
  'entering the oracle chamber',
  'gazing into the crystal ball',
  'heeding the inner counsel',
  'inverting the pyramid of thought',
  'journeying through knowledge',
  'learning from the sages',
  'peering through the astrolabe',
  'piloting the ship of wisdom',
  'pondering the riddle',
  'seeking the council of elders',
  'sharpening the vizier\'s blade',
  'sorting the palace archives',
  'unraveling the mystery',
  'weighing the scrolls',
]

interface ThinkingIndicatorProps {
  isVisible: boolean
  // The turn's trail as it streams. Rendered through the same component that renders a
  // reloaded turn's, so a turn does not change appearance on refresh.
  trail: TrailEvent[]
  agentName: string
  toolLabel: (name: string, args: Record<string, unknown>) => string
  onAbort?: () => void
}

function ThinkingIndicatorComponent({
  isVisible,
  trail,
  agentName,
  toolLabel,
  onAbort,
}: ThinkingIndicatorProps) {
  const thinkingWord = useMemo(
    () => THINKING_WORDS[Math.floor(Math.random() * THINKING_WORDS.length)],
    []
  )

  if (!isVisible) {
    return null
  }

  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
    }}>
      <div style={{
        fontWeight: '600',
        fontSize: '14px',
        color: 'var(--accent-primary)',
      }}>
        {agentName}
      </div>
      <div style={{
        padding: '12px 16px',
        borderRadius: '8px',
        // Green while the turn is still running, so a glance says "working". Once the
        // answer lands the same trail is re-rendered from `ActivityTrail`'s own chrome,
        // which is grey — the rule going cold is what marks the turn finished.
        borderLeft: '3px solid var(--accent-primary)',
        display: 'flex',
        flexDirection: 'column',
        color: 'var(--text-secondary)',
        background: 'transparent',
      }}>
        <div style={{
          display: 'flex',
          alignItems: 'center',
          color: 'var(--text-tertiary)',
        }}>
          {thinkingWord}
          <div className="thinking-dots">
            <span>.</span>
            <span>.</span>
            <span>.</span>
          </div>
        </div>
        <ActivityTrail trail={trail} live label={toolLabel} variant="bare" />
        {onAbort && (
          <button
            onClick={onAbort}
            className="thinking-abort-btn"
            title="Abort"
          >
            <FaStop size={10} />
            <span>Abort</span>
          </button>
        )}
      </div>
    </div>
  )
}

// Memoize component to prevent re-renders when parent re-renders
// Only re-render if the trail or agentName changes
export const ThinkingIndicator = memo(ThinkingIndicatorComponent)
