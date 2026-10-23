import type { ReactNode } from 'react'

/** One family of integrations inside Config: what it is for, and its action, over the content. */
export function Panel({ meta, actions, children }: { meta: ReactNode; actions?: ReactNode; children: ReactNode }) {
    return (
        <div className="flex flex-col gap-16">
            <div className="flex flex-wrap items-center justify-between gap-12">
                <p className="type-small text-faint">
                    {meta} · applied at once, without restarting the agent
                </p>
                {actions}
            </div>
            {children}
        </div>
    )
}
