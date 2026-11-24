import { CallChip } from '@vizier/design'
import type { ReactNode } from 'react'
import type { ClientCallView, Turn } from '../../api/frames'
import { findClientTool, visibleClientCalls, type ClientCallContext } from '../../lib/clientTools'

/** A call this WebUI has no renderer for (another client's, or an extension that is gone). */
export function ClientCallChip({ call }: { call: ClientCallView }) {
    const status = call.outcome?.status ?? 'pending'
    return <CallChip status={status as never} detail={`${call.name}${call.outcome?.status === 'error' ? ` · ${call.outcome.message}` : ''}`} />
}

/** The client tool calls of one turn, drawn by their tools' own renderers. */
export function renderClientCalls(
    turn: Turn,
    running: boolean,
    contextFor: (call: ClientCallView) => ClientCallContext
): ReactNode {
    // A tool this client provides but draws nothing for (a lookup like ui_components) leaves no
    // trace in the reply; only a call this client has no registration for gets a chip.
    const calls = visibleClientCalls(turn.clientCalls, running).filter((call) => {
        const registration = findClientTool(call.category, call.tool)
        return !registration || registration.render
    })
    if (calls.length === 0) return null
    return (
        <div className="flex flex-col gap-8">
            {calls.map((call) => {
                const render = findClientTool(call.category, call.tool)?.render
                return <div key={call.callId}>{render ? render(call, contextFor(call)) : <ClientCallChip call={call} />}</div>
            })}
        </div>
    )
}
