import { useEffect, useState } from 'react'
import { Button, Dialog, IconButton, TextField } from '@vizier/design'
import type { SensitiveAsk } from '../../api/frames'

export type { SensitiveAsk } from '../../api/frames'

/**
 * The masked dialog for one sensitive value (FR-035–FR-038, FR-043).
 *
 * The field starts masked, reveals only on an explicit press, never autocompletes and never
 * offers itself to a password manager. Dismissing the dialog — reject, Escape, an outside press or
 * the close control — declines the ask rather than leaving the script waiting (FR-036), which
 * `Dialog` already gives by closing on all three.
 *
 * The value is not kept after submit: the dialog reports it once and unmounts (FR-035).
 */
export function SensitiveAskDialog({
    ask,
    onAnswer,
    onClosed,
}: {
    ask: SensitiveAsk | null
    onAnswer: (askId: string, value: string | null) => void
    onClosed?: () => void
}) {
    const [value, setValue] = useState('')
    const [revealed, setRevealed] = useState(false)
    const [error, setError] = useState<string | null>(null)

    useEffect(() => {
        // A new ask starts empty, masked, and focused: the person's next keypress is the value.
        setValue('')
        setRevealed(false)
        setError(null)
        // Focus lands in the field so the person's next keypress is the value.
        document.getElementById('sensitive-ask-value')?.focus()
    }, [ask?.ask_id])

    if (!ask) return null
    const current = ask

    const tooShort = value.length > 0 && value.length < current.min_bytes
    const tooLong = value.length > current.max_bytes

    function submit() {
        if (value.trim().length === 0) {
            setError('A value is required. Dismiss the dialog to decline.')
            return
        }
        if (tooLong) {
            setError(`The value is ${value.length} bytes; the limit is ${current.max_bytes}.`)
            return
        }
        // Never trimmed: the value is what the person typed (FR-037).
        onAnswer(current.ask_id, value)
        // And not kept: the field is empty before the dialog has even closed (FR-035).
        setValue('')
        setRevealed(false)
    }

    return (
        <Dialog
            open
            title={current.label ?? 'A value is needed'}
            onCancel={() => {
                // Every way of closing declines, so nothing is left waiting (FR-036).
                onAnswer(current.ask_id, null)
                onClosed?.()
            }}
            confirmLabel=""
            onConfirm={submit}
        >
            <div className="flex flex-col gap-12">
                <p className="type-ui text-soft">{current.purpose}</p>

                <TextField
                    type={revealed ? 'text' : 'password'}
                    id="sensitive-ask-value"
                    value={value}
                    autoComplete="off"
                    data-1p-ignore
                    data-lpignore="true"
                    onChange={(event) => setValue(event.target.value)}
                    aria-describedby={`${current.ask_id}-help`}
                    // The reveal lives inside the field, as an eye: the value stays masked until it is pressed.
                    suffix={
                        <IconButton
                            variant="ghost"
                            size="sm"
                            icon={revealed ? 'eye-closed' : 'eye'}
                            aria-label={revealed ? 'Hide value' : 'Show value'}
                            onClick={() => setRevealed((shown) => !shown)}
                        />
                    }
                />

                <span id={`${current.ask_id}-help`} className="type-ui text-faint">
                    {current.min_bytes}–{current.max_bytes} bytes. The value goes to this run only: it is never
                    written to the conversation.
                </span>

                {!current.protected || tooShort ? (
                    <p className="type-ui text-danger" role="alert">
                        {`A value shorter than ${current.min_bytes} bytes cannot be scrubbed from the run's report.`}
                    </p>
                ) : null}
                {error ? (
                    <p className="type-ui text-danger" role="alert">
                        {error}
                    </p>
                ) : null}

                <div className="flex items-center gap-8">
                    <Button variant="primary" onClick={submit}>
                        Send
                    </Button>
                    <Button
                        variant="secondary"
                        onClick={() => {
                            onAnswer(current.ask_id, null)
                            onClosed?.()
                        }}
                    >
                        Reject
                    </Button>
                </div>
            </div>
        </Dialog>
    )
}
