import type { Meta, StoryObj } from "@storybook/react-vite";
import { Kbd } from "./Kbd";

const meta = {
  title: "Components/Layout/Kbd",
  component: Kbd,
  tags: ["approved"],
  parameters: { vizier: { id: "X6", replaces: "webui/app/components/MediaKeyHint.tsx" } },
  args: { children: "Enter", look: "inline" },
} satisfies Meta<typeof Kbd>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** In running text, as in the picker and file preview footers. */
export const Inline: Story = {
  render: () => (
    <p className="type-small text-muted">
      <Kbd>↑</Kbd> <Kbd>↓</Kbd> to move · <Kbd>Enter</Kbd> to open · <Kbd>Esc</Kbd> to close
    </p>
  ),
};

/** Inside a control, taking its colour (see Button's shortcut). */
export const Inside: Story = {
  render: () => (
    <span className="inline-flex items-center gap-10 rounded-md bg-ink px-14 py-8 type-ui-strong text-ground">
      Save <Kbd look="inside">Ctrl S</Kbd>
    </span>
  ),
};
