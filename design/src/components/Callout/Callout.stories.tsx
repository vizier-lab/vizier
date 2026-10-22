import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button/Button";
import { Callout } from "./Callout";

const meta = {
  title: "Components/Layout/Callout",
  component: Callout,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X5", replaces: null } },
  args: { children: "No API key? Ollama runs open models on this machine, no key needed." },
  decorators: [(Story) => <div className="w-700">{Story()}</div>],
} satisfies Meta<typeof Callout>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Home, first run. */
export const Default: Story = {};

/** Addons: the end of the list. */
export const WithTitle: Story = {
  args: {
    title: "That's every addon in this version.",
    children: "New addons ship with Vizier releases. For anything else, connect an MCP server.",
  },
};

export const WithAction: Story = {
  args: { action: <Button size="sm">Set up Ollama</Button> },
};
