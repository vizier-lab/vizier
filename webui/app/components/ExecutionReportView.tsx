import ReactMarkdown from 'react-markdown'
import rehypeHighlight from 'rehype-highlight'
import type { ExecutionReport } from '../interfaces/types'

// Same key set as the server's `ExecutionReport::looks_like`.
const REPORT_KEYS = ['ok', 'stdout', 'tool_calls', 'duration_ms'] as const

export function parseExecutionReport(value: unknown): ExecutionReport | null {
  if (!value || typeof value !== 'object') return null
  return REPORT_KEYS.every((key) => key in value) ? (value as ExecutionReport) : null
}

const scrollBox: React.CSSProperties = {
  maxHeight: '16rem',
  overflow: 'auto',
  margin: 0,
  padding: '0.5rem',
  borderRadius: '0.25rem',
  background: 'var(--background)',
  fontSize: '0.75rem',
  whiteSpace: 'pre-wrap',
  wordBreak: 'break-word',
}

const sectionTitle: React.CSSProperties = {
  fontSize: '0.75rem',
  fontWeight: 600,
  color: 'var(--text-secondary)',
  margin: '0.5rem 0 0.25rem',
}

// `arguments` is absent from a report that came back from the model rather than
// from the session record, where it is always kept.
const formatArgs = (args: Record<string, unknown> | undefined) =>
  Object.entries(args ?? {})
    .map(([key, value]) => `${key}=${JSON.stringify(value)}`)
    .join(', ')

export default function ExecutionReportView({ report }: { report: ExecutionReport }) {
  const seconds = (report.duration_ms / 1000).toFixed(1)
  const failure = report.error?.limit ?? report.error?.kind
  const heading = report.ok
    ? `✅ Python finished in ${seconds}s`
    : `❌ Python failed (${failure}) in ${seconds}s`

  return (
    <details open={!report.ok} style={{ width: '100%', fontSize: '0.8rem' }}>
      <summary style={{ cursor: 'pointer' }}>{heading}</summary>

      {report.tool_calls.length > 0 && (
        <>
          <div style={sectionTitle}>Tool calls</div>
          <ol style={{ margin: 0, paddingLeft: '1.25rem' }}>
            {report.tool_calls.map((call) => (
              <li key={call.seq} style={{ fontFamily: 'monospace', fontSize: '0.75rem' }}>
                {call.name}({formatArgs(call.arguments)}) {call.ok ? '✓' : '✗'} {call.duration_ms}ms
                {call.error && <span style={{ color: 'var(--text-tertiary)' }}> — {call.error}</span>}
              </li>
            ))}
          </ol>
        </>
      )}

      {report.stdout && (
        <>
          <div style={sectionTitle}>Output</div>
          <pre style={scrollBox}>{report.stdout}</pre>
        </>
      )}

      {report.ok && (
        <>
          <div style={sectionTitle}>Result</div>
          <div className="prose" style={{ ...scrollBox, whiteSpace: 'normal' }}>
            <ReactMarkdown rehypePlugins={[rehypeHighlight]}>
              {'```json\n' + JSON.stringify(report.result, null, 2) + '\n```'}
            </ReactMarkdown>
          </div>
        </>
      )}

      {report.error && (
        <>
          <div style={sectionTitle}>Error</div>
          <pre style={scrollBox}>
            {report.error.message}
            {report.error.traceback && `\n\n${report.error.traceback}`}
          </pre>
        </>
      )}
    </details>
  )
}
