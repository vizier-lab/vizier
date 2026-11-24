import type { OutlineItem } from "../OutlineNav/OutlineNav";

/** The `#`, `##` and `###` headings of a markdown document, skipping fenced code. */
export function headingsOf(markdown: string): (OutlineItem & { line: number })[] {
  const items: (OutlineItem & { line: number })[] = [];
  let fenced = false;
  markdown.split("\n").forEach((text, line) => {
    if (/^\s*(```|~~~)/.test(text)) fenced = !fenced;
    if (fenced) return;
    const m = /^(#{1,3})\s+(.+?)\s*#*\s*$/.exec(text);
    if (m) items.push({ id: `h${items.length}`, label: m[2], level: m[1].length as 1 | 2 | 3, line });
  });
  return items;
}

/** The character offset where `line` (0-based) starts. */
export function offsetOfLine(markdown: string, line: number): number {
  let offset = 0;
  const lines = markdown.split("\n");
  for (let i = 0; i < line && i < lines.length; i++) offset += lines[i].length + 1;
  return offset;
}

/** Wraps the selection (or inserts a placeholder), returning the new text and selection. */
export function wrapSelection(
  text: string,
  start: number,
  end: number,
  before: string,
  after: string,
  placeholder: string,
): { text: string; start: number; end: number } {
  const selected = text.slice(start, end) || placeholder;
  const next = text.slice(0, start) + before + selected + after + text.slice(end);
  return { text: next, start: start + before.length, end: start + before.length + selected.length };
}

/** Prefixes every line the selection touches. `prefix(i)` gives line i's prefix. */
export function prefixLines(
  text: string,
  start: number,
  end: number,
  prefix: (i: number) => string,
): { text: string; start: number; end: number } {
  const from = text.lastIndexOf("\n", start - 1) + 1;
  const toIdx = text.indexOf("\n", end);
  const to = toIdx === -1 ? text.length : toIdx;
  const block = text
    .slice(from, to)
    .split("\n")
    .map((l, i) => prefix(i) + l)
    .join("\n");
  return { text: text.slice(0, from) + block + text.slice(to), start: from, end: from + block.length };
}

export const TABLE_TEMPLATE = "| Column | Column |\n| --- | --- |\n| Cell | Cell |\n";
