import { Notice } from '@vizier/design'

/** The trust disclosure shown before a folder becomes a project (FR-002). */
export function TrustDisclosure({ folder }: { folder: string }) {
    return (
        <Notice tone="failed" title="This gives the agent your machine" icon="alert">
            <ul className="mt-4 list-disc pl-20">
                <li>Commands run with your operating-system account.</li>
                <li>There is no sandbox: anything you can reach, the agent can reach.</li>
                <li>Only open folders you trust with that access.</li>
            </ul>
            <p className="mt-8 type-code">{folder}</p>
        </Notice>
    )
}
