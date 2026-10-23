import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button/Button";
import { EmptyState } from "./EmptyState";

const meta = {
  title: "Components/Feedback/EmptyState",
  component: EmptyState,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D29", replaces: null } },
  args: {
    kind: "empty",
    icon: "tasks",
    title: "No duties scheduled",
    message: 'Your steward waits on your word. Ask in chat ("every Monday at 9, review the backlog") or add one here.',
    action: <Button variant="primary" icon="plus">New task</Button>,
  },
  decorators: [(Story) => <div className="w-420">{Story()}</div>],
} satisfies Meta<typeof EmptyState>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Empty: Story = {};

export const NoResults: Story = {
  args: { kind: "no-results", icon: undefined, title: "Nothing matched", message: 'No passages for "pinecone" in any bundle. Try fewer words, or search all agents.', action: undefined },
};

export const Error: Story = {
  args: {
    kind: "error",
    icon: undefined,
    title: "Couldn't load tasks",
    message: "The server didn't answer (502). Nothing is lost; try again in a moment.",
    action: <Button icon="refresh">Try again</Button>,
  },
};
