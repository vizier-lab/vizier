import type { Meta, StoryObj } from "@storybook/react-vite";
import { PanelHeader } from "./PanelHeader";

const meta = {
  title: "Components/Layout/PanelHeader",
  component: PanelHeader,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X2", replaces: null } },
  args: { title: "Recalled memory", meta: "4 passages" },
  decorators: [(Story) => <div className="w-280">{Story()}</div>],
} satisfies Meta<typeof PanelHeader>;

export default meta;
type Story = StoryObj<typeof meta>;

export const WithCount: Story = {};

export const WithClose: Story = { args: { title: "Background jobs", meta: "2 running", onClose: () => {} } };

export const WithAction: Story = {
  args: { title: "History", meta: undefined, action: <button type="button" className="type-small-medium text-fg hover:underline">Compare</button> },
};
