import type { Meta, StoryObj } from "@storybook/react-vite";
import { Legend } from "./Legend";

const meta = {
  title: "Components/Visualisation/Legend",
  component: Legend,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X72", replaces: null } },
  args: {
    items: [
      { label: "Chat", colour: "ink" },
      { label: "Dreams", colour: "chart-1" },
      { label: "Compaction", colour: "chart-2" },
      { label: "Vision", colour: "chart-3" },
      { label: "Background", colour: "chart-4" },
      { label: "Scheduled", colour: "chart-5" },
    ],
  },
} satisfies Meta<typeof Legend>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The Usage activities. */
export const Default: Story = {};

export const WithValues: Story = {
  args: {
    items: [
      { label: "Chat", colour: "ink", value: "$2.41" },
      { label: "Dreams", colour: "chart-1", value: "$0.88" },
      { label: "Compaction", colour: "chart-2", value: "$0.31" },
    ],
  },
};
