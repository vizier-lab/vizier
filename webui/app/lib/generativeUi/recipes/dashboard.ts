import type { Recipe } from './index'

export const dashboard: Recipe = {
    intent: 'A summary page: headline numbers, a chart and a table, all static.',
    notes: [
        'Change the numbers, labels and rows; keep the structure.',
        'Grid minCol makes the stats wrap on a narrow screen.',
        'Chart kind can be bar, line, area or pie; add "series" to each datum to compare several.',
    ],
    spec: {
        root: 'page',
        elements: {
            page: { type: 'Stack', props: { gap: 'md' }, children: ['title', 'stats', 'chart', 'table'] },
            title: { type: 'Heading', props: { text: 'Weekly report', level: 2 } },
            stats: { type: 'Grid', props: { cols: 3, gap: 'md', minCol: 160 }, children: ['replies', 'errors', 'latency'] },
            replies: { type: 'Stat', props: { label: 'Replies', value: 1284, delta: '+4.2%', tone: 'ok' } },
            errors: { type: 'Stat', props: { label: 'Errors', value: 7, delta: '-2', tone: 'ok' } },
            latency: { type: 'Stat', props: { label: 'Median latency', value: 840, delta: '+60 ms', tone: 'warn' } },
            chart: { type: 'Chart', props: { kind: 'bar', height: 180, data: [{ label: 'Mon', value: 12 }, { label: 'Tue', value: 19 }, { label: 'Wed', value: 7 }, { label: 'Thu', value: 24 }, { label: 'Fri', value: 15 }] } },
            table: {
                type: 'Table',
                props: {
                    columns: [{ key: 'agent', label: 'Agent' }, { key: 'replies', label: 'Replies', align: 'end' }],
                    rows: [{ agent: 'Vizier', replies: 842 }, { agent: 'Scout', replies: 442 }],
                },
            },
        },
    },
}
