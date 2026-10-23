import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { KeywordInput } from "./KeywordInput";

const meta = {
  title: "Components/Actions & inputs/KeywordInput",
  component: KeywordInput,
  tags: ["approved"],
  parameters: { vizier: { id: "X18", replaces: null } },
  args: { label: "Keywords", value: ["review", "quality", "security"], onChange: () => {} },
  decorators: [(Story) => <div className="w-271">{Story()}</div>],
} satisfies Meta<typeof KeywordInput>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The skill editor's keywords. Click + Add, type, Enter. */
export const Default: Story = {
  render: function Render(args) {
    const [v, setV] = useState(args.value);
    return <KeywordInput {...args} value={v} onChange={setV} />;
  },
};

export const Empty: Story = { args: { value: [] } };
