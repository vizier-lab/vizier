import type { Meta, StoryObj } from "@storybook/react-vite";
import { TrailHead } from "./TrailHead";

const meta = {
  title: "Components/Chat/TrailHead",
  component: TrailHead,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D50", replaces: null } },
  args: { state: "live", title: "Working 6.4s", counts: "2 python runs" },
  argTypes: { state: { control: "inline-radio", options: ["live", "done", "failed"] } },
  decorators: [(Story) => <div className="w-480">{Story()}</div>],
} satisfies Meta<typeof TrailHead>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Live: Story = {};

export const Done: Story = { args: { state: "done", title: "Worked 18.4s", counts: "3 python runs · 4 passages" } };

export const Failed: Story = { args: { state: "failed", title: "Failed after 12.0s", counts: "1 python run · provider unavailable" } };

/** Stopped by a limit: the count says so. */
export const Stopped: Story = { args: { state: "done", title: "Worked 30.2s", counts: "1 python run · stopped" } };

/** Collapsed: one line with ›. */
export const Collapsed: Story = { args: { state: "done", title: "Worked 18.4s", counts: "3 python runs · 4 passages", expanded: false } };
