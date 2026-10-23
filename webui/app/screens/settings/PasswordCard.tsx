import { Button, Card, Notice, TextField } from '@vizier/design'
import { useState } from 'react'
import { changePassword } from '../../api/endpoints/auth'
import { toast } from '../../stores/toasts'

const MIN = 8

/** Change the password you sign in with. */
export function PasswordCard() {
    const [current, setCurrent] = useState('')
    const [next, setNext] = useState('')
    const [confirm, setConfirm] = useState('')
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()

    const short = next !== '' && next.length < MIN
    const mismatch = confirm !== '' && confirm !== next
    const ready = current !== '' && next.length >= MIN && confirm === next

    return (
        <Card title="Password" description="Change the password you sign in with.">
            <form
                className="flex flex-col gap-16"
                onSubmit={async (e) => {
                    e.preventDefault()
                    if (!ready) return
                    setBusy(true)
                    setError(undefined)
                    try {
                        await changePassword(current, next)
                        setCurrent('')
                        setNext('')
                        setConfirm('')
                        toast.success('Password changed')
                    } catch (err) {
                        setError(err instanceof Error ? err.message : 'The password could not be changed.')
                    } finally {
                        setBusy(false)
                    }
                }}
            >
                {error && <Notice tone="failed" title="The password wasn’t changed">{error}</Notice>}
                <div className="grid grid-cols-1 gap-16 sm:grid-cols-3">
                    <TextField label="Current password" type="password" autoComplete="current-password" value={current} onChange={(e) => setCurrent(e.target.value)} />
                    <TextField label="New password" type="password" autoComplete="new-password" value={next} invalid={short} helper={short ? `At least ${MIN} characters.` : undefined} onChange={(e) => setNext(e.target.value)} />
                    <TextField label="Confirm new password" type="password" autoComplete="new-password" value={confirm} invalid={mismatch} helper={mismatch ? 'Passwords do not match.' : undefined} onChange={(e) => setConfirm(e.target.value)} />
                </div>
                <div>
                    <Button type="submit" icon="save" loading={busy} disabled={!ready}>
                        Change password
                    </Button>
                </div>
            </form>
        </Card>
    )
}
