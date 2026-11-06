import { Composer } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { ApiError } from '../../api/client'
import { getCommands, type CommandsMenu, type WorkspaceMember } from '../../api/endpoints/workspaces'
import { insertMention } from '../../lib/mentions'
import { mentionSuggestion, slashItems } from '../../lib/workspaceCompose'
import { toast } from '../../stores/toasts'

type Props = {
    workspaceId: string
    topicName: string
    me: string
    members: WorkspaceMember[]
    /** Posts the text and resolves when it is in. */
    onPost: (text: string) => Promise<void>
}

/**
 * Where a person writes in a topic. `/` at the start lists commands and every member agent's
 * skills; `@` offers agents and people; after a command, `@` offers only the agents that can take
 * it. Files are not offered here: a Workspace topic does not take attachments yet.
 */
export function WorkspaceComposer({ workspaceId, topicName, me, members, onPost }: Props) {
    const [value, setValue] = useState('')
    const [menu, setMenu] = useState<CommandsMenu | null>(null)
    // Escape puts the menu away until the text changes.
    const [dismissedAt, setDismissedAt] = useState<string | null>(null)
    const [sending, setSending] = useState(false)

    useEffect(() => {
        let live = true
        getCommands(workspaceId)
            .then((m) => live && setMenu(m))
            .catch(() => live && setMenu(null))
        return () => {
            live = false
        }
        // Members changing can change whose skills are on offer.
    }, [workspaceId, members.length])

    const suggestion = useMemo(() => mentionSuggestion(value, members, menu, me), [value, members, menu, me])

    const send = async () => {
        const text = value.trim()
        if (!text || sending) return
        setSending(true)
        try {
            await onPost(text)
            setValue('')
        } catch (e) {
            // A refused command (/btw, /clear-files, /lobotomy) is explained in place, and the text is kept.
            toast.error(e instanceof ApiError ? e.message : "Couldn't send that")
        } finally {
            setSending(false)
        }
    }

    return (
        <Composer
            agentName={topicName}
            placeholder={`Message #${topicName}: @ to mention, / for commands`}
            value={value}
            onValueChange={setValue}
            onSend={() => void send()}
            skills={slashItems(menu)}
            suggest={
                suggestion && dismissedAt !== value
                    ? {
                          label: 'Agents and people',
                          query: suggestion.query,
                          items: suggestion.items,
                          emptyText: suggestion.emptyText,
                          onPick: (item) => {
                              if ('id' in item) setValue(insertMention(value, suggestion.start, item.id))
                          },
                          onDismiss: () => setDismissedAt(value),
                      }
                    : null
            }
            mic={false}
        />
    )
}
