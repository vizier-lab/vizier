import { Toast, ToastRegion as Region } from '@vizier/design'
import { useToasts } from '../stores/toasts'

export function ToastRegion() {
    const { items, dismiss } = useToasts()
    return (
        <Region>
            {items.map((t) => (
                <Toast key={t.id} tone={t.tone} title={t.title} message={t.message} onClose={() => dismiss(t.id)} />
            ))}
        </Region>
    )
}
