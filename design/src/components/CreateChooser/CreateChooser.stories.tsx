import type { Meta, StoryObj } from "@storybook/react-vite";
import { CreateChooser } from "./CreateChooser";

const meta = {
  title: "Components/Overlays/CreateChooser",
  component: CreateChooser,
  tags: ["approved"],
  parameters: { vizier: { id: "X63", replaces: null } },
  args: { open: true, onWorkspace: () => {}, onAgent: () => {}, onClose: () => {} },
} satisfies Meta<typeof CreateChooser>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
