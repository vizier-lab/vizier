import type { Meta, StoryObj } from "@storybook/react-vite";
import { SecretReveal } from "./SecretReveal";

const meta = {
  title: "Components/Actions & inputs/SecretReveal",
  component: SecretReveal,
  tags: ["approved"],
  parameters: { vizier: { id: "X23", replaces: null } },
  args: {
    title: "Key “home-assistant” created. Copy it now: it won't be shown again.",
    secret: "vz_live_8f3K2pQ9xR7mT1vB6nW4yH0cJ5sL",
  },
  decorators: [(Story) => <div className="w-860">{Story()}</div>],
} satisfies Meta<typeof SecretReveal>;

export default meta;
type Story = StoryObj<typeof meta>;

/** API keys, right after creating one. */
export const Default: Story = {};
