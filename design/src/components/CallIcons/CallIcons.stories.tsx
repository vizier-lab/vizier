import type { Meta, StoryObj } from "@storybook/react-vite";
import { CallIcons, type CallIconItem } from "./CallIcons";

const memory: CallIconItem = { key: "memory", label: "Memory", glyph: { icon: "memory" }, count: 48 };
const web: CallIconItem = { key: "web", label: "Web", glyph: { icon: "globe" }, count: 31 };
const scratch: CallIconItem = { key: "scratchpad", label: "Scratchpad", glyph: { icon: "notebook-pen" }, count: 12 };
const github: CallIconItem = { key: "mcp:github", label: "GitHub (MCP)", glyph: { brand: "github" }, count: 5 };
const functions: CallIconItem = {
  key: "functions",
  label: "Functions",
  glyph: { icon: "functions" },
  count: 2,
  details: [{ label: "tidy_tags", count: 2 }],
};
const time: CallIconItem = { key: "time", label: "Time", glyph: { icon: "clock" }, count: 2 };
const acme: CallIconItem = { key: "mcp:acme_internal", label: "acme_internal (MCP)", glyph: { letter: "a" }, count: 3 };

const meta = {
  title: "Components/Chat/CallIcons",
  component: CallIcons,
  tags: ["approved"],
  parameters: { vizier: { id: "X93", replaces: null } },
  args: { items: [memory, web], total: 79, duration: "1.9s" },
  decorators: [(Story) => <div className="flex w-320 justify-end rounded-lg bg-surface p-12 text-faint">{Story()}</div>],
} satisfies Meta<typeof CallIcons>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Two categories, most-used first. Hover or press for the list. */
export const Two: Story = {};

/** Six categories: three glyphs, then `+3`. The popover lists every one, functions by name. */
export const Overflow: Story = { args: { items: [memory, web, scratch, github, functions, time], total: 100, duration: "12.8s" } };

/** A call in GitHub failed: its glyph is red, and it stays among the three shown. */
export const Failed: Story = { args: { items: [memory, web, scratch, { ...github, failed: true }], total: 96 } };

/** An MCP server nothing recognises: the plug with its first letter. */
export const Letter: Story = { args: { items: [acme, memory], total: 7 } };

/** A phone row: one glyph and a count of the rest. */
export const Compact: Story = { args: { items: [memory, web, scratch, github, functions, time], compact: true, total: 100 } };

/** A phone row with a failure: the failed glyph takes the one place. */
export const CompactFailed: Story = { args: { items: [memory, { ...github, failed: true }], compact: true, total: 53 } };

/** No tool calls: an empty slot the same width, so times still line up. */
export const Empty: Story = { args: { items: [] } };
