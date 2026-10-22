import type { Meta, StoryObj } from "@storybook/react-vite";
import { Avatar } from "./Avatar";

const meta = {
  title: "Components/Surfaces & data/Avatar",
  component: Avatar,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D40", replaces: "webui/app/components/avatar.tsx" } },
  args: { name: "ganesha", size: "md" },
  argTypes: { size: { control: "inline-radio", options: ["sm", "md", "lg"] } },
} satisfies Meta<typeof Avatar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Sm in lists, Md in menus, Lg on the profile header. */
export const Sizes: Story = {
  render: () => (
    <div className="flex items-end gap-16">
      <Avatar name="ganesha" size="sm" />
      <Avatar name="ganesha" size="md" />
      <Avatar name="ganesha" size="lg" />
    </div>
  ),
};
