import type { Recipe } from './index'

export const tabsAndFilter: Recipe = {
    intent: 'Tabs that show one panel at a time, and a list filtered by a dropdown.',
    notes: [
        'Each panel is shown by a visible condition on the tab id in state; Tabs writes the chosen id with $bindState.',
        'A repeat renders its children once per item; inside it {"$item":"field"} reads a field of the item.',
        'The filter is a visible on the repeated element comparing the item to the Select\'s value ("all" shows everything).',
    ],
    spec: {
        root: 'page',
        state: {
            tab: 'list',
            filter: 'all',
            items: [
                { name: 'Draft the report', kind: 'work', done: false },
                { name: 'Book the dentist', kind: 'home', done: false },
                { name: 'Review the PR', kind: 'work', done: true },
                { name: 'Water the plants', kind: 'home', done: true },
            ],
        },
        elements: {
            page: { type: 'Stack', props: { gap: 'md' }, children: ['tabs', 'list-panel', 'about-panel'] },
            tabs: {
                type: 'Tabs',
                props: { tabs: [{ id: 'list', label: 'Tasks' }, { id: 'about', label: 'About' }], value: { $bindState: '/tab' } },
            },
            'list-panel': { type: 'Stack', props: { gap: 'sm' }, visible: { $state: '/tab', eq: 'list' }, children: ['filter', 'rows'] },
            filter: { type: 'Select', props: { label: 'Show', value: { $bindState: '/filter' }, options: ['all', 'work', 'home'] } },
            rows: {
                type: 'Stack',
                props: { gap: 'sm' },
                repeat: { statePath: '/items' },
                children: ['row'],
            },
            row: {
                type: 'Row',
                props: { gap: 'sm', justify: 'between' },
                visible: { $or: [{ $state: '/filter', eq: 'all' }, { $item: 'kind', eq: { $state: '/filter' } }] },
                children: ['row-name', 'row-kind'],
            },
            'row-name': { type: 'Text', props: { text: { $item: 'name' } } },
            'row-kind': { type: 'Badge', props: { text: { $item: 'kind' } } },
            'about-panel': { type: 'Text', props: { text: 'Tabs and filters change what is shown without asking the agent.' }, visible: { $state: '/tab', eq: 'about' } },
        },
    },
}
