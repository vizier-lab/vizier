import type { Meta, StoryObj } from "@storybook/react-vite";
import { Toast } from "./Toast";

const meta = {
  title: "Components/Feedback/Toast",
  component: Toast,
  tags: ["approved"],
  parameters: { vizier: { id: "D27", replaces: "webui/app/components/Toast.tsx" } },
  args: { tone: "success", title: "Memory updated", message: "infra/vector-dbs/summary.md · rev 3", onClose: () => {} },
  argTypes: { tone: { control: "inline-radio", options: ["success", "error", "warning", "info"] } },
} satisfies Meta<typeof Toast>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Tones: Story = {
  render: () => (
    <div className="flex flex-col gap-10">
      <Toast tone="success" title="Memory updated" message="infra/vector-dbs/summary.md · rev 3" onClose={() => {}} />
      <Toast tone="error" title="Couldn't save task" message='Cron "0 25 * * *": hour must be 0-23.' onClose={() => {}} />
      <Toast tone="warning" title="Near the monthly cap" message="Archivist has used 82% of its $40 limit." onClose={() => {}} />
      <Toast tone="info" title="Scribe is restarting" message="Messages you send now run once it is up." onClose={() => {}} />
    </div>
  ),
};
