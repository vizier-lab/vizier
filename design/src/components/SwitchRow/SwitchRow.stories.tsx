import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { SwitchRow } from "./SwitchRow";

const meta = {
  title: "Components/Actions & inputs/SwitchRow",
  component: SwitchRow,
  tags: ["approved"],
  parameters: { vizier: { id: "X16", replaces: "webui/app/components/SelfManagementSection.tsx" } },
  args: { label: "MCP servers", badge: "granted", helper: "Archivist may add, change and remove its own MCP servers.", checked: true, onCheckedChange: () => {} },
  decorators: [(Story) => <div className="w-334">{Story()}</div>],
} satisfies Meta<typeof SwitchRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: function Render(args) {
    const [on, setOn] = useState(args.checked);
    return <SwitchRow {...args} checked={on} onCheckedChange={setOn} />;
  },
};

export const Off: Story = { args: { checked: false, badge: undefined } };

/** Disabled, with the reason. */
export const DisabledWithHelper: Story = {
  args: { label: "Ask for a spoken reply", badge: undefined, checked: false, disabled: true, helper: "Turn on Text-to-Speech in Config first." },
};
