// The WebUI's own client tools, registered through the same API an extension uses
// (contracts/webui-extension-api.md). Imported once, for its side effect, from `root.tsx`.

import { ClientCallChip } from '../../components/client/ClientCallChip'
import { InteractivePage } from '../../components/client/InteractivePage'
import { registerClientCategory, registerClientTool } from '../clientTools'

const MAX_PAGE_BYTES = 256 * 1024

const byteLength = (text: string) => new TextEncoder().encode(text).length

registerClientCategory({ name: 'ui', description: 'Show interactive pages in the conversation' })

registerClientTool({
  definition: {
    name: 'render_interactive_ui',
    category: 'ui',
    // A page drawn while the person is away appears when they open the conversation again.
    deferrable: true,
    description: [
      'Show an interactive HTML page inside your reply in the WebUI: a chart, a table, buttons, a form.',
      'It is shown above your reply text and stays in the conversation.',
      'Write one complete, self-contained page: inline <style> and <script>, images only as data: URLs.',
      'The page has no network access and cannot see the WebUI, the person\'s login or anything else.',
      'Its script gets a `vizier` object:',
      '- vizier.send(text, data?) — the person clicked something: posts `text` as their message and starts a turn; `data` (any JSON) reaches you but is not shown. Only works after the person interacts with the page, at most once a second.',
      '- vizier.state — the state the page last saved (null at first), so it can redraw as it was;',
      '- vizier.saveState(value) — save the page\'s state (any JSON, up to 64 KB); never starts a turn and is not shown to you;',
      '- vizier.onState(cb) — called when the page\'s state is saved from another tab.',
      'Style: match the WebUI with the CSS variables --bg, --surface, --border, --text, --muted, --accent, --error (they follow its light/dark theme). Leave the page background transparent; use --surface for cards, 8px corners, --accent for highlights and chart series. Text, links, buttons and inputs are already themed.',
      'The page appears in your reply once the reply is complete. A reply shows one page: calling this again in the same reply replaces the earlier page, so draw the final version once.',
      'Keep pages small (the limit is 256 KB). Returns {"ui_id", "status": "rendered"}.',
    ].join('\n'),
    input_schema: {
      type: 'object',
      properties: {
        title: {
          type: 'string',
          description: 'Short title, shown to screen readers and in the collapsed view',
        },
        html: {
          type: 'string',
          description: 'A complete, self-contained HTML document (inline CSS/JS, data: images only, no network)',
        },
      },
      required: ['title', 'html'],
    },
  },
  handle: (args, ctx) => {
    if (typeof args.html !== 'string' || args.html.trim() === '') throw new Error('html is required')
    if (byteLength(args.html) > MAX_PAGE_BYTES) throw new Error('html is larger than 256 KB; make the page smaller')
    return { ui_id: ctx.callId, status: 'rendered' }
  },
  // A page that was refused (too large) has nothing to draw.
  render: (call, ctx) =>
    call.outcome?.status === 'error' ? <ClientCallChip call={call} /> : <InteractivePage call={call} ctx={ctx} />,
  renderAfterTurn: true,
  latestPerTurn: true,
})
