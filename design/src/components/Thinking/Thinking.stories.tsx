import type { Meta, StoryObj } from "@storybook/react-vite";
import { Thinking } from "./Thinking";

const meta = {
  title: "Components/Chat/Thinking",
  component: Thinking,
  tags: ["approved"],
  parameters: { vizier: { id: "D47", replaces: "webui/app/components/ThinkingIndicator.tsx" } },
  args: { children: "Archivist is thinking…" },
} satisfies Meta<typeof Thinking>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Woken by a background report. */
export const Answering: Story = { args: { children: "Archivist is answering Scout's report…" } };

export const RunningTool: Story = { args: { children: "Archivist is running execute_python…" } };
