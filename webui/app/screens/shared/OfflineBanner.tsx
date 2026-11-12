import { Banner, Button } from '@vizier/design'

/**
 * The app cannot reach the engine. Says what is unavailable rather than "you are
 * offline": reading what is already open still works, sending does not.
 */
export function OfflineBanner({ onRetry }: { onRetry: () => void }) {
    return (
        <Banner
            tone="offline"
            title="Vizier can't reach the engine"
            message="Replies and new conversations need a connection. What is already open stays readable."
            actions={
                <Button size="sm" variant="secondary" onClick={onRetry}>
                    Try again
                </Button>
            }
        />
    )
}
