/**
 * Agent sigils: a mirror-symmetric 5×5 dot pattern drawn from the agent id, so
 * an agent looks the same everywhere.
 *
 * The rule:
 * 1. Hash the id with 32-bit FNV-1a.
 * 2. Read 15 bits, lowest first, into the left three columns, row by row
 *    (row 0: columns 0, 1, 2; row 1: …). A set bit is a lit dot.
 * 3. Mirror column 0 onto column 4 and column 1 onto column 3.
 * 4. If fewer than 5 or more than 19 dots are lit, the pattern reads as empty
 *    or as a block: hash the id again with "#" appended, and repeat.
 *
 * Changing this rule changes every agent's sigil, so treat it as fixed.
 */

const FNV_OFFSET = 0x811c9dc5;
const FNV_PRIME = 0x01000193;

function fnv1a(text: string): number {
  let hash = FNV_OFFSET;
  for (const byte of new TextEncoder().encode(text)) {
    hash ^= byte;
    hash = Math.imul(hash, FNV_PRIME) >>> 0;
  }
  return hash;
}

export const SIGIL_MIN_LIT = 5;
export const SIGIL_MAX_LIT = 19;

/** The 25 cells of an agent's sigil, row-major; `true` is lit. */
export function sigilCells(agentId: string): boolean[] {
  let seed = agentId;
  for (;;) {
    const bits = fnv1a(seed);
    const cells: boolean[] = [];
    for (let row = 0; row < 5; row++) {
      const left = [0, 1, 2].map((col) => ((bits >>> (row * 3 + col)) & 1) === 1);
      cells.push(left[0], left[1], left[2], left[1], left[0]);
    }
    const lit = cells.filter(Boolean).length;
    if (lit >= SIGIL_MIN_LIT && lit <= SIGIL_MAX_LIT) return cells;
    seed += "#";
  }
}

/**
 * The cell that carries the live signal: the lit cell nearest the top right,
 * matching the mark, whose top-right dot is the one that turns live.
 */
export function sigilSignalCell(cells: boolean[]): number {
  let best = -1;
  let bestScore = -Infinity;
  cells.forEach((lit, i) => {
    if (!lit) return;
    const score = (i % 5) - Math.floor(i / 5);
    if (score > bestScore) [best, bestScore] = [i, score];
  });
  return best;
}
