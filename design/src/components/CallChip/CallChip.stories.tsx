import type { Meta, StoryObj } from "@storybook/react-vite";
import { CallChip, callStatuses, type CallStatus } from "./CallChip";

const DETAIL: Record<CallStatus, string> = {
  pending: "pick_files · waiting on this tab",
  ok: "pick_files · 2 files chosen",
  error: "pick_files · the browser denied access",
  deferred: "pick_files · runs when a tab reconnects",
  timeout: "pick_files · no answer in 60 s",
  disconnected: "pick_files · the tab closed before answering",
  cancelled: "pick_files · the turn was stopped",
  interrupted: "pick_files · the server restarted",
};

const meta = {
  title: "Components/Chat/CallChip",
  component: CallChip,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D58", replaces: "webui/app/components/client/ClientCallChip.tsx" } },
  args: { status: "pending", detail: DETAIL.pending },
  argTypes: { status: { control: "select", options: callStatuses } },
  decorators: [(Story) => <div className="w-380">{Story()}</div>],
} satisfies Meta<typeof CallChip>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const All: Story = {
  render: () => (
    <div className="flex flex-col gap-8">
      {callStatuses.map((s) => (
        <CallChip key={s} status={s} detail={DETAIL[s]} />
      ))}
    </div>
  ),
};
