import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button/Button";
import { Banner } from "./Banner";

const meta = {
  title: "Components/Feedback/Banner",
  component: Banner,
  tags: ["approved"],
  parameters: { vizier: { id: "D26", replaces: null } },
  args: { tone: "offline", title: "Connection lost · reconnecting in 3 s", actions: <Button variant="ghost" size="sm">Retry now</Button> },
  argTypes: { tone: { control: "inline-radio", options: ["offline", "reconnected", "failed"] } },
  decorators: [(Story) => <div className="w-860">{Story()}</div>],
} satisfies Meta<typeof Banner>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Also used inline in chat. */
export const Offline: Story = {};

export const Reconnected: Story = { args: { tone: "reconnected", title: "Back online · 2 queued messages sent", actions: undefined } };

/** The agent-failed banner, with its actions. */
export const Failed: Story = {
  args: {
    tone: "failed",
    title: 'Scribe couldn\'t start: model "llama3.3:70b" not found on Ollama.',
    message: "Failed 3 times · retrying in 40 s. Messages you send now wait in line until it starts.",
    actions: (
      <>
        <Button icon="refresh">Retry now</Button>
        <Button icon="agent">Open config</Button>
      </>
    ),
  },
};
