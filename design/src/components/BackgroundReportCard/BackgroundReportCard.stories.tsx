import type { Meta, StoryObj } from "@storybook/react-vite";
import { BackgroundReportCard } from "./BackgroundReportCard";
import { TrailCall } from "../TrailCall/TrailCall";
import { Notice } from "../Notice/Notice";

const meta = {
  title: "Components/Chat/BackgroundReportCard",
  component: BackgroundReportCard,
  tags: ["approved"],
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

/** Detached scripts that finished while the agent was idle (spec 058): one trail row per
 * script, opening like an `execute_python` row, in place of a link. */
export const DetachedDelivery: Story = {
  args: {
    title: "Background work finished",
    meta: "2 scripts",
    linkLabel: undefined,
    href: undefined,
    children: (
      <ul className="flex flex-col gap-8">
        <TrailCall name="background" intent="check vendor B's pricing page" duration="d_wbt6a42i · succeeded in 41s">
          <Notice compact>The script's code, calls and result open here.</Notice>
        </TrailCall>
        <TrailCall name="background" intent="fetch the changelog" state="failed" duration="d_j1rzxu02 · failed in 12s">
          <Notice compact tone="failed">HTTP 403</Notice>
        </TrailCall>
      </ul>
    ),
  },
};
