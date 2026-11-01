import { Notice } from '@vizier/design'
import { transport } from '../../api/transport'

/**
 * The persistent "commands run on the host machine" indicator for a session reached through the
 * browser (FR-023). Desktop mode is the host, so it needs no notice; nor does an agent that has
 * no way to run a command, which is why `shell` is asked for.
 */
export function HostExecutionNotice({
    shell,
    className,
}: {
    /** This session can run a host command: the Shell addon, or the project's own shell switch. */
    shell: boolean
    className?: string
}) {
    if (transport.authMode === 'desktop' || !shell) return null
    return (
        <Notice tone="info" icon="square-terminal" className={className}>
            Commands run on the host machine, with the host account.
        </Notice>
    )
}
