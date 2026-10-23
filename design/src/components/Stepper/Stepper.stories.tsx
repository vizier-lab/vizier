import type { Meta, StoryObj } from "@storybook/react-vite";
import { Stepper } from "./Stepper";

const meta = {
  title: "Components/Layout/Stepper",
  component: Stepper,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X10", replaces: "webui/app/routes/agent-new.tsx" } },
  args: {
    "aria-label": "New agent",
    steps: [{ label: "Config", state: "done" }, { label: "System Prompt", state: "done" }, { label: "Tools", state: "current" }, { label: "Review" }],
  },
} satisfies Meta<typeof Stepper>;

export default meta;
type Story = StoryObj<typeof meta>;

/** New agent's header. */
export const Header: Story = {};

/** Onboarding. */
export const Compact: Story = {
  args: { look: "compact", "aria-label": "Setup", steps: [{ label: "Account", state: "done" }, { label: "Provider", state: "current" }, { label: "First agent" }] },
};

/** A step with a field to fix. */
export const WithError: Story = {
  args: { steps: [{ label: "Config", state: "done" }, { label: "System Prompt", state: "error" }, { label: "Tools", state: "current" }, { label: "Review" }] },
};
