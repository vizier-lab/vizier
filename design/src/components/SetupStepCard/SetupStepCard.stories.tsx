import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button/Button";
import { Callout } from "../Callout/Callout";
import { TextLink } from "../TextLink/TextLink";
import { SetupStepCard } from "./SetupStepCard";

const meta = {
  title: "Components/Agents & dreams/SetupStepCard",
  component: SetupStepCard,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X54", replaces: "webui/app/components/SetupProgress.tsx" } },
  args: { step: 2, state: "current", title: "Create your first agent", description: "Pick a model, give it a name, and choose what it may do. You can change everything later." },
  decorators: [(Story) => <ol className="flex w-760 flex-col gap-16">{Story()}</ol>],
} satisfies Meta<typeof SetupStepCard>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Home, first run. */
export const FirstRun: Story = {
  render: () => (
    <>
      <SetupStepCard step={1} state="done" title="Add a provider" description="Anthropic connected · key saved." action={<TextLink size="ui" href="#">Manage</TextLink>} />
      <SetupStepCard step={2} state="current" title="Create your first agent" description="Pick a model, give it a name, and choose what it may do. You can change everything later." action={<Button variant="primary" icon="plus">Create agent</Button>} />
      <SetupStepCard step={3} state="upcoming" title="Talk to it anywhere" description="Chat here, or connect Discord and Telegram from the agent's Channels page." />
      <li className="list-none">
        <Callout>No API key? Ollama runs open models on this machine, no key needed.</Callout>
      </li>
    </>
  ),
};
