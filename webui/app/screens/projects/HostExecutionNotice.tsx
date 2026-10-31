import { Notice } from '@vizier/design'
import { transport } from '../../api/transport'

/**
 * The persistent "commands run on the host machine" indicator for a project session reached
 * through the browser (FR-023). Desktop mode is the host, so it needs no notice.
 */
export function HostExecutionNotice({ className }: { className?: string }) {
    if (transport.authMode === 'desktop') return null
    return (
        <Notice tone="info" icon="square-terminal" className={className}>
            Commands run on the host machine, with the host account.
        </Notice>
    )
}
