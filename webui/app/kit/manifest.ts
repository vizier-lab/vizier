// What the kit offers a page (specs/063-html-generative-ui research R11, tasks T051 and T053): every
// `<vz-*>` element with its attributes, events and an example, and the libraries a page may
// import. One source for three readers: the elements parse their attributes with it
// (`elements/parse.ts`), the kit build writes it into `ui-kit/manifest.json`, and
// `scripts/ui-kit-reference.test.mjs` fails when `REFERENCE.md` (the agent's guide) leaves out
// or invents an element, attribute or library. No React here: the build reads it in Node.
//
// The json-render catalogue's zod schemas (`lib/generativeUi/catalog/`) stay where they are: they
// describe props of a JSON spec, which the read-only renderer still validates. These describe
// HTML attributes, which are strings; JSON-valued ones are checked with the zod schemas below.

import { z } from 'zod'

// zod probes `new Function` to compile its checks, which a page's policy blocks and reports as a
// problem on every page that uses an element. A schema reads this when it is built, so it is set
// before the schemas below.
z.config({ jitless: true })

export type AttrType = 'string' | 'number' | 'boolean' | 'enum' | 'json'

export type AttrSpec = {
    type: AttrType
    description: string
    /** The allowed values of an `enum` attribute. */
    values?: readonly string[]
    required?: boolean
    /** The shape of a `json` attribute's value. */
    schema?: z.ZodType
}

export type ElementSpec = {
    name: string
    description: string
    attributes: Record<string, AttrSpec>
    /** DOM events it fires, with what `event.detail` holds. */
    events: Record<string, string>
    /** What its children are for; absent when it takes none. */
    children?: string
    /** JSON attributes can also be set as properties of the same name, from script. */
    example: string
}

const tone = { type: 'enum', values: ['default', 'accent', 'ok', 'warn', 'error'], description: 'colour of the element' } as const

const format = {
    type: 'enum',
    values: ['number', 'percent', 'currency', 'bytes', 'duration'],
    description: 'how a number is shown: percent takes a fraction (0.42 is 42%), duration takes seconds',
} as const

const column = z.object({
    key: z.string().min(1).max(100),
    label: z.string().max(200),
    align: z.enum(['start', 'end']).optional(),
    format: z.enum(['number', 'percent', 'currency', 'bytes', 'duration']).optional(),
})

const option = z.union([z.string().max(200), z.object({ value: z.string().max(200), label: z.string().max(200) })])

export const ELEMENTS: readonly ElementSpec[] = [
    {
        name: 'vz-card',
        description: 'A bordered panel with an optional title, holding any HTML.',
        attributes: {
            title: { type: 'string', description: 'heading of the card' },
            subtitle: { type: 'string', description: 'one line under the title' },
            tone: { type: 'enum', values: ['default', 'error'], description: 'error draws it as a danger card' },
        },
        events: {},
        children: 'any HTML, drawn inside the card (add more later through `card.body`)',
        example: '<vz-card title="This week" subtitle="Signups by day"><p>Steady growth.</p></vz-card>',
    },
    {
        name: 'vz-stat',
        description: 'One figure with its label, and an optional change under it.',
        attributes: {
            label: { type: 'string', description: 'what the figure is', required: true },
            value: { type: 'number', description: 'the figure', required: true },
            format,
            delta: { type: 'string', description: 'a change to show under it, as text ("+12% on last week")' },
            tone: { type: 'enum', values: ['default', 'error'], description: 'error draws the figure in the failed colour' },
        },
        events: {},
        example: '<vz-stat label="Activated" value="0.61" format="percent" delta="+4 pts"></vz-stat>',
    },
    {
        name: 'vz-table',
        description: 'Rows of data under labelled columns, numbers right-aligned on request.',
        attributes: {
            columns: { type: 'json', description: 'list of {key, label, align?: "start"|"end", format?}', required: true, schema: z.array(column).min(1).max(50) },
            rows: { type: 'json', description: 'list of objects keyed by the column keys', schema: z.array(z.record(z.string(), z.unknown())).max(5000) },
            label: { type: 'string', description: 'what the table holds, for screen readers' },
            'max-rows': { type: 'number', description: 'rows shown before "Showing N of M" (default 200)' },
            selectable: { type: 'boolean', description: 'rows can be pressed; fires select' },
        },
        events: { select: '{row, index}: the row pressed (selectable only)' },
        example:
            '<vz-table label="Scores" columns=\'[{"key":"name","label":"Name"},{"key":"score","label":"Score","align":"end"}]\' rows=\'[{"name":"Ada","score":91},{"name":"Lin","score":84}]\'></vz-table>',
    },
    {
        name: 'vz-chart',
        description: 'A bar, line, area or pie chart in the theme colours.',
        attributes: {
            kind: { type: 'enum', values: ['bar', 'line', 'area', 'pie'], description: 'the kind of chart', required: true },
            data: {
                type: 'json',
                description: 'list of {label, value, series?}; points with the same series form one line or bar group',
                required: true,
                schema: z.array(z.object({ label: z.string().max(200), value: z.number(), series: z.string().max(100).optional() })).max(2000),
            },
            title: { type: 'string', description: 'heading over the chart' },
            height: { type: 'number', description: 'height in px (default 220)' },
            legend: { type: 'boolean', description: 'show the series legend' },
        },
        events: { press: '{label, value}: the bar, point or slice pressed' },
        example: '<vz-chart kind="bar" title="Signups" data=\'[{"label":"Mon","value":120},{"label":"Tue","value":180}]\'></vz-chart>',
    },
    {
        name: 'vz-tabs',
        description: 'Tabs over sections: each child `<section label="…">` is one tab.',
        attributes: {
            value: { type: 'string', description: 'the label of the tab shown first (default the first)' },
            label: { type: 'string', description: 'what the tabs choose between, for screen readers' },
        },
        events: { change: '{value}: the label of the tab chosen' },
        children: '`<section label="…">` elements, one per tab',
        example: '<vz-tabs><section label="Summary"><p>…</p></section><section label="Details"><p>…</p></section></vz-tabs>',
    },
    {
        name: 'vz-field',
        description: 'A labelled input: text, multi-line text, number, select, checkbox, switch or slider.',
        attributes: {
            kind: {
                type: 'enum',
                values: ['text', 'textarea', 'number', 'select', 'checkbox', 'switch', 'slider'],
                description: 'the kind of input (default text)',
            },
            label: { type: 'string', description: 'the label shown with it', required: true },
            name: { type: 'string', description: 'a name for your own code; it is in every event' },
            value: { type: 'string', description: 'the starting value ("true" for a checked checkbox or switch)' },
            placeholder: { type: 'string', description: 'hint inside an empty text or number field' },
            options: { type: 'json', description: 'select only: list of strings, or of {value, label}', schema: z.array(option).max(500) },
            min: { type: 'number', description: 'number and slider: the least value' },
            max: { type: 'number', description: 'number and slider: the greatest value' },
            step: { type: 'number', description: 'number and slider: the step' },
            help: { type: 'string', description: 'a line of help under it' },
            disabled: { type: 'boolean', description: 'cannot be changed' },
        },
        events: {
            input: '{name, value}: on every change while typing or dragging',
            change: '{name, value}: when a value is committed (and on every change for select, checkbox and switch)',
        },
        example: '<vz-field kind="select" name="range" label="Range" value="7d" options=\'["24h","7d","30d"]\'></vz-field>',
    },
    {
        name: 'vz-badge',
        description: 'A small label, its text the element\'s own text.',
        attributes: { tone },
        events: {},
        children: 'the text of the badge',
        example: '<vz-badge tone="ok">live</vz-badge>',
    },
    {
        name: 'vz-callout',
        description: 'A note set apart from the page: a quiet hint, or a warning or error.',
        attributes: { tone, title: { type: 'string', description: 'heading of the note' } },
        events: {},
        children: 'the text of the note (any HTML)',
        example: '<vz-callout tone="warn" title="Estimates">These numbers are from a sample.</vz-callout>',
    },
    {
        name: 'vz-progress',
        description: 'A progress bar with its label.',
        attributes: {
            value: { type: 'number', description: 'progress from 0 to max', required: true },
            max: { type: 'number', description: 'the value that is complete (default 1)' },
            label: { type: 'string', description: 'what is progressing' },
            tone: { type: 'enum', values: ['default', 'error'], description: 'error draws it as failed' },
        },
        events: {},
        example: '<vz-progress label="Import" value="0.4"></vz-progress>',
    },
    {
        name: 'vz-image',
        description: 'An image from the conversation\'s files or an https address.',
        attributes: {
            file: { type: 'string', description: 'the name of a file in this conversation' },
            src: { type: 'string', description: 'an https: or data: address (when there is no file)' },
            alt: { type: 'string', description: 'what the image shows', required: true },
            width: { type: 'number', description: 'width in px' },
            height: { type: 'number', description: 'height in px' },
        },
        events: { load: '{}: the image loaded', error: '{message}: it could not be loaded' },
        example: '<vz-image file="forecast.png" alt="Forecast for the week"></vz-image>',
    },
]

/** Libraries a page may `import` by name in a `<script type="module">` (research R11). */
export const LIBRARIES: readonly { name: string; description: string }[] = [
    { name: 'three', description: '3D graphics: `import * as THREE from "three"`' },
]

/** The manifest as JSON data (the zod schemas left out), for `ui-kit/manifest.json`. */
export function manifestData() {
    return {
        elements: ELEMENTS.map((element) => ({
            ...element,
            attributes: Object.fromEntries(
                Object.entries(element.attributes).map(([name, { schema: _schema, ...attr }]) => [name, attr])
            ),
        })),
        libraries: LIBRARIES.map((lib) => lib.name),
    }
}
