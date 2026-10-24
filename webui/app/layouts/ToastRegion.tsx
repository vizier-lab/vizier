import { Presence, Toast, ToastRegion as Region } from '@vizier/design'
import { useToasts } from '../stores/toasts'

export function ToastRegion() {
    const { items, dismiss } = useToasts()
    return (
        <Region>
            {/* Presence keeps a dismissed toast on screen long enough to leave. */}
            <Presence>
                {items.map((t) => (
                    <Toast
                        key={t.id}
                        tone={t.tone}
                        title={t.title}
                        message={t.message}
                        onClose={() => dismiss(t.id)}
                    />
                ))}
            </Presence>
        </Region>
    )
}
