import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { JobsPill } from "./JobsPill";

const meta = {
  title: "Components/Chat/JobsPill",
  component: JobsPill,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D60", replaces: "webui/app/components/BackgroundJobTray.tsx" } },
  args: { count: 1 },
} satisfies Meta<typeof JobsPill>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Several: Story = { args: { count: 3 } };

/** Mobile: icon and count. */
export const Compact: Story = { args: { count: 3, compact: true } };

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  render: () => <StateGrid rows={[{ cells: interactionCells(() => <JobsPill count={1} />).filter((c) => c.label !== "Pressed") }]} />,
};
