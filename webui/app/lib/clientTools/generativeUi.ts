// What the agent reads about `ui_components` and `ui_render` (contracts/generative-ui-tools.md).
// The descriptions are the cheat sheet: the full component list is `ui_components`, generated from
// the catalogue, so these never list a component's props.
import { ACTIVE_GROUPS, COMPONENT_NAMES } from '../generativeUi/catalog/index'
import { RECIPE_NAMES } from '../generativeUi/recipes/index'

export const UI_CATEGORY_DESCRIPTION =
    'Show an interface made of ready-made components (stats, tables, charts, forms, 3D) in your reply. Start with ui_components.'

export const UI_COMPONENTS_DESCRIPTION =
    'Look up the components, props and ready-made examples for ui_render. No arguments: the index. names=[…]: exact props, events and an example for each. group=…: one group. recipe=…: a complete interface to adapt.'

// Kept under 1.8 KB: the 2 KB limit also has to hold the turn-ending notice vizier adds.
export const UI_RENDER_DESCRIPTION = [
    'Show an interface in your reply: ui_render(title=…, spec=…). It is your final answer.',
    'Only catalogue components work: ui_components() lists them, ui_components(names=[…]) gives props, ui_components(recipe=…) gives full examples.',
    'spec = {"root":"id","elements":{"id":{"type":"Card","props":{…},"children":["id2"]}},"state":{…},"logic":{…}}.',
    'Props can be values or expressions: {"$state":"/a/b"} read, {"$bindState":"/a/b"} read+write (inputs), {"$template":"Hi ${/name}"}, {"$cond":{"$state":"/x","eq":1},"$then":…,"$else":…}, {"$computed":"add","args":{"a":…,"b":…}}. In a repeat: {"$item":"field"}, {"$index":true}.',
    'Put visible, repeat, on, watch beside props on the element, never inside props.',
    'Events: "on":{"press":{"action":"setState","params":{"statePath":"/tab","value":"b"}}}. Actions: setState, pushState, removeState, validateForm, sendMessage {text,data} (sends a message as the person), run {fn,args,into} (calls a function from "logic").',
    '"logic" holds small pure JS functions {"name":{"params":["a"],"body":"return …"}} for rules and maths; they run locally, with no network or page access. State keys starting "_" are derived and not saved.',
    'Show files of this conversation with Image/Audio/Video/FileCard/Gallery by file name (see list_session_files); make the file first. Link takes https/http/mailto only.',
    'Limits: 256 KB, 1500 elements, depth 16. A mistake returns every problem with the valid options; fix and call again.',
    'Returns {"ui_id","status":"rendered"}. The person sees it after your turn, so do all other work first.',
].join('\n')

export function uiComponentsSchema() {
    return {
        type: 'object' as const,
        properties: {
            names: { type: 'array', items: { type: 'string' }, description: 'Component names for full detail' },
            group: { type: 'string', enum: [...ACTIVE_GROUPS], description: 'List one group of components' },
            recipe: { type: 'string', enum: [...RECIPE_NAMES], description: 'A complete valid interface to adapt' },
        },
    }
}

export function uiRenderSchema() {
    return {
        type: 'object' as const,
        required: ['title', 'spec'],
        properties: {
            title: { type: 'string', maxLength: 120, description: 'Short title for screen readers and the collapsed view' },
            spec: {
                type: 'object',
                required: ['root', 'elements'],
                properties: {
                    root: { type: 'string' },
                    elements: {
                        type: 'object',
                        additionalProperties: {
                            type: 'object',
                            required: ['type'],
                            properties: {
                                type: { enum: [...COMPONENT_NAMES] },
                                props: { type: 'object' },
                                children: { type: 'array', items: { type: 'string' } },
                                visible: {},
                                repeat: { type: 'object' },
                                on: { type: 'object' },
                                watch: { type: 'object' },
                            },
                        },
                    },
                    state: { type: 'object' },
                    logic: { type: 'object' },
                },
            },
        },
    }
}
