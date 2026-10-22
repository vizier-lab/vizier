import type { Meta, StoryObj } from "@storybook/react-vite";
import { ProgressBar } from "./ProgressBar";

const meta = {
  title: "Components/Feedback/ProgressBar",
  component: ProgressBar,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D30", replaces: null } },
  args: { label: "Pulling qwen3:32b from Ollama", value: 62, detail: "12.4 GB of 20.0 GB · about 3 min", state: "running" },
  argTypes: { state: { control: "inline-radio", options: ["running", "done", "failed"] } },
  decorators: [(Story) => <div className="w-360">{Story()}</div>],
} satisfies Meta<typeof ProgressBar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Running: Story = {};

export const Indeterminate: Story = { args: { value: undefined, detail: "4.1 GB so far · size unknown" } };

export const Done: Story = { args: { state: "done", detail: "20.0 GB · 6 min" } };

export const Failed: Story = { args: { state: "failed", value: 41, detail: "Stopped at 8.1 GB: connection reset. Try again to resume." } };
