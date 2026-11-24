import { AddReaction, EmojiPicker, Reaction } from '@vizier/design'
import { useMemo } from 'react'
import type { ReactionEntry } from '../../api/frames'
import { Pop } from '../../lib/motion'
import { useSession } from '../../stores/session'

type Props = {
    reactions: ReactionEntry[]
    onToggle: (emoji: string, action: 'added' | 'removed') => void
    /** A reply without a server uid can't be reacted to. */
    disabled?: boolean
}

/** The reactions on a reply, with an add button that opens the emoji picker. */
export function Reactions({ reactions, onToggle, disabled }: Props) {
    // The server records the reactor as the login name (the session's channel owner), not the user id.
    const me = useSession((s) => s.user?.username)
    const groups = useMemo(() => {
        const by = new Map<string, { count: number; mine: boolean }>()
        for (const r of reactions) {
            const g = by.get(r.emoji) ?? { count: 0, mine: false }
            g.count += 1
            if (r.user_id === me) g.mine = true
            by.set(r.emoji, g)
        }
        return [...by.entries()]
    }, [reactions, me])

    return (
        <>
            {groups.map(([emoji, g]) => (
                <Pop key={emoji}>
                    <Reaction emoji={emoji} count={g.count} mine={g.mine} disabled={disabled} onClick={() => onToggle(emoji, g.mine ? 'removed' : 'added')} />
                </Pop>
            ))}
            {!disabled && (
                <EmojiPicker
                    recent={groups.map(([e]) => e)}
                    trigger={<AddReaction />}
                    onSelect={(emoji) => onToggle(emoji, 'added')}
                />
            )}
        </>
    )
}
