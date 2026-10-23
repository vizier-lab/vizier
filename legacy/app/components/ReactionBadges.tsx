import { memo, useMemo } from 'react'
import type { ReactionEntry } from '../interfaces/types'

interface ReactionBadgesProps {
  reactions: ReactionEntry[]
  currentUserId?: string
  // Sent and not yet confirmed: dimmed, and not clickable until the server answers.
  pendingEmojis?: string[]
  // Without it the badges are read-only.
  onToggleReaction?: (emoji: string) => void
}

interface BadgeData {
  emoji: string
  count: number
  hasReacted: boolean
}

function ReactionBadgesComponent({ reactions, currentUserId, pendingEmojis, onToggleReaction }: ReactionBadgesProps) {
  const badges = useMemo(() => {
    const emojiMap = new Map<string, BadgeData>()

    for (const entry of reactions) {
      const existing = emojiMap.get(entry.emoji)
      if (existing) {
        existing.count++
      } else {
        emojiMap.set(entry.emoji, {
          emoji: entry.emoji,
          count: 1,
          hasReacted: false,
        })
      }
    }

    if (currentUserId) {
      for (const entry of reactions) {
        if (entry.user_id === currentUserId) {
          const badge = emojiMap.get(entry.emoji)
          if (badge) {
            badge.hasReacted = true
          }
        }
      }
    }

    return Array.from(emojiMap.values())
      .filter((b) => b.count > 0)
      .sort((a, b) => b.count - a.count)
  }, [reactions, currentUserId])

  if (badges.length === 0) {
    return null
  }

  return (
    <div
      style={{
        display: 'flex',
        flexWrap: 'wrap',
        gap: '4px',
      }}
    >
      {badges.map((badge) => {
        const pending = pendingEmojis?.includes(badge.emoji) ?? false
        const interactive = onToggleReaction !== undefined && !pending
        const style = {
          display: 'inline-flex',
          alignItems: 'center',
          gap: '4px',
          padding: '2px 8px',
          borderRadius: '12px',
          border: `1px solid ${badge.hasReacted ? 'var(--accent-primary)' : 'var(--border)'}`,
          background: badge.hasReacted ? 'rgba(var(--accent-primary-rgb, 59, 130, 246), 0.1)' : 'var(--surface)',
          color: 'var(--text-primary)',
          cursor: interactive ? 'pointer' : 'default',
          fontSize: '12px',
          lineHeight: '18px',
          transition: 'all 0.15s',
          opacity: pending ? 0.5 : 1,
        } as const
        const body = (
          <>
            <span>{badge.emoji}</span>
            <span style={{ fontWeight: 500 }}>{badge.count}</span>
            {pending && <span>…</span>}
          </>
        )
        if (onToggleReaction === undefined) {
          return (
            <span key={badge.emoji} style={style}>
              {body}
            </span>
          )
        }
        return (
          <button
            key={badge.emoji}
            onClick={() => interactive && onToggleReaction(badge.emoji)}
            disabled={pending}
            style={style}
            title={pending ? 'Saving reaction…' : badge.hasReacted ? 'Remove reaction' : 'Add reaction'}
          >
            {body}
          </button>
        )
      })}
    </div>
  )
}

export const ReactionBadges = memo(ReactionBadgesComponent)
