import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import type { CodeLine } from "../CodeBlock/CodeBlock";
import { CodeEditor } from "./CodeEditor";

const SCRIPT = 'limit = input["limit"]\nthreads = [{"subject": f"Thread {i}"} for i in range(limit)]\n[t["subject"] for t in threads]';

// What a caller's highlighter hands over; the design system only maps tones to colours.
const TOKENS: CodeLine[] = [
  [{ text: "limit = input[" }, { text: '"limit"', tone: "string" }, { text: "]" }],
  [
    { text: "threads = [{" },
    { text: '"subject"', tone: "string" },
    { text: ": " },
    { text: 'f"Thread {i}"', tone: "string" },
    { text: "} " },
    { text: "for", tone: "keyword" },
    { text: " i " },
    { text: "in", tone: "keyword" },
    { text: " " },
    { text: "range", tone: "keyword" },
    { text: "(limit)]" },
  ],
  [{ text: "[t[" }, { text: '"subject"', tone: "string" }, { text: "] " }, { text: "for", tone: "keyword" }, { text: " t " }, { text: "in", tone: "keyword" }, { text: " threads]" }],
];

const meta = {
  title: "Components/Actions & inputs/CodeEditor",
  component: CodeEditor,
  parameters: { vizier: { id: "061-code-editor" } },
  args: { label: "Script", helper: "The arguments are the dict input; the last expression is the result.", value: SCRIPT, onChange: () => {} },
  decorators: [(Story) => <div className="w-560">{Story()}</div>],
} satisfies Meta<typeof CodeEditor>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Highlighted, as the WebUI draws a function's script. Tokens only match the first value. */
export const Highlighted: Story = {
  render: function Render(args) {
    const [v, setV] = useState(SCRIPT);
    return <CodeEditor {...args} value={v} onChange={setV} tokens={v === SCRIPT ? TOKENS : undefined} />;
  },
};

/** No tokens: plain mono, like a mono Textarea. */
export const Plain: Story = {
  render: function Render(args) {
    const [v, setV] = useState('{\n  "type": "object"\n}');
    return <CodeEditor {...args} label="Input schema (JSON)" helper={undefined} value={v} onChange={setV} />;
  },
};

export const Invalid: Story = { args: { invalid: true, helper: "line 2, column 40: expected ')'" } };
