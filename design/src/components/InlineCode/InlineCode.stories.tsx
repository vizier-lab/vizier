import type { Meta, StoryObj } from "@storybook/react-vite";
import { InlineCode } from "./InlineCode";

const meta = {
  title: "Components/Surfaces & data/InlineCode",
  component: InlineCode,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D42", replaces: null } },
  args: { children: "memory_search" },
} satisfies Meta<typeof InlineCode>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** In a sentence of reply prose. */
export const InProse: Story = {
  render: () => (
    <p className="max-w-560 type-prose text-fg">
      I searched with <InlineCode>memory_search</InlineCode> and found the note in <InlineCode>projects/vector-db-review.md</InlineCode>; the
      index lives under <InlineCode>agents/archivist/memory</InlineCode>.
    </p>
  ),
};
