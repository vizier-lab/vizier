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
            size={p.size === 'sm' || p.size === 'lg' ? p.size : p.square ? 'lg' : 'md'}
            disabled={p.disabled === true}
            aria-pressed={p.selected === undefined ? undefined : selected}
            className={[
                selected ? 'ring-2 ring-ink' : accent ? 'ring-2 ring-chart-1' : '',
                // A square fills its grid cell: no fixed height or side padding.
                p.square === true ? 'aspect-square h-auto! w-full px-0!' : '',
            ]
                .filter(Boolean)
                .join(' ')}
            onClick={(e) => send('press', e.nativeEvent)}
        >
            {p.square === true ? <span className="type-title">{String(p.label ?? '')}</span> : String(p.label ?? '')}
        </Button>
    )
}
