// lint-tokens-exempt: chess glyphs and source text for a recipe; no colour values.
import type { Recipe } from './index'

// Pieces are glyphs; each board cell is {g: glyph, s: button variant (the square's shade),
// m: a legal target of the selection, sel: the selected square}. The logic keeps that shape so the
// page can style a cell from its own fields with no arithmetic.

const SIDE = `
function side(g) { return '♔♕♖♗♘♙'.indexOf(g) >= 0 && g ? 'w' : '♚♛♜♝♞♟'.indexOf(g) >= 0 && g ? 'b' : ''; }
`

/** Where a piece can go ignoring check: slides, jumps, pawn pushes and captures. */
const REACH = `${SIDE}
var W = '♔♕♖♗♘♙', B = '♚♛♜♝♞♟', T = 'kqrbnp';
function kind(g) { var i = W.indexOf(g); if (i < 0) i = B.indexOf(g); return T.charAt(i); }
var g = board[from].g;
if (!g) return [];
var me = side(g), t = kind(g), r = Math.floor(from / 8), c = from % 8, out = [];
function add(rr, cc, capOnly, moveOnly) {
  if (rr < 0 || rr > 7 || cc < 0 || cc > 7) return false;
  var q = board[rr * 8 + cc].g;
  if (side(q) === me) return false;
  if (capOnly && !q) return false;
  if (moveOnly && q) return false;
  out.push(rr * 8 + cc);
  return !q;
}
function ray(dr, dc) { for (var k = 1; k < 8; k++) { if (!add(r + dr * k, c + dc * k)) break; } }
if (t === 'p') {
  var d = me === 'w' ? -1 : 1;
  if (add(r + d, c, false, true) && r === (me === 'w' ? 6 : 1)) add(r + 2 * d, c, false, true);
  add(r + d, c - 1, true, false);
  add(r + d, c + 1, true, false);
} else if (t === 'n') {
  [[-2, -1], [-2, 1], [-1, -2], [-1, 2], [1, -2], [1, 2], [2, -1], [2, 1]].forEach(function (m) { add(r + m[0], c + m[1]); });
} else if (t === 'k') {
  for (var a = -1; a <= 1; a++) for (var b = -1; b <= 1; b++) if (a || b) add(r + a, c + b);
} else {
  if (t !== 'b') { ray(1, 0); ray(-1, 0); ray(0, 1); ray(0, -1); }
  if (t !== 'r') { ray(1, 1); ray(1, -1); ray(-1, 1); ray(-1, -1); }
}
return out;
`

/** reach, minus the moves that leave the mover's own king attacked. */
const LEGAL_MOVES = `${SIDE}
var g = board[from].g;
if (!g) return [];
var me = side(g);
return fns.reach(board, from).filter(function (to) {
  var nb = board.map(function (cell) { return { g: cell.g }; });
  nb[to].g = nb[from].g;
  nb[from].g = '';
  var king = -1;
  for (var i = 0; i < 64; i++) if (nb[i].g === (me === 'w' ? '♔' : '♚')) king = i;
  if (king < 0) return true;
  for (var j = 0; j < 64; j++) if (nb[j].g && side(nb[j].g) !== me && fns.reach(nb, j).indexOf(king) >= 0) return false;
  return true;
});
`

/** One press: select a piece, move the selected one, or change the selection. Returns the new game. */
const STEP = `${SIDE}
if (game.over) return game;
var board = game.board, turn = game.turn, mover = turn === 'White' ? 'w' : 'b';
function marked(sel, legal) {
  return board.map(function (c, i) { return { g: c.g, s: c.s, m: legal.indexOf(i) >= 0, sel: i === sel }; });
}
function choose() {
  if (side(board[index].g) !== mover) return { board: marked(null, []), turn: turn, selected: null, legal: [], status: game.status, over: false };
  var legal = fns.legalMoves(board, index);
  return { board: marked(index, legal), turn: turn, selected: index, legal: legal, status: game.status, over: false };
}
if (game.selected === null || game.selected === undefined) return choose();
if (game.legal.indexOf(index) < 0) return choose();

var piece = board[game.selected].g;
var nb = board.map(function (c) { return { g: c.g, s: c.s, m: false, sel: false }; });
nb[index].g = piece;
nb[game.selected].g = '';
if (piece === '♙' && index < 8) nb[index].g = '♕';
if (piece === '♟' && index >= 56) nb[index].g = '♛';

var next = mover === 'w' ? 'b' : 'w';
var movable = false;
for (var i = 0; i < 64 && !movable; i++) if (side(nb[i].g) === next && fns.legalMoves(nb, i).length) movable = true;
var king = -1;
for (var k = 0; k < 64; k++) if (nb[k].g === (next === 'w' ? '♔' : '♚')) king = k;
var check = false;
for (var j = 0; j < 64 && !check; j++) if (side(nb[j].g) === mover && fns.reach(nb, j).indexOf(king) >= 0) check = true;
var status = movable ? (check ? 'Check.' : '') : check ? 'Checkmate. ' + turn + ' wins.' : 'Stalemate.';
return { board: nb, turn: movable ? (turn === 'White' ? 'Black' : 'White') : turn, selected: null, legal: [], status: status, over: !movable };
`

const BACK = ['♜', '♞', '♝', '♛', '♚', '♝', '♞', '♜']

/** The starting position as board cells, shaded like a board (row 0 is rank 8). */
export function startingBoard() {
    const pieces = [...BACK, ...Array(8).fill('♟'), ...Array(32).fill(''), ...Array(8).fill('♙'), ...BACK.map((g) => '♔♕♖♗♘♙'['♚♛♜♝♞♟'.indexOf(g)])]
    return pieces.map((g, i) => ({ g, s: (Math.floor(i / 8) + (i % 8)) % 2 ? 'secondary' : 'ghost', m: false, sel: false }))
}

export const boardGame: Recipe = {
    intent: 'A board game played in the interface itself, with the rules in logic functions: select a piece, then a square; no round trip to you per move.',
    notes: [
        'Each press runs the logic function "step" locally and writes the new game into /game. The rules live in logic; the page only draws cells.',
        'A cell is {g, s, m, sel}: glyph, square shade, legal-target mark, selected mark. Buttons style themselves from their own cell with $item.',
        'This chess has no castling or en passant. Adapt the logic for other games; keep functions pure and under 250 ms.',
        'The "Ask the agent" button sends the position to you as the person\'s message; you can answer in text or with a replacement interface.',
    ],
    spec: {
        root: 'page',
        state: { game: { board: startingBoard(), turn: 'White', selected: null, legal: [], status: '', over: false } },
        logic: {
            reach: { params: ['board', 'from'], body: REACH },
            legalMoves: { params: ['board', 'from'], body: LEGAL_MOVES },
            step: { params: ['game', 'index'], body: STEP },
        },
        elements: {
            page: { type: 'Stack', props: { gap: 'md' }, children: ['status', 'board', 'ask'] },
            status: { type: 'Text', props: { text: { $template: '**${/game/turn}** to move. ${/game/status}' } } },
            board: { type: 'Grid', props: { cols: 8, gap: 'none', maxWidth: 400 }, repeat: { statePath: '/game/board' }, children: ['cell'] },
            cell: {
                type: 'Button',
                props: {
                    label: { $item: 'g' },
                    variant: { $item: 's' },
                    tone: { $cond: { $item: 'm' }, $then: 'accent', $else: 'default' },
                    selected: { $item: 'sel' },
                    square: true,
                },
                on: { press: { action: 'run', params: { fn: 'step', args: [{ $state: '/game' }, { $index: true }], into: '/game' } } },
            },
            ask: {
                type: 'Button',
                props: { label: 'Ask the agent for a move', variant: 'secondary' },
                on: { press: { action: 'sendMessage', params: { text: 'Your move', data: { $state: '/game' } } } },
            },
        },
    },
}
