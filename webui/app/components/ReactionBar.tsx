import { useState, useCallback, memo } from 'react'
import { EmojiPickerPopup } from './EmojiPickerPopup'

const QUICK_REACTIONS = ['👍', '❤️', '😂', '🔥', '👀']

interface ReactionBarProps {
  onReact: (emoji: string) => void
}

function ReactionBarComponent({ onReact }: ReactionBarProps) {
  const [showPicker, setShowPicker] = useState(false)

  return (
    <div
      style={{
        position: 'absolute',
        top: '-12px',
        right: '8px',
        display: 'flex',
        alignItems: 'center',
        gap: '2px',
        background: 'var(--surface)',
        border: '1px solid var(--border)',
        borderRadius: '6px',
        padding: '2px',
        boxShadow: 'var(--shadow-sm)',
        zIndex: 50,
      }}
    >
      {QUICK_REACTIONS.map((emoji) => (
        <button
          key={emoji}
          onClick={() => onReact(emoji)}
          style={{
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            padding: '4px 6px',
            borderRadius: '4px',
            fontSize: '16px',
            lineHeight: 1,
            transition: 'background 0.15s',
          }}
          onMouseEnter={(e) => {
            (e.target as HTMLElement).style.background = 'var(--border)'
          }}
          onMouseLeave={(e) => {
            (e.target as HTMLElement).style.background = 'none'
          }}
        >
          {emoji}
        </button>
      ))}
      <div style={{ position: 'relative' }}>
        <button
          onClick={() => setShowPicker(!showPicker)}
          style={{
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            padding: '4px 6px',
            borderRadius: '4px',
            fontSize: '14px',
            lineHeight: 1,
            color: 'var(--text-secondary)',
            transition: 'background 0.15s',
          }}
          onMouseEnter={(e) => {
            (e.target as HTMLElement).style.background = 'var(--border)'
          }}
          onMouseLeave={(e) => {
            (e.target as HTMLElement).style.background = 'none'
          }}
        >
          +
        </button>
        {showPicker && (
          <EmojiPickerPopup
            onSelect={(emoji) => {
              onReact(emoji)
              setShowPicker(false)
            }}
            onClose={() => setShowPicker(false)}
          />
        )}
      </div>
    </div>
  )
}

export const ReactionBar = memo(ReactionBarComponent)
