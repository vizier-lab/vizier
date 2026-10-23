import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import { useState } from "react";
import { MarkdownEditor } from "./MarkdownEditor";
import { FIXTURE } from "./fixture";

const CORE = `# CORE

## Operating Framework
1. Be concise — direct answers, no filler
2. Verify before modify

## Identity
You are Archivist. You keep the team's memory.
`;

const meta = {
  title: "Components/Actions & inputs/MarkdownEditor",
  component: MarkdownEditor,
  tags: ["approved"],
  parameters: {
    vizier: { id: "X76", replaces: "webui/app/components/MarkdownEditor.tsx" },
    docs: {
      description: {
        component:
          "Edits a markdown document in `rich`, `markdown` or `split` mode, under the library's own toolbar, outline and document typography. Markdown goes in and comes out; the `RoundTrip` story pins that the fixture document survives an edit unchanged.",
      },
    },
  },
  args: { value: CORE, onChange: () => {} },
  decorators: [(Story) => <div className="h-520 w-860">{Story()}</div>],
} satisfies Meta<typeof MarkdownEditor>;

export default meta;
type Story = StoryObj<typeof meta>;

function Controlled(props: React.ComponentProps<typeof MarkdownEditor>) {
  const [value, setValue] = useState(props.value);
  return <MarkdownEditor {...props} value={value} onChange={(v) => (setValue(v), props.onChange(v))} />;
}

/** Core: the compact toolbar and a character count. */
export const Compact: Story = { render: (args) => <Controlled {...args} look="compact" /> };

/** The skill editor: every tool, the three modes and the outline. */
export const Full: Story = { args: { value: FIXTURE }, render: (args) => <Controlled {...args} look="full" outline /> };

export const MarkdownMode: Story = { args: { value: FIXTURE, mode: "markdown", look: "full" }, render: (args) => <Controlled {...args} /> };
export const Split: Story = { args: { value: FIXTURE, mode: "split", look: "full" }, render: (args) => <Controlled {...args} /> };
export const ReadOnly: Story = { args: { value: CORE, readOnly: true } };

/** Typing and deleting a character in the rich pane gives back exactly the fixture: nothing is lost. */
export const RoundTrip: Story = {
  args: { value: FIXTURE, look: "full", onChange: fn() },
  render: (args) => <Controlled {...args} />,
  play: async ({ canvasElement, args }) => {
    const doc = await within(canvasElement).findByRole("textbox", { name: "Document" });
    await userEvent.click(doc);
    await userEvent.keyboard("x{Backspace}");
    await waitFor(() => expect(args.onChange).toHaveBeenCalled());
    const last = (args.onChange as ReturnType<typeof fn>).mock.calls.at(-1)?.[0] as string;
    const tidy = (t: string) => t.replace(/\n{3,}/g, "\n\n").trim();
    expect(tidy(last)).toBe(tidy(FIXTURE));
  },
};
