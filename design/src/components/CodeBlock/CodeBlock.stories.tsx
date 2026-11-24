import type { Meta, StoryObj } from "@storybook/react-vite";
import { CodeBlock, type CodeLine } from "./CodeBlock";

const PY = `hits = memory_search(query="vector db", limit=8)
for h in hits:
    print(h["bundle"], h["path"], round(h["score"], 2))`;

const meta = {
  title: "Components/Surfaces & data/CodeBlock",
  component: CodeBlock,
  tags: ["approved"],
  parameters: { vizier: { id: "D41", replaces: null } },
  args: { code: PY, language: "python" },
  decorators: [(Story) => <div className="w-580">{Story()}</div>],
} satisfies Meta<typeof CodeBlock>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Run detail: numbered lines. */
export const LineNumbers: Story = { args: { lineNumbers: true } };

/** A file, named. */
export const Filename: Story = {
  args: {
    filename: "SKILL.md",
    language: "markdown",
    code: "---\nname: code-review\ndescription: Review a diff for bugs, then for clarity.\n---\n\nPoint at lines, never rewrite whole files.",
  },
};

/**
 * Highlighted: what the WebUI passes for a fenced block. One line of tokens per line of source;
 * each tone is an existing token, so this works in both themes without a code palette.
 */
export const Highlighted: Story = {
  args: {
    tokens: [
      [
        { text: "hits", tone: "function" },
        { text: " = " },
        { text: "memory_search", tone: "function" },
        { text: "(", tone: "punctuation" },
        { text: "query", tone: "keyword" },
        { text: "=", tone: "punctuation" },
        { text: '"vector db"', tone: "string" },
        { text: ", ", tone: "punctuation" },
        { text: "limit", tone: "keyword" },
        { text: "=", tone: "punctuation" },
        { text: "8", tone: "number" },
        { text: ")", tone: "punctuation" },
      ],
      [{ text: "# one passage per bundle", tone: "comment" }],
      [{ text: "for ", tone: "keyword" }, { text: "h", tone: "function" }, { text: " in ", tone: "keyword" }, { text: "hits", tone: "function" }, { text: ":", tone: "punctuation" }],
      [
        { text: "    " },
        { text: "print", tone: "function" },
        { text: "(", tone: "punctuation" },
        { text: "h", tone: "function" },
        { text: "[", tone: "punctuation" },
        { text: '"bundle"', tone: "string" },
        { text: "], ", tone: "punctuation" },
        { text: "h", tone: "function" },
        { text: "[", tone: "punctuation" },
        { text: '"score"', tone: "string" },
        { text: "])", tone: "punctuation" },
      ],
    ] satisfies CodeLine[],
  },
};

/**
 * The report this wrapping exists for: an agent's script on one long line. Nothing scrolls —
 * the line wraps inside the frame, in the story and in the activity trail.
 */
export const LongLine: Story = {
  args: {
    language: "execute_python",
    code: 'hits = [h for h in memory_search(query="vector database benchmarks comparison, 2026 edition", limit=50) if h["score"] > 0.8 and h["bundle"] not in ("archive", "scratch")]',
  },
};

/** The skill editor's example diff. */
export const Diff: Story = {
  args: {
    language: "diff",
    code: " def total(items):\n-    return sum(items)\n+    return sum(i.price for i in items)\n",
  },
};
