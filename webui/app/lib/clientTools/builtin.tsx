// The WebUI's own client tools, registered through the same API an extension uses
// (contracts/webui-extension-api.md). Imported once, for its side effect, from `root.tsx`.
//
// The WebUI declares no tools to the agent any more: generative UI is the built-in `ui_render`
// (spec 063). The two tools below are the ones it replaced, kept `declare: false` so the pages
// they saved still draw, with their state. Tracked for removal with the other legacy shims (#82).

import { ClientCallChip } from '../../screens/chat/ClientCalls'
import { GenerativeUi } from '../../screens/chat/client/GenerativeUi'
import { InteractivePage } from '../../screens/chat/client/InteractivePage'
import { registerClientTool } from '../clientTools'

const retired = (name: string) => () => {
    throw new Error(`${name} was replaced by the built-in ui_render tool; see its guide.`)
}

// The json-render tool of spec 026 (`ui_render` as a client tool, `ui` + `render`).
registerClientTool({
    declare: false,
    definition: {
        name: 'render',
        category: 'ui',
        description: 'Retired: use the built-in ui_render.',
        input_schema: { type: 'object' },
    },
    handle: retired('ui_render (client)'),
    render: (call, ctx) => (call.outcome?.status === 'error' ? <ClientCallChip call={call} /> : <GenerativeUi call={call} ctx={ctx} />),
    renderAfterTurn: true,
    latestPerTurn: true,
})

// The hand-written-HTML tool before it.
registerClientTool({
    declare: false,
    definition: {
        name: 'render_interactive_ui',
        category: 'ui',
        description: 'Retired: use the built-in ui_render.',
        input_schema: { type: 'object' },
    },
    handle: retired('render_interactive_ui'),
    render: (call, ctx) => (call.outcome?.status === 'error' ? <ClientCallChip call={call} /> : <InteractivePage call={call} ctx={ctx} />),
    renderAfterTurn: true,
    latestPerTurn: true,
})
