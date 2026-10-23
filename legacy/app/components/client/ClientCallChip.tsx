// A client tool call this WebUI has no renderer for: made by another client, or by an
// extension that is no longer installed. Shown as a folded item naming the tool, never as
// an error (specs/019-client-tools FR-029).

import type { ClientCallView } from '../../interfaces/types'

const STATUS_LABEL: Record<string, string> = {
  pending: 'running',
  ok: 'done',
  error: 'failed',
  disconnected: 'disconnected',
  timeout: 'timed out',
  cancelled: 'cancelled',
  interrupted: 'interrupted',
  deferred: 'waiting for the client',
}

export function ClientCallChip({ call }: { call: ClientCallView }) {
  const status = call.outcome?.status ?? 'pending'
  return (
    <details className="activity-trail activity-trail--block">
      <summary className="activity-trail-summary">
        <span className="activity-trail-caret" aria-hidden="true">
          ▸
        </span>
        <span className="activity-trail-label">
          Client tool <code>{call.name}</code>
        </span>
        <span
          className="ml-2 inline-flex items-center gap-1 rounded px-1.5 text-[11px]"
          style={{ background: 'var(--surface)', color: 'var(--text-tertiary)' }}
        >
          {status === 'pending' && (
            <span
              aria-hidden="true"
              className="inline-block h-2.5 w-2.5 animate-spin rounded-full border border-current border-t-transparent"
            />
          )}
          {STATUS_LABEL[status] ?? status}
        </span>
      </summary>
      <div className="activity-trail-body">
        <pre className="activity-trail-event overflow-x-auto text-xs">
          {JSON.stringify(call.args, null, 2)}
        </pre>
        {call.outcome?.status === 'error' && (
          <div className="activity-trail-event text-xs" style={{ color: '#ef4444' }}>
            {call.outcome.message}
          </div>
        )}
      </div>
    </details>
  )
}

export default ClientCallChip
