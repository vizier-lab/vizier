import type { Recipe } from './index'

const ADD_DOT = `
var next = dots.slice();
if (next.length < 40) next.push({ kind: 'circle', x: x, y: y, r: 8, fill: 'accent' });
return next;
`

const DRIFT = `
return dots.map(function (d) { return { kind: d.kind, x: (d.x + 6) % 200, y: d.y, r: d.r, fill: d.fill }; });
`

export const canvas2d: Recipe = {
    intent: 'A drawing the person can click, that animates by itself: shapes held in state, changed by logic.',
    notes: [
        'Canvas2D draws its shapes from state; press reports {x, y, id?} in the drawing\'s own units, readable as {"$state":"/_event/x"}.',
        'The Effect ticks every 500 ms while the interface is on screen and the person is about; a tick can change state but cannot send messages.',
        'Shapes: rect {x,y,w,h}, circle {x,y,r}, line {x1,y1,x2,y2}, path {d}, text {x,y,text,size}; colours from the closed list or #rrggbb.',
    ],
    spec: {
        root: 'page',
        state: { dots: [{ kind: 'circle', x: 30, y: 50, r: 8, fill: 'accent' }] },
        logic: {
            addDot: { params: ['dots', 'x', 'y'], body: ADD_DOT },
            drift: { params: ['dots'], body: DRIFT },
        },
        elements: {
            page: { type: 'Stack', props: { gap: 'sm' }, children: ['help', 'canvas', 'clock'] },
            help: { type: 'Text', props: { text: 'Click the drawing to add a dot.', tone: 'muted', size: 'sm' } },
            canvas: {
                type: 'Canvas2D',
                props: { width: 200, height: 100, background: 'surface', shapes: { $state: '/dots' } },
                on: {
                    press: { action: 'run', params: { fn: 'addDot', args: [{ $state: '/dots' }, { $state: '/_event/x' }, { $state: '/_event/y' }], into: '/dots' } },
                },
            },
            clock: { type: 'Effect', props: { intervalMs: 500 }, on: { tick: { action: 'run', params: { fn: 'drift', args: [{ $state: '/dots' }], into: '/dots' } } } },
        },
    },
}
