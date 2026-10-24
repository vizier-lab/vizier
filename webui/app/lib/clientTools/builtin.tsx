// The WebUI's own client tools, registered through the same API an extension uses
// (contracts/webui-extension-api.md). Imported once, for its side effect, from `root.tsx`.

import { ClientCallChip } from '../../screens/chat/ClientCalls'
import { GenerativeUi } from '../../screens/chat/client/GenerativeUi'
import { InteractivePage } from '../../screens/chat/client/InteractivePage'
import { topicKey } from '../../stores/connection'
import { useTurns } from '../../stores/turns'
import { registerClientCategory, registerClientTool } from '../clientTools'
import { answerComponents } from '../generativeUi/reference'
import { LIMITS } from '../generativeUi/limits'
import { formatIssues, validateUi } from '../generativeUi/validate'
import {
    UI_CATEGORY_DESCRIPTION,
    UI_COMPONENTS_DESCRIPTION,
    UI_RENDER_DESCRIPTION,
    uiComponentsSchema,
    uiRenderSchema,
} from './generativeUi'

const byteLength = (text: string) => new TextEncoder().encode(text).length

registerClientCategory({ name: 'ui', description: UI_CATEGORY_DESCRIPTION })

registerClientTool({
    definition: {
        name: 'components',
        category: 'ui',
        description: UI_COMPONENTS_DESCRIPTION,
        input_schema: uiComponentsSchema(),
    },
    handle: (args) => answerComponents(args),
})

registerClientTool({
    definition: {
        name: 'render',
        category: 'ui',
        description: UI_RENDER_DESCRIPTION,
        input_schema: uiRenderSchema(),
        // The interface is the reply: a successful call ends the turn.
        ends_turn: true,
    },
    handle: (args, ctx) => {
        if (byteLength(JSON.stringify(args.spec ?? null)) > LIMITS.specBytes) {
            throw new Error(`the interface is larger than ${LIMITS.specBytes / 1024} KB; make it smaller`)
        }
        // The conversation's files as the topic holds them (kept current by its own frames).
        const held = useTurns.getState().topics[topicKey(ctx.agentId, ctx.topicId)]?.files
        const result = validateUi(args, { files: held?.map((f) => ({ name: f.filename, mime: f.mime_type })) })
        if (!result.ok) throw new Error(formatIssues(result.issues))
        return { ui_id: ctx.callId, status: 'rendered', ...(result.fixed.length ? { fixed: result.fixed } : {}) }
    },
    // A refused interface has nothing to draw; the chip says why.
    render: (call, ctx) => (call.outcome?.status === 'error' ? <ClientCallChip call={call} /> : <GenerativeUi call={call} ctx={ctx} />),
    renderAfterTurn: true,
    latestPerTurn: true,
})

// The hand-written-HTML tool this replaced. No longer offered to the agent, but pages it saved
// still draw, with their state. Tracked for removal with the other legacy shims (#82).
registerClientTool({
    declare: false,
    definition: {
        name: 'render_interactive_ui',
        category: 'ui',
        description: 'Retired: use ui_render.',
        input_schema: { type: 'object' },
    },
    handle: () => {
        throw new Error('render_interactive_ui was replaced by ui_render. Start with ui_components().')
    },
    render: (call, ctx) => (call.outcome?.status === 'error' ? <ClientCallChip call={call} /> : <InteractivePage call={call} ctx={ctx} />),
    renderAfterTurn: true,
    latestPerTurn: true,
})
