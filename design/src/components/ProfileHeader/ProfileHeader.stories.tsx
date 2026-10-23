import type { Meta, StoryObj } from "@storybook/react-vite";
import { ProfileHeader } from "./ProfileHeader";

const meta = {
  title: "Components/Integrations & settings/ProfileHeader",
  component: ProfileHeader,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X66", replaces: null } },
  args: { name: "ganesha", role: "superadmin" },
  decorators: [(Story) => <div className="w-720">{Story()}</div>],
} satisfies Meta<typeof ProfileHeader>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
