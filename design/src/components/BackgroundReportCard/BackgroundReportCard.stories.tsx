import type { Meta, StoryObj } from "@storybook/react-vite";
import { BackgroundReportCard } from "./BackgroundReportCard";

const meta = {
  title: "Components/Chat/BackgroundReportCard",
  component: BackgroundReportCard,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X44", replaces: "webui/app/components/BackgroundReportItem.tsx" } },
  args: {
    agentId: "scout",
    meta: "delegate_agent → Scout · 2m 14s",
    linkLabel: "Open Scout's conversation",
    href: "#",
    children: "Scout checked the LanceDB release notes: v0.19 adds the filtered-search fix we were waiting on. Worth a fourth row once it ships.",
  },
  decorators: [(Story) => <div className="w-720">{Story()}</div>],
} satisfies Meta<typeof BackgroundReportCard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
