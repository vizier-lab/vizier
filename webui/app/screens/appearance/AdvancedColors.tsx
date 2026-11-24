import { Button, ColorField } from '@vizier/design'
import { useState } from 'react'
import { ROLE_GROUPS, ROLE_LABEL, rolesIn, type Hex, type Role, type Variant } from '../../lib/themes/roles'

type Props = {
    variant: Variant
    onRole: (role: Role, hex: Hex) => void
    onReset: (role: Role) => void
}

/** Every role, by purpose, each settable on its own. Collapsed: most people never need it. */
export function AdvancedColors({ variant, onRole, onReset }: Props) {
    const [open, setOpen] = useState(false)
    return (
        <div className="flex flex-col gap-16">
            <div>
                <Button variant="secondary" size="sm" aria-expanded={open} icon={open ? 'chevron-down' : 'chevron-right'} onClick={() => setOpen(!open)}>
                    {open ? 'Hide advanced' : 'Show advanced'}
                </Button>
            </div>
            {open &&
                ROLE_GROUPS.map(({ group, label }) => (
                    <section key={group} className="flex flex-col gap-8">
                        <h3 className="type-small-medium text-muted">{label}</h3>
                        <div className="grid gap-12" style={{ gridTemplateColumns: 'repeat(auto-fill, minmax(136px, 1fr))' }}>
                            {rolesIn(group).map((role) => (
                                <ColorField
                                    key={role}
                                    label={ROLE_LABEL[role]}
                                    value={variant.roles[role]}
                                    overridden={variant.overridden.includes(role)}
                                    onChange={(hex) => onRole(role, hex)}
                                    onReset={() => onReset(role)}
                                />
                            ))}
                        </div>
                    </section>
                ))}
        </div>
    )
}
