import type { Meta, StoryObj } from "@storybook/react-vite";
import { OriginBadge } from "./OriginBadge";

const meta = {
  title: "Components/Chat/OriginBadge",
  component: OriginBadge,
  tags: ["approved"],
  parameters: { vizier: { id: "X62", replaces: null } },
  args: { kind: "discord", label: "Discord" },
  argTypes: { kind: { control: "inline-radio", options: ["task", "chat", "discord", "telegram", "workspace"] } },
} satisfies Meta<typeof OriginBadge>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Kinds: Story = {
  render: () => (
    <div className="flex flex-wrap items-center gap-8">
      <OriginBadge kind="task" label="daily-report" />
      <OriginBadge kind="chat" label="a direct chat" />
      <OriginBadge kind="discord" label="Discord" />
      <OriginBadge kind="telegram" label="Telegram" />
      <OriginBadge kind="workspace" label="design-review" />
    </div>
  ),
};
