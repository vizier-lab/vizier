import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Pagination } from "./Pagination";

const meta = {
  title: "Components/Surfaces & data/Pagination",
  component: Pagination,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D39", replaces: null } },
  args: { page: 1, pageCount: 13, total: 312, perPage: 25, onPageChange: () => {} },
  decorators: [(Story) => <div className="w-640">{Story()}</div>],
} satisfies Meta<typeof Pagination>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: function Render(args) {
    const [page, setPage] = useState(args.page);
    return <Pagination {...args} page={page} onPageChange={setPage} />;
  },
};

export const Positions: Story = {
  render: (args) => (
    <div className="flex flex-col divide-y divide-border">
      <Pagination {...args} page={1} />
      <Pagination {...args} page={2} />
      <Pagination {...args} page={7} />
      <Pagination {...args} page={13} />
      <Pagination {...args} page={1} pageCount={1} total={12} />
    </div>
  ),
};

/** Summary and arrows only. */
export const Compact: Story = { args: { compact: true, total: 1024, pageCount: 41, page: 2 } };
