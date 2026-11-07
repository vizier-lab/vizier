import { Button, Card } from '@vizier/design'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useBlocker, useNavigate, useParams, useSearchParams, Navigate } from 'react-router'
import { addVariant, duplicateVariant, overrideRole, removeVariant, resetRole, setKey } from '../lib/themes/derive'
import { distinctName, nameProblem, newCustomId } from '../lib/themes/names'
import { MAX_NAME_LENGTH, type Appearance, type Hex, type KeyColor, type Role, type Scheme, type Variant } from '../lib/themes/roles'
import { vizierVariant } from '../lib/themes/vizier'
import { AdvancedColors } from '../screens/appearance/AdvancedColors'
import { KeyColorsCard } from '../screens/appearance/KeyColorsCard'
import { SchemePreview } from '../screens/appearance/SchemePreview'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PageFrame } from '../screens/shared/PageFrame'
import { useAppearance } from '../stores/appearance'
import { toast } from '../stores/toasts'

const BACK = '/settings/appearance'

/** A new theme starts as a copy with a free name, and nothing marked as changed by hand. */
function copyOf(source: Scheme, takenNames: string[]): Scheme {
    const suffix = ' copy'
    return {
        id: newCustomId(),
        name: distinctName(`${source.name.slice(0, MAX_NAME_LENGTH - suffix.length).trimEnd()}${suffix}`, takenNames),
        kind: 'custom',
        ...(source.light && { light: duplicateVariant(source.light) }),
        ...(source.dark && { dark: duplicateVariant(source.dark) }),
    }
}

function Editor({ initial, isNew, showing }: { initial: Scheme; isNew: boolean; showing: Appearance }) {
    const navigate = useNavigate()
    const { presets, customs, saveCustom } = useAppearance()
    // Nothing here touches the store or the page until it is saved (FR-014).
    const [draft, setDraft] = useState(initial)
    const [appearance, setAppearance] = useState<Appearance>(initial[showing] ? showing : initial.light ? 'light' : 'dark')
    const [nameError, setNameError] = useState<string | null>(null)

    const dirty = useMemo(() => JSON.stringify(draft) !== JSON.stringify(initial), [draft, initial])
    const leaving = useRef(false)
    const blocker = useBlocker(() => dirty && !leaving.current)
    useEffect(() => {
        const warn = (e: BeforeUnloadEvent) => {
            if (dirty) e.preventDefault()
        }
        window.addEventListener('beforeunload', warn)
        return () => window.removeEventListener('beforeunload', warn)
    }, [dirty])

    const variant = draft[appearance]
    const change = (edit: (v: Variant) => Variant) => setDraft((d) => (d[appearance] ? { ...d, [appearance]: edit(d[appearance]) } : d))

    const save = () => {
        const problem = saveCustom(draft)
        if (problem) {
            // Anything but a name problem can't be fixed in this field, so it is said out loud.
            if (nameProblem(draft.name, [...presets, ...customs], draft.id)) setNameError(problem)
            else toast.error(problem)
            return
        }
        leaving.current = true
        navigate(BACK)
    }

    return (
        <PageFrame
            width="narrow"
            title={isNew ? 'New theme' : `Edit ${initial.name}`}
            description="Your current theme stays on until you save and select this one."
            back={{ href: BACK, label: 'Appearance' }}
            actions={
                <>
                    <Button variant="secondary" onClick={() => navigate(BACK)}>
                        Cancel
                    </Button>
                    <Button variant="primary" onClick={save}>
                        Save theme
                    </Button>
                </>
            }
        >
            <div className="flex flex-col gap-16">
                <KeyColorsCard
                    draft={draft}
                    nameError={nameError}
                    appearance={appearance}
                    onName={(name) => {
                        setNameError(null)
                        setDraft((d) => ({ ...d, name }))
                    }}
                    onAppearance={setAppearance}
                    onAddVariant={() => setDraft((d) => addVariant(d, appearance, vizierVariant(appearance).keys))}
                    onRemoveVariant={() => {
                        setDraft((d) => removeVariant(d, appearance))
                        setAppearance(appearance === 'light' ? 'dark' : 'light')
                    }}
                    onKey={(key: KeyColor, hex: Hex) => change((v) => setKey(v, key, hex))}
                />
                {variant && (
                    <>
                        <Card title="Every color" description="Each one follows the key colors above until you set it here.">
                            <AdvancedColors variant={variant} onRole={(role: Role, hex: Hex) => change((v) => overrideRole(v, role, hex))} onReset={(role: Role) => change((v) => resetRole(v, role))} />
                        </Card>
                        <Card title="Preview">
                            <SchemePreview variant={variant} appearance={appearance} />
                        </Card>
                    </>
                )}
            </div>
            <ConfirmDialog
                open={blocker.state === 'blocked'}
                title="Discard your changes?"
                message="This theme has changes that haven't been saved."
                confirmLabel="Discard"
                onConfirm={() => blocker.proceed?.()}
                onCancel={() => blocker.reset?.()}
            />
        </PageFrame>
    )
}

/** `new?from=<id>` starts a copy of any scheme; a custom scheme's id edits it. Presets can't be edited. */
export default function SettingsAppearanceEdit() {
    const { schemeId } = useParams()
    const [search] = useSearchParams()
    const { loaded, load, presets, customs, mode, deviceIsDark } = useAppearance()

    useEffect(() => void load(), [load])

    // Fixed when the editor opens: a change from another tab must not reset what is being typed.
    const [initial, setInitial] = useState<Scheme | null>(null)
    useEffect(() => {
        if (!loaded || initial) return
        const all = [...presets, ...customs]
        if (schemeId === 'new') {
            const source = all.find((s) => s.id === search.get('from'))
            if (source) setInitial(copyOf(source, all.map((s) => s.name)))
        } else {
            const own = customs.find((s) => s.id === schemeId)
            if (own) setInitial(own)
        }
    }, [loaded, initial, presets, customs, schemeId, search])

    if (!loaded) return null
    const known = schemeId === 'new' ? [...presets, ...customs].some((s) => s.id === search.get('from')) : customs.some((s) => s.id === schemeId)
    if (!known && !initial) return <Navigate to={BACK} replace />
    if (!initial) return null
    const showing: Appearance = mode === 'system' ? (deviceIsDark ? 'dark' : 'light') : mode
    return <Editor initial={initial} isNew={schemeId === 'new'} showing={showing} />
}
