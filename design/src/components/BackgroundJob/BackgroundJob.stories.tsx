import type { Meta, StoryObj } from "@storybook/react-vite";
import { BackgroundJob } from "./BackgroundJob";

const meta = {
  title: "Components/Chat/BackgroundJob",
  component: BackgroundJob,
  tags: ["approved"],
  parameters: { vizier: { id: "D59", replaces: "webui/app/components/BackgroundJobTray.tsx" } },
  args: {
    title: "paralel_subtasks",
    goal: "Re-run the p99 benchmark on all three at 4M passages",
    state: "running",
    elapsed: "1:42",
    pieces: { done: 2, total: 3 },
    onJumpToStart: () => {},
    onCancel: () => {},
  },
  argTypes: { state: { control: "select", options: ["running", "reporting", "reported", "undelivered", "cancelled", "interrupted"] } },
  decorators: [(Story) => <div className="w-280">{Story()}</div>],
} satisfies Meta<typeof BackgroundJob>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Running: Story = {};

/** Every state, as in the tray. */
export const States: Story = {
  render: () => (
    <div className="flex flex-col gap-8">
      <BackgroundJob title="delegate_agent → Scout" goal="LanceDB release notes check" state="running" elapsed="1:42" pieces={{ done: 0, total: 1 }} onCancel={() => {}} />
      <BackgroundJob title="delegate_agent → Scout" goal="LanceDB release notes check" state="reporting" elapsed="3:08" pieces={{ done: 1, total: 1 }} />
      <BackgroundJob title="delegate_agent → Scout" goal="LanceDB release notes check" state="reported" elapsed="3:10" />
      <BackgroundJob title="delegate_agent → Scout" goal="LanceDB release notes check" state="undelivered" elapsed="3:10" />
      <BackgroundJob title="paralel_subtasks" goal="Re-run the p99 benchmark on all three" state="cancelled" />
      <BackgroundJob title="delegate_agent → Scout" goal="LanceDB release notes check" state="interrupted" />
    </div>
  ),
};
