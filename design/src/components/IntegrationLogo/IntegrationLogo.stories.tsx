import type { Meta, StoryObj } from "@storybook/react-vite";
import { IntegrationLogo } from "./IntegrationLogo";

const meta = {
  title: "Components/Integrations & settings/IntegrationLogo",
  component: IntegrationLogo,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X60", replaces: null } },
  args: { kind: "discord", size: 44 },
  argTypes: { kind: { control: "inline-radio", options: ["discord", "telegram", "brave_search", "mcp"] } },
} satisfies Meta<typeof IntegrationLogo>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const All: Story = {
  render: () => (
    <div className="flex flex-col gap-16">
      {[44, 28].map((size) => (
        <div key={size} className="flex items-center gap-12">
          {["discord", "telegram", "brave_search", "mcp"].map((k) => (
            <IntegrationLogo key={k} kind={k} size={size} />
          ))}
        </div>
      ))}
    </div>
  ),
};
