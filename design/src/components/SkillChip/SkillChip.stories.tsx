import type { Meta, StoryObj } from "@storybook/react-vite";
import { SkillChip } from "./SkillChip";

const meta = {
  title: "Components/Chat/SkillChip",
  component: SkillChip,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D56", replaces: null } },
  args: { skill: "code-review" },
} satisfies Meta<typeof SkillChip>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** On a message bubble's surface. */
export const OnMessage: Story = {
  render: (args) => (
    <div className="flex max-w-480 flex-col items-start gap-6 rounded-xl bg-surface px-14 py-10">
      <SkillChip {...args} />
      <p className="type-body text-fg">Please look at the last commit on vector-db-review.</p>
    </div>
  ),
};
