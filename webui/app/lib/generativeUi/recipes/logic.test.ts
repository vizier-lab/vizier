// The recipes' logic, run through the real runner script and host (see logic/vmTransport.ts).
import { describe, expect, it } from 'vitest'
import { LogicHost, type Defs } from '../logic/host'
import { vmTransport } from '../logic/vmTransport'
import { RECIPES } from './index'
import { startingBoard } from './board_game'

const host = (name: string) => new LogicHost((RECIPES[name].spec.logic ?? {}) as Defs, vmTransport, 5000)

type Cell = { g: string; s: string; m: boolean; sel: boolean }
type Game = { board: Cell[]; turn: string; selected: number | null; legal: number[]; status: string; over: boolean }

const start = (): Game => ({ board: startingBoard(), turn: 'White', selected: null, legal: [], status: '', over: false })
const square = (name: string) => (8 - Number(name[1])) * 8 + (name.charCodeAt(0) - 97)

describe('board_game', () => {
    const chess = host('board_game')
    const press = (game: Game, name: string) => chess.call('step', [game, square(name)]) as Promise<Game>
    const move = async (game: Game, from: string, to: string) => press(await press(game, from), to)

    it('starts with 32 pieces, white to move', () => {
        const b = start().board
        expect(b.filter((c) => c.g).length).toBe(32)
        expect(b[square('e1')].g).toBe('♔')
        expect(b[square('e8')].g).toBe('♚')
        expect(b[square('d1')].g).toBe('♕')
    })

    it('selecting a pawn shows its one- and two-square moves', async () => {
        const game = await press(start(), 'e2')
        expect(game.selected).toBe(square('e2'))
        expect(game.legal.sort()).toEqual([square('e3'), square('e4')].sort())
        expect(game.board.filter((c) => c.m).length).toBe(2)
        expect(game.board[square('e2')].sel).toBe(true)
    })

    it('moves a pawn and passes the turn', async () => {
        const game = await move(start(), 'e2', 'e4')
        expect(game.board[square('e4')].g).toBe('♙')
        expect(game.board[square('e2')].g).toBe('')
        expect(game.turn).toBe('Black')
        expect(game.selected).toBeNull()
        expect(game.board.some((c) => c.m || c.sel)).toBe(false)
    })

    it('shows a knight its two opening moves and no moves for a blocked rook', async () => {
        expect((await press(start(), 'g1')).legal.sort()).toEqual([square('f3'), square('h3')].sort())
        expect((await press(start(), 'a1')).legal).toEqual([])
    })

    it('ignores the opponent\'s pieces on your turn', async () => {
        const game = await press(start(), 'e7')
        expect(game.selected).toBeNull()
        expect(game.legal).toEqual([])
    })

    it('refuses an illegal target: the press just chooses again', async () => {
        const selected = await press(start(), 'e2')
        const after = await press(selected, 'e5')
        expect(after.board[square('e5')].g).toBe('')
        expect(after.board[square('e2')].g).toBe('♙')
        expect(after.turn).toBe('White')
        expect(after.selected).toBeNull()
    })

    it('lets a different own piece be chosen instead', async () => {
        const game = await press(await press(start(), 'e2'), 'd2')
        expect(game.selected).toBe(square('d2'))
    })

    it('captures', async () => {
        let g = start()
        g = await move(g, 'e2', 'e4')
        g = await move(g, 'd7', 'd5')
        g = await move(g, 'e4', 'd5')
        expect(g.board[square('d5')].g).toBe('♙')
        expect(g.board.filter((c) => c.g).length).toBe(31)
        expect(g.turn).toBe('Black')
    })

    it('does not let a pinned piece expose its king', async () => {
        const g = start()
        const empty = g.board.map((c) => ({ ...c, g: '' }))
        empty[square('e1')].g = '♔'
        empty[square('e2')].g = '♗'
        empty[square('e8')].g = '♜'
        empty[square('a8')].g = '♚'
        const selected = await press({ ...g, board: empty }, 'e2')
        expect(selected.legal).toEqual([])
    })

    it('calls fool\'s mate checkmate and then stops accepting moves', async () => {
        let g = start()
        g = await move(g, 'f2', 'f3')
        g = await move(g, 'e7', 'e5')
        g = await move(g, 'g2', 'g4')
        g = await move(g, 'd8', 'h4')
        expect(g.status).toBe('Checkmate. Black wins.')
        expect(g.over).toBe(true)
        expect(await press(g, 'e2')).toEqual(g)
    })

    it('says check', async () => {
        let g = start()
        g = await move(g, 'e2', 'e4')
        g = await move(g, 'f7', 'f6')
        g = await move(g, 'd1', 'h5')
        expect(g.status).toBe('Check.')
    })

    it('promotes a pawn that reaches the last rank', async () => {
        const g = start()
        const board = g.board.map((c) => ({ ...c, g: '' }))
        board[square('a7')].g = '♙'
        board[square('e1')].g = '♔'
        board[square('h8')].g = '♚'
        const out = await move({ ...g, board }, 'a7', 'a8')
        expect(out.board[square('a8')].g).toBe('♕')
    })

    it('answers each press well inside the 250 ms limit, even a mating one', async () => {
        const timed = new LogicHost(RECIPES.board_game.spec.logic as Defs, vmTransport, 250)
        let g = start()
        const t = Date.now()
        for (const [from, to] of [['f2', 'f3'], ['e7', 'e5'], ['g2', 'g4'], ['d8', 'h4']]) {
            g = (await timed.call('step', [await timed.call('step', [g, square(from)]), square(to)])) as Game
        }
        expect(g.over).toBe(true)
        expect(Date.now() - t).toBeLessThan(1500)
    })
})

describe('scene_3d', () => {
    const terrain = host('scene_3d')

    it('makes n×n heights between 0 and 1', async () => {
        const h = (await terrain.call('generate', [7, 0.55, 48])) as number[]
        expect(h.length).toBe(48 * 48)
        expect(Math.min(...h)).toBeGreaterThanOrEqual(0)
        expect(Math.max(...h)).toBeLessThanOrEqual(1)
        expect(new Set(h).size).toBeGreaterThan(100)
    })

    it('is the same for the same seed and different for another', async () => {
        const a = await terrain.call('generate', [7, 0.55, 16])
        expect(await terrain.call('generate', [7, 0.55, 16])).toEqual(a)
        expect(await terrain.call('generate', [8, 0.55, 16])).not.toEqual(a)
        expect(await terrain.call('generate', [7, 0.9, 16])).not.toEqual(a)
    })

    it('fits the 250 ms limit at the recipe\'s size', async () => {
        const t = Date.now()
        await terrain.call('generate', [42, 0.7, 48])
        expect(Date.now() - t).toBeLessThan(250)
    })
})

describe('canvas_2d', () => {
    const canvas = host('canvas_2d')

    it('adds a dot where pressed, up to a cap, and drifts them right', async () => {
        const dots = (await canvas.call('addDot', [[], 20, 30])) as { x: number; y: number }[]
        expect(dots).toEqual([{ kind: 'circle', x: 20, y: 30, r: 8, fill: 'accent' }])
        expect(((await canvas.call('addDot', [Array(40).fill(dots[0]), 1, 1])) as unknown[]).length).toBe(40)
        const moved = (await canvas.call('drift', [[{ kind: 'circle', x: 198, y: 5, r: 8, fill: 'accent' }]])) as { x: number }[]
        expect(moved[0].x).toBe(4)
    })
})
