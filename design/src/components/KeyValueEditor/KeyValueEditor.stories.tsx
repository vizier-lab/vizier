import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { KeyValueEditor } from "./KeyValueEditor";

const meta = {
  title: "Components/Actions & inputs/KeyValueEditor",
  component: KeyValueEditor,
  tags: ["approved"],
  parameters: { vizier: { id: "X20", replaces: null } },
  args: { label: "Environment", value: [{ key: "DATABASE_URL", value: "postgres://archivist@db/vizier" }], onChange: () => {} },
  decorators: [(Story) => <div className="w-336">{Story()}</div>],
} satisfies Meta<typeof KeyValueEditor>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Adding an MCP server. */
export const Default: Story = {
  render: function Render(args) {
    const [v, setV] = useState(args.value);
    return <KeyValueEditor {...args} value={v} onChange={setV} />;
  },
};

export const Empty: Story = { args: { value: [] } };
