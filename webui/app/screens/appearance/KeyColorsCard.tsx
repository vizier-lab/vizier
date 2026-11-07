import { Button, Card, ColorField, FilterTabs, Notice, TextField } from '@vizier/design'
import { colorProblems } from '../../lib/themes/contrast'
import { KEY_COLORS, KEY_LABEL, MAX_NAME_LENGTH, type Appearance, type Hex, type KeyColor, type Scheme, type Variant } from '../../lib/themes/roles'

type Props = {
    draft: Scheme
    nameError: string | null
    appearance: Appearance
    onName: (name: string) => void
    onAppearance: (appearance: Appearance) => void
    onAddVariant: () => void
    onRemoveVariant: () => void
    onKey: (key: KeyColor, hex: Hex) => void
}

/** The name, which variant is being edited, and the colours most people will ever touch. */
export function KeyColorsCard({ draft, nameError, appearance, onName, onAppearance, onAddVariant, onRemoveVariant, onKey }: Props) {
    const variant: Variant | undefined = draft[appearance]
    const both = !!draft.light && !!draft.dark
    const problems = variant ? colorProblems(variant.roles) : []
    return (
        <Card title="Theme">
            <TextField label="Name" value={draft.name} maxLength={MAX_NAME_LENGTH} invalid={!!nameError} helper={nameError ?? undefined} onChange={(e) => onName(e.target.value)} />
            <div className="flex flex-wrap items-center gap-8">
                <FilterTabs
                    aria-label="Variant to edit"
                    options={[
                        { value: 'light' as const, label: draft.light ? 'Light' : 'Light (none)' },
                        { value: 'dark' as const, label: draft.dark ? 'Dark' : 'Dark (none)' },
                    ]}
                    value={appearance}
                    onChange={onAppearance}
                />
                {variant ? (
                    <Button variant="secondary" size="sm" disabled={!both} onClick={onRemoveVariant}>
                        Remove this variant
                    </Button>
                ) : (
                    <Button variant="secondary" size="sm" onClick={onAddVariant}>
                        Add this variant
                    </Button>
                )}
            </div>
            {variant ? (
                <>
                    <div className="grid gap-12" style={{ gridTemplateColumns: 'repeat(auto-fill, minmax(136px, 1fr))' }}>
                        {KEY_COLORS.map((key) => (
                            <ColorField key={key} label={KEY_LABEL[key]} value={variant.keys[key]} onChange={(hex) => onKey(key, hex)} />
                        ))}
                    </div>
                    {problems.length > 0 && (
                        <Notice tone="retrying" title="Some colors may be hard to tell apart">
                            <ul className="list-disc pl-16">
                                {problems.map((p) => (
                                    <li key={p.id}>{p.message}</li>
                                ))}
                            </ul>
                        </Notice>
                    )}
                </>
            ) : (
                <p className="type-ui-para text-muted">This theme has no {appearance} variant, so it always shows the other one.</p>
            )}
        </Card>
    )
}
