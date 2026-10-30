import { Banner, Button } from '@vizier/design'
import { transport } from '../../api/transport'
import { stoppedSubsystems } from '../../lib/desktop'
import { useDesktop } from '../../stores/desktop'

/** The engine is running but part of it stopped: say which, and offer a restart. */
export function RuntimeBanner() {
    const state = useDesktop((s) => s.state)
    if (state?.state !== 'degraded') return null
    const stopped = stoppedSubsystems(state)
    return (
        <Banner
            tone="failed"
            title="Part of Vizier stopped"
            message={stopped.length ? `Stopped: ${stopped.join(', ')}.` : undefined}
            actions={
                <Button variant="secondary" onClick={() => void transport.desktop?.restart()}>
                    Restart
                </Button>
            }
        />
    )
}
