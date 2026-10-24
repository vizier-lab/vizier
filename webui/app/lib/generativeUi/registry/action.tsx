import { Button } from '@vizier/design'
import type { ComponentRenderProps } from '@json-render/react'
import { useEmitter } from './shared'

type P = Record<string, unknown>

const VARIANT = { primary: 'primary', secondary: 'secondary', ghost: 'ghost', danger: 'danger' } as const

export function ButtonC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const send = useEmitter(emit)
    const selected = p.selected === true
    const accent = p.tone === 'accent'
    return (
        <Button
            variant={VARIANT[(p.variant as keyof typeof VARIANT) ?? 'secondary'] ?? 'secondary'}
            disabled={p.disabled === true}
            aria-pressed={p.selected === undefined ? undefined : selected}
            className={selected ? 'ring-2 ring-ink' : accent ? 'ring-1 ring-chart-1' : undefined}
            onClick={(e) => send('press', e.nativeEvent)}
        >
            {String(p.label ?? '')}
        </Button>
    )
}
