import type { Meta, StoryObj } from "@storybook/react-vite";
import { Step } from "./Step";

const meta = {
  title: "Components/Shell/Step",
  component: Step,
  tags: ["approved"],
  parameters: { vizier: { id: "D9", replaces: null } },
  args: { number: 3, label: "Tools", state: "current" },
  argTypes: { state: { control: "inline-radio", options: ["done", "current", "upcoming", "error"] } },
  decorators: [(Story) => <ol className="flex">{Story()}</ol>],
} satisfies Meta<typeof Step>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const States: Story = {
  render: () => (
    <>
      <Step number={1} label="Identity" state="done" />
      <Step number={2} label="Model" state="error" className="ml-24" />
      <Step number={3} label="Tools" state="current" className="ml-24" />
      <Step number={4} label="Review" state="upcoming" className="ml-24" />
    </>
  ),
};
