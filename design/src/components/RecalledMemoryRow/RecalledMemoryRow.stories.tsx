import type { Meta, StoryObj } from "@storybook/react-vite";
import { PanelHeader } from "../PanelHeader/PanelHeader";
import { RecalledMemoryRow } from "./RecalledMemoryRow";

const meta = {
  title: "Components/Lists/RecalledMemoryRow",
  component: RecalledMemoryRow,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X29", replaces: null } },
  args: { path: "qdrant.md#3", bundle: "infra", score: 0.82 },
  decorators: [(Story) => <div className="w-280">{Story()}</div>],
} satisfies Meta<typeof RecalledMemoryRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** The chat side panel. */
export const Panel: Story = {
  render: () => (
    <div className="flex flex-col gap-12">
      <PanelHeader title="Recalled memory" meta="4 passages" />
      <div className="flex flex-col gap-4">
        <RecalledMemoryRow path="qdrant.md#3" bundle="infra" score={0.82} href="#" />
        <RecalledMemoryRow path="sqlite-vec.md#1" bundle="infra" score={0.77} href="#" />
        <RecalledMemoryRow path="decisions/2026-09.md#4" bundle="journal" score={0.64} href="#" />
        <RecalledMemoryRow path="ganesha.md#2" bundle="people" score={0.61} href="#" />
      </div>
    </div>
  ),
};
