// The pure functions an interface may use in `{ "$computed": name, "args": {...} }`. Each takes
// the resolved args and returns a value, or `null` when the args don't fit: nothing throws, so
// a bad expression shows as an empty prop, never a broken page.

type Args = Record<string, unknown>;
type Fn = (args: Args) => unknown;

const num = (value: unknown): number | null => (typeof value === "number" && Number.isFinite(value) ? value : null);
const list = (value: unknown): unknown[] | null => (Array.isArray(value) ? value : null);

/** Both operands as numbers (`a`, `b`), or null. */
function two(args: Args): [number, number] | null {
  const a = num(args.a);
  const b = num(args.b);
  return a === null || b === null ? null : [a, b];
}

const safe =
  (fn: Fn): Fn =>
  (args) => {
    try {
      const out = fn(args ?? {});
      return typeof out === "number" && !Number.isFinite(out) ? null : (out ?? null);
    } catch {
      return null;
    }
  };

export const COMPUTED_FUNCTIONS: Record<string, Fn> = Object.fromEntries(
  Object.entries({
    add: (args: Args) => two(args) && two(args)![0] + two(args)![1],
    sub: (args: Args) => two(args) && two(args)![0] - two(args)![1],
    mul: (args: Args) => two(args) && two(args)![0] * two(args)![1],
    div: (args: Args) => {
      const p = two(args);
      return p && p[1] !== 0 ? p[0] / p[1] : null;
    },
    mod: (args: Args) => {
      const p = two(args);
      return p && p[1] !== 0 ? p[0] % p[1] : null;
    },
    min: (args: Args) => {
      const p = two(args);
      return p && Math.min(...p);
    },
    max: (args: Args) => {
      const p = two(args);
      return p && Math.max(...p);
    },
    abs: (args: Args) => {
      const v = num(args.value);
      return v === null ? null : Math.abs(v);
    },
    round: (args: Args) => {
      const v = num(args.value);
      if (v === null) return null;
      const digits = num(args.digits) ?? 0;
      const f = 10 ** Math.max(0, Math.min(10, digits));
      return Math.round(v * f) / f;
    },
    floor: (args: Args) => {
      const v = num(args.value);
      return v === null ? null : Math.floor(v);
    },
    eq: (args: Args) => args.a === args.b,
    not: (args: Args) => !args.value,
    and: (args: Args) => Boolean(args.a) && Boolean(args.b),
    or: (args: Args) => Boolean(args.a) || Boolean(args.b),
    len: (args: Args) => {
      const v = args.value;
      return typeof v === "string" || Array.isArray(v) ? v.length : null;
    },
    at: (args: Args) => {
      const l = list(args.list);
      const i = num(args.index);
      return l && i !== null ? (l[i < 0 ? l.length + i : i] ?? null) : null;
    },
    concat: (args: Args) => {
      const a = args.a;
      const b = args.b;
      if (Array.isArray(a) && Array.isArray(b)) return [...a, ...b];
      return `${a ?? ""}${b ?? ""}`;
    },
    join: (args: Args) => {
      const l = list(args.list);
      return l ? l.map((v) => String(v ?? "")).join(typeof args.separator === "string" ? args.separator : ", ") : null;
    },
    slice: (args: Args) => {
      const v = args.value;
      const start = num(args.start) ?? 0;
      const end = num(args.end) ?? undefined;
      return Array.isArray(v) || typeof v === "string" ? v.slice(start, end) : null;
    },
    includes: (args: Args) => {
      const l = list(args.list);
      if (l) return l.includes(args.value);
      return typeof args.list === "string" && typeof args.value === "string" ? args.list.includes(args.value) : false;
    },
    range: (args: Args) => {
      const n = num(args.count);
      if (n === null || n < 0 || n > 10_000) return null;
      const start = num(args.start) ?? 0;
      return Array.from({ length: Math.floor(n) }, (_, i) => start + i);
    },
  }).map(([name, fn]) => [name, safe(fn)]),
);

export const FUNCTION_NAMES = Object.keys(COMPUTED_FUNCTIONS);
