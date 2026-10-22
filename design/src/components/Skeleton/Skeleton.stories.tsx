import type { Meta, StoryObj } from "@storybook/react-vite";
import { Skeleton } from "./Skeleton";

const meta = {
  title: "Components/Feedback/Skeleton",
  component: Skeleton,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D31", replaces: "webui/app/components/Skeleton.tsx" } },
  args: { variant: "card", lines: 3 },
  argTypes: { variant: { control: "inline-radio", options: ["card", "lines"] } },
  decorators: [(Story) => <div className="w-420">{Story()}</div>],
} satisfies Meta<typeof Skeleton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Card: Story = {};

/** Text lines, while chat history loads. */
export const Lines: Story = { args: { variant: "lines", lines: 5 } };
