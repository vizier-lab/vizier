import { Button } from '@vizier/design'
import type { ProviderSummary } from '../../api/endpoints/catalogue'
import { PERMISSIONS, useCan } from '../../lib/access'
import { toast } from '../../stores/toasts'
import { ConnectForm } from '../providers/ConnectForm'

type Props = {
    provider: ProviderSummary
    /** Closed without connecting. */
    onDismiss: () => void
    /** Connected: re-read the catalogue so the pickers show it. */
    onConnected: () => void
}

/**
 * A chosen provider that isn't connected yet gets its credentials form right where it was chosen,
 * so picking a model never sends anyone to Settings and back.
 */
export function ProviderConnectPanel({ provider, onDismiss, onConnected }: Props) {
    const canConnect = useCan(PERMISSIONS.providers)
    return (
        <div className="flex flex-col gap-12 rounded-lg border border-dashed border-border-strong p-16">
            <div className="flex flex-col gap-2">
                <span className="type-ui-medium text-fg">Connect {provider.name} to use it</span>
                <span className="type-small text-faint">
                    {canConnect
                        ? 'Its credentials are saved in Vizier and shared with every agent that uses this provider.'
                        : `${provider.name} isn’t connected yet. Ask someone who manages providers to connect it.`}
                </span>
            </div>
            {canConnect ? (
                <ConnectForm
                    key={provider.id}
                    provider={provider}
                    onCancel={onDismiss}
                    onDone={() => {
                        toast.success(`${provider.name} connected`)
                        onConnected()
                    }}
                />
            ) : (
                <div>
                    <Button onClick={onDismiss}>Dismiss</Button>
                </div>
            )}
        </div>
    )
}
