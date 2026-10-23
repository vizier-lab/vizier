import type { Meta, StoryObj } from "@storybook/react-vite";
import { IconButton } from "../IconButton/IconButton";
import { JobsPill } from "../JobsPill/JobsPill";
import { MobileTopBar } from "./MobileTopBar";

const meta = {
  title: "Components/Mobile/MobileTopBar",
  component: MobileTopBar,
  tags: ["approved"],
  parameters: { vizier: { id: "X75", replaces: null }, layout: "fullscreen" },
  args: { onMenu: () => {}, trailing: <IconButton icon="plus" aria-label="New agent" /> },
  decorators: [(Story) => <div className="w-375">{Story()}</div>],
} satisfies Meta<typeof MobileTopBar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Home: Story = {};

export const Chat: Story = {
  args: {
    chat: { agentId: "archivist", agentName: "Archivist", topic: "vector-db-review", state: "live", detail: "replying", onTopic: () => {} },
    trailing: <JobsPill count={1} compact />,
  },
};
