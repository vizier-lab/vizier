import { Presence, Toast, ToastRegion as Region } from '@vizier/design'
import { useToasts } from '../stores/toasts'

export function ToastRegion() {
    const { items, dismiss } = useToasts()
    return (
        // The corner itself (24px in, as from the right edge); the design default sits under the header.
        <Region position="top-right" className="top-24!">
            {/* Presence keeps a dismissed toast on screen long enough to leave. */}
            <Presence>
                {items.map((t) => (
                    <Toast
                        key={t.id}
                        tone={t.tone}
                        title={t.title}
                        message={t.message}
                        onSelect={
                            t.onSelect
                                ? () => {
                                      t.onSelect?.()
                                      dismiss(t.id)
                                  }
                                : undefined
                        }
                        onClose={() => dismiss(t.id)}
                    />
                ))}
            </Presence>
        </Region>
    )
}
