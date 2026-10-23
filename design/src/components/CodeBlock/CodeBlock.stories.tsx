import type { Meta, StoryObj } from "@storybook/react-vite";
import { CodeBlock } from "./CodeBlock";

const PY = `hits = memory_search(query="vector db", limit=8)
for h in hits:
    print(h["bundle"], h["path"], round(h["score"], 2))`;

const meta = {
  title: "Components/Surfaces & data/CodeBlock",
  component: CodeBlock,
  tags: ["needs-review"],
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

/** The skill editor's example diff. */
export const Diff: Story = {
  args: {
    language: "diff",
    code: " def total(items):\n-    return sum(items)\n+    return sum(i.price for i in items)\n",
  },
};
