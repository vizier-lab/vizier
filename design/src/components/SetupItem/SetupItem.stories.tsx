import type { Meta, StoryObj } from "@storybook/react-vite";
import { SetupItem } from "./SetupItem";

const meta = {
  title: "Components/Feedback/SetupItem",
  component: SetupItem,
  tags: ["approved"],
  parameters: {
    docs: {
      description: {
        component: "An agent's live setup, in the same card shape as a notification. No read state, never counted as unread.",
      },
    },
  },
  args: {
    agentId: "scout",
    agentName: "Scout",
    step: "Downloading a model",
    percent: 34,
    detail: "nomic-embed-text · 412 MB of 1.2 GB",
    eta: "about 2 min",
  },
  argTypes: { state: { control: "inline-radio", options: ["running", "failed"] } },
  decorators: [(Story) => <div className="w-376">{Story()}</div>],
} satisfies Meta<typeof SetupItem>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Running: Story = {};

/** The size isn't known, so the bar moves without a position and there is no percent. */
export const UnknownSize: Story = { args: { percent: undefined, detail: "4.1 GB so far", eta: undefined } };

export const Starting: Story = { args: { step: "Building tools", percent: undefined, detail: undefined, eta: undefined } };

export const Failed: Story = { args: { state: "failed", percent: 41, step: "Setup failed", detail: "Stopped at 8.1 GB: connection reset", eta: undefined } };
