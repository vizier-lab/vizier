import { Attachment, Composer, DropZone, Icon } from '@vizier/design'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { ApiError } from '../../api/client'
import { getCommands, type CommandsMenu, type WorkspaceMember } from '../../api/endpoints/workspaces'
import { insertMention } from '../../lib/mentions'
import { fileSize, MAX_UPLOAD_BYTES, uploadFileId } from '../../lib/upload'
import { mentionSuggestion, slashItems } from '../../lib/workspaceCompose'
import { toast } from '../../stores/toasts'

type Props = {
    workspaceId: string
    topicName: string
    me: string
    members: WorkspaceMember[]
    /** Posts the text with the uploaded files' ids and resolves when it is in. */
    onPost: (text: string, files: string[]) => Promise<void>
}

type Pending = { id: number; file: File; state: 'ready' | 'failed'; reason?: string }

let pendingId = 1

/**
 * Where a person writes in a topic. `/` at the start lists commands and every member agent's
 * skills; `@` offers agents and people; after a command, `@` offers only the agents that can take
 * it. Files attach, drop and paste as in a direct chat, and reach every member agent.
 */
export function WorkspaceComposer({ workspaceId, topicName, me, members, onPost }: Props) {
    const [value, setValue] = useState('')
    const [menu, setMenu] = useState<CommandsMenu | null>(null)
    // Escape puts the menu away until the text changes.
    const [dismissedAt, setDismissedAt] = useState<string | null>(null)
    const [sending, setSending] = useState(false)
    const [pending, setPending] = useState<Pending[]>([])
    const [dragging, setDragging] = useState(false)
    const picker = useRef<HTMLInputElement>(null)

    const add = useCallback((files: File[]) => {
        setPending((p) => [
            ...p,
            ...files.map((file): Pending =>
                file.size > MAX_UPLOAD_BYTES
                    ? { id: pendingId++, file, state: 'failed', reason: 'over 25 MB' }
                    : { id: pendingId++, file, state: 'ready' }
            ),
        ])
    }, [])

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
        const ready = pending.filter((p) => p.state === 'ready')
        if ((!text && ready.length === 0) || sending) return
        setSending(true)
        try {
            const files = await Promise.all(ready.map((p) => uploadFileId(p.file)))
            await onPost(text, files)
            setValue('')
            setPending([])
        } catch (e) {
            // A refused command (/btw, /clear-files, /lobotomy) is explained in place, and the text is kept.
            toast.error(e instanceof ApiError ? e.message : "Couldn't send that")
        } finally {
            setSending(false)
        }
    }

    return (
        <div
            className="relative"
            onDragEnter={(e) => {
                if (e.dataTransfer.types.includes('Files')) setDragging(true)
            }}
            onDragLeave={(e) => {
                if (!e.currentTarget.contains(e.relatedTarget as Node)) setDragging(false)
            }}
            onDragOver={(e) => e.preventDefault()}
            onDrop={() => setDragging(false)}
            onPaste={(e) => {
                const files = Array.from(e.clipboardData.files)
                if (files.length) add(files)
            }}
        >
            <Composer
                agentName={topicName}
                placeholder={`Message # ${topicName}. Agents reply only when mentioned.`}
                value={value}
                onValueChange={setValue}
                onSend={() => void send()}
                onAttach={() => picker.current?.click()}
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
                overlay={
                    <DropZone
                        look="overlay"
                        active={dragging}
                        hint="Files, images and audio"
                        onFiles={(files) => {
                            setDragging(false)
                            add(files)
                        }}
                    />
                }
                attachments={
                    pending.length > 0 ? (
                        <>
                            {pending.map((p) => (
                                <Attachment
                                    key={p.id}
                                    name={p.file.name}
                                    state={p.state}
                                    detail={p.reason ?? fileSize(p.file.size)}
                                    onRemove={() => setPending((all) => all.filter((x) => x.id !== p.id))}
                                />
                            ))}
                        </>
                    ) : undefined
                }
                afterTools={
                    <button
                        type="button"
                        aria-label="Mention someone"
                        onClick={() => setValue((v) => (v && !/\s$/.test(v) ? `${v} @` : `${v}@`))}
                        className="flex size-28 items-center justify-center rounded-sm text-muted hover:bg-raised hover:text-fg"
                    >
                        <Icon name="at-sign" />
                    </button>
                }
            />
            <input
                ref={picker}
                type="file"
                multiple
                hidden
                onChange={(e) => {
                    add(Array.from(e.target.files ?? []))
                    e.target.value = ''
                }}
            />
        </div>
    )
}
