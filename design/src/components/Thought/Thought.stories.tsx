import type { Meta, StoryObj } from "@storybook/react-vite";
import { Thought } from "./Thought";

const meta = {
  title: "Components/Chat/Thought",
  component: Thought,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D53", replaces: null } },
  args: { children: "The benchmark notes live in the infra bundle. Search there first rather than the whole memory, then read the three newest." },
  decorators: [(Story) => <div className="w-480">{Story()}</div>],
} satisfies Meta<typeof Thought>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
