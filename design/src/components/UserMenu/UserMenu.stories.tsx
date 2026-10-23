import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, statePseudo } from "../../stories/StateGrid";
import { UserMenu } from "./UserMenu";

const meta = {
  title: "Components/Shell/UserMenu",
  component: UserMenu,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D5", replaces: null } },
  args: { name: "ganesha", role: "superadmin", onSignOut: () => {}, onOpenProfile: () => {} },
  decorators: [(Story) => <div className="w-212 bg-sidebar">{Story()}</div>],
} satisfies Meta<typeof UserMenu>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Collapsed: Story = { args: { collapsed: true }, decorators: [(Story) => <div className="w-64 px-10">{Story()}</div>] };

export const States: Story = {
  parameters: statePseudo,
  render: (args) => (
    <StateGrid
      rows={[
        {
          cells: [
            { label: "Default", node: <div className="w-212"><UserMenu {...args} /></div> },
            { label: "Hover", pseudo: "hover", node: <div className="w-212"><UserMenu {...args} /></div> },
          ],
        },
      ]}
    />
  ),
};
