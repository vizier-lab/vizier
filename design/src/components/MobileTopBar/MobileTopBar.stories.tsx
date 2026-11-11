import type { Meta, StoryObj } from "@storybook/react-vite";
import { IconButton } from "../IconButton/IconButton";
import { JobsPill } from "../JobsPill/JobsPill";
import { MobileTopBar } from "./MobileTopBar";

const bell = <IconButton variant="ghost" size="lg" icon="bell" aria-label="Notifications, 5 unread" count={5} />;

const meta = {
  title: "Components/Mobile/MobileTopBar",
  component: MobileTopBar,
  tags: ["approved"],
  parameters: { vizier: { id: "X75", replaces: null }, layout: "fullscreen" },
  args: { onMenu: () => {}, title: "Vizier", trailing: bell },
  decorators: [(Story) => <div className="w-402">{Story()}</div>],
} satisfies Meta<typeof MobileTopBar>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Home and the app's own pages: ☰, a title, the bell. */
export const Home: Story = {};

/** A chat: the agent's name switches agent, the sliders open its details, the topic row opens the topic sheet. */
export const Chat: Story = {
  args: {
    title: undefined,
    agent: {
      agentId: "archivist",
      name: "Archivist",
      state: "running",
      detail: "replying",
      onSwitch: () => {},
      onDetails: () => {},
    },
    topic: {
      title: "l15PmT7Vk_",
      meta: "webui · new topic",
      onTopic: () => {},
      actions: (
        <>
          <IconButton variant="ghost" icon="trash" aria-label="Delete conversation" />
          <IconButton variant="ghost" icon="plus" aria-label="New conversation" />
        </>
      ),
    },
    trailing: (
      <div className="flex items-center">
        <JobsPill count={1} compact />
        {bell}
      </div>
    ),
  },
};

/** A page inside an agent (Memory, Schedule…): back to its details, the agent's name under the title. */
export const AgentPage: Story = {
  args: {
    onMenu: undefined,
    onBack: () => {},
    backLabel: "Back to Archivist",
    title: "Memory",
    subtitle: "Archivist",
    trailing: <IconButton variant="ghost" size="lg" icon="plus" aria-label="New memory" />,
  },
};

/** The narrowest phone the shell supports. */
export const Narrow: Story = {
  ...Chat,
  decorators: [(Story) => <div className="w-360">{Story()}</div>],
};
