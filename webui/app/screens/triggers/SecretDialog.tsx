import { Button, CodeBlock, Dialog, SecretReveal } from '@vizier/design'
import { transport } from '../../api/transport'

/**
 * Where a webhook is reached from outside. The desktop app serves its page itself, so there the
 * path alone is true: the host is the one web access answers on.
 */
export function webhookAddress(path: string): string {
    return transport.authMode === 'desktop' ? path : `${window.location.origin}${path}`
}

/** A new webhook secret, shown once. */
export function SecretDialog({ secret, path, onClose }: { secret: string | null; path?: string | null; onClose: () => void }) {
    return (
        <Dialog open={secret !== null} size="lg" title="Your webhook is ready" confirmLabel="" onConfirm={() => {}} onCancel={onClose}>
            <div className="flex flex-col gap-16">
                <p className="type-ui-para text-muted">Copy the secret now. It is shown once; you can replace it later, not read it.</p>
                {path && (
                    <div className="flex flex-col gap-6">
                        <span className="type-ui-medium text-soft">Address</span>
                        <CodeBlock code={webhookAddress(path)} />
                    </div>
                )}
                {secret && <SecretReveal title="Secret" secret={secret} />}
                <div className="flex flex-col gap-6">
                    <span className="type-ui-medium text-soft">Send it in this header</span>
                    <CodeBlock code="Authorization: Bearer <secret>" />
                </div>
                <p className="type-small-para text-faint">
                    Can’t set headers? Add /&lt;secret&gt; to the end of the address instead. Proxies often log addresses, so prefer the header.
                    {transport.authMode === 'desktop' && ' The desktop app receives webhooks only while web access is on.'}
                </p>
                <div className="flex justify-end">
                    <Button variant="primary" onClick={onClose}>
                        Done
                    </Button>
                </div>
            </div>
        </Dialog>
    )
}
