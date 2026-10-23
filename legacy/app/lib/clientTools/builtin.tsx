// The WebUI's own client tools, registered through the same API an extension uses
// (contracts/webui-extension-api.md). Imported once, for its side effect, from `root.tsx`.

import { ClientCallChip } from '../../components/client/ClientCallChip'
import { InteractivePage } from '../../components/client/InteractivePage'
import { registerClientCategory, registerClientTool } from '../clientTools'
import { RENDER_INTERACTIVE_UI_DESCRIPTION } from './interactiveUi'

const MAX_PAGE_BYTES = 256 * 1024

const byteLength = (text: string) => new TextEncoder().encode(text).length

registerClientCategory({
  name: 'ui',
  description: 'Show interactive pages in the conversation (describe ui_render_interactive_ui first: it lists the built-in styles)',
})

registerClientTool({
  definition: {
    name: 'render_interactive_ui',
    category: 'ui',
    // A page drawn while the person is away appears when they open the conversation again.
    deferrable: true,
    description: RENDER_INTERACTIVE_UI_DESCRIPTION,
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
