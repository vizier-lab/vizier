import { Button, Notice, TextField } from '@vizier/design'
import { useState } from 'react'

/** A new role's name: blank, or the duplicated role's with `-copy`. Its permissions come next. */
export function RoleNameForm({ initial, onCreate, onCancel }: { initial: string; onCreate: (name: string) => Promise<void>; onCancel: () => void }) {
    const [name, setName] = useState(initial)
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()
    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                setBusy(true)
                setError(undefined)
                try {
                    await onCreate(name.trim())
                } catch (err) {
                    setError(err instanceof Error ? err.message : 'The role could not be made.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            {error && <Notice tone="failed" title="The role wasn’t made">{error}</Notice>}
            <TextField label="Name" mono value={name} autoFocus helper="Choose its permissions next." onChange={(e) => setName(e.target.value)} />
            <div className="flex justify-end gap-8">
                <Button type="button" variant="ghost" onClick={onCancel}>Cancel</Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!name.trim()}>Create role</Button>
            </div>
        </form>
    )
}
