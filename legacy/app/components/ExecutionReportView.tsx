// A python run, read as an intent.
//
// What the agent said it was doing is the always-visible line; the script, its output and
// its result are one disclosure deeper. The tool calls the script made are not shown at
// all — the agent's own view of them is unchanged, but a twelve-iteration loop is noise in
// a transcript, so neither the report's list nor the live stream carries them any more.

import ReactMarkdown from 'react-markdown'
import rehypeHighlight from 'rehype-highlight'

import type { ExecutionReport } from '../interfaces/types'
import '../styles/activity-trail.css'

export { parseExecutionReport } from '../lib/trail'

interface ExecutionReportViewProps {
  // Null while the run is still in flight, and for a stored run whose result could not be
  // read back.
  report: ExecutionReport | null
  // Null for a run recorded before `intent` was required.
  intent?: string | null
  code?: string | null
}

export default function ExecutionReportView({
  report,
  intent = null,
  code = null,
}: ExecutionReportViewProps) {
  const seconds = report ? (report.duration_ms / 1000).toFixed(1) : null
  const failure = report?.error?.limit ?? report?.error?.kind
  const failed = report !== null && !report.ok

  const outcome = report === null ? 'running…' : failed ? `${failure} ${seconds}s` : `${seconds}s`

  return (
    <details open={failed} className="execution-report">
      <summary className="execution-report-summary">
        <span className="execution-report-caret" aria-hidden="true">
          ▸
        </span>
        <span className="execution-report-icon">{report === null ? '🐍' : failed ? '❌' : '✅'}</span>
        <span
          className={`execution-report-intent${intent ? '' : ' execution-report-intent--absent'}`}
          title={intent ?? undefined}
        >
          {intent ?? 'Python run'}
        </span>
        <span
          className={`execution-report-outcome${failed ? ' execution-report-outcome--failed' : ''}`}
        >
          {outcome}
        </span>
      </summary>

      {/* The collapsed line truncates a long intent, so the full text lives here. */}
      {intent && (
        <p className="execution-report-full-intent">{intent}</p>
      )}

      {code && (
        <>
          <div className="execution-report-section">Code</div>
          <div className="prose execution-report-box">
            <ReactMarkdown rehypePlugins={[rehypeHighlight]}>
              {'```python\n' + code + '\n```'}
            </ReactMarkdown>
          </div>
        </>
      )}

      {report?.stdout && (
        <>
          <div className="execution-report-section">Output</div>
          <pre className="execution-report-box">{report.stdout}</pre>
        </>
      )}

      {report?.ok && (
        <>
          <div className="execution-report-section">Result</div>
          <div className="prose execution-report-box" style={{ whiteSpace: 'normal' }}>
            <ReactMarkdown rehypePlugins={[rehypeHighlight]}>
              {'```json\n' + JSON.stringify(report.result, null, 2) + '\n```'}
            </ReactMarkdown>
          </div>
        </>
      )}

      {report?.error && (
        <>
          <div className="execution-report-section">Error</div>
          <pre className="execution-report-box">
            {report.error.message}
            {report.error.traceback && `\n\n${report.error.traceback}`}
          </pre>
        </>
      )}
    </details>
  )
}
