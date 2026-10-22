import type { Meta, StoryObj } from "@storybook/react-vite";
import { TableHeader } from "./TableHeader";

const meta = {
  title: "Components/Surfaces & data/TableHeader",
  component: TableHeader,
  tags: ["needs-review"],
  parameters: {
    vizier: { id: "D37", replaces: null },
    docs: { description: { component: "Column labels. The table's `<colgroup>` sets the widths for the header and every row together." } },
  },
  args: { columns: [{ label: "Task" }, { label: "Latest run" }, { label: "Requested by" }, { label: "", srLabel: "Actions" }] },
  decorators: [
    (Story) => (
      <table className="w-760 border-collapse">
        <colgroup>
          <col className="w-280" />
          <col className="w-200" />
          <col className="w-150" />
          <col />
        </colgroup>
        {Story()}
      </table>
    ),
  ],
} satisfies Meta<typeof TableHeader>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
