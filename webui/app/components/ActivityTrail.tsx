// The one renderer for an activity trail, whether it was just streamed or reloaded from
// history. Both producers in `app/lib/trail.ts` feed this component, so a turn looks the
// same before and after a refresh.
//
// A native `<details>`/`<summary>`, as `ExecutionReportView` already uses: each turn's
// disclosure keeps its own state, so expanding one does nothing to any other. The summary
// is counts and a duration rather than a list, which is what keeps it two lines however
// much the trail holds.

import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'

import ExecutionReportView from './ExecutionReportView'
import { trailSummary } from '../lib/trail'
import type { TrailEvent } from '../interfaces/types'

interface ActivityTrailProps {
  trail: TrailEvent[]
  // A streaming turn renders expanded; a finished one renders folded.
  live: boolean
  durationMs?: number
  // What a `tool` event is labelled with. Supplied by the caller so the trail does not
  // have to know the whole tool catalogue.
  label: (name: string, args: Record<string, unknown>) => string
  // Where the trail is being drawn, which decides whether it draws a container of its own:
  //
  //   `inset` — inside the answer's own bubble, above the answer. The default for a turn
  //             that produced one, so what the agent did and what it concluded read as one
  //             thing. Separated from the answer by a hairline, not by a box.
  //   `bare`  — inside the live thinking balloon, which already draws the container.
  //   `block` — standalone, for a trail with no answer to sit inside: one closed by a
  //             checkpoint or a command, or one nothing closed at all.
  variant?: 'inset' | 'bare' | 'block'
}

const quote = (text: string) =>
  text
    .split('\n')
    .map((line) => `> ${line} `)
    .join('\n')

export function ActivityTrail({
  trail,
  live,
  durationMs,
  label,
  variant = 'block',
}: ActivityTrailProps) {
  // FR-017: nothing on the way to the answer means no disclosure at all, not an empty one.
  if (trail.length === 0) return null

  return (
    <details open={live} className={`activity-trail activity-trail--${variant}`}>
      <summary className="activity-trail-summary">
        <span className="activity-trail-caret" aria-hidden="true">
          ▸
        </span>
        <span className="activity-trail-label">{trailSummary(trail, durationMs)}</span>
      </summary>

      <div className="activity-trail-body">
        {trail.map((event) => {
          switch (event.kind) {
            case 'thought':
              return (
                <div key={event.id} className="prose activity-trail-event">
                  <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                    {quote(event.text)}
                  </ReactMarkdown>
                </div>
              )
            case 'narration':
              return (
                <div key={event.id} className="prose activity-trail-event">
                  <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                    {event.text}
                  </ReactMarkdown>
                </div>
              )
            case 'tool':
              return (
                <div
                  key={event.id}
                  className="prose activity-trail-event"
                  data-job-id={event.jobId}
                >
                  <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                    {label(event.name, event.args)}
                  </ReactMarkdown>
                </div>
              )
            case 'python':
              return (
                <div key={event.id} className="activity-trail-event">
                  <ExecutionReportView
                    report={event.report}
                    intent={event.intent}
                    code={event.code}
                  />
                </div>
              )
          }
        })}
      </div>
    </details>
  )
}

export default ActivityTrail
