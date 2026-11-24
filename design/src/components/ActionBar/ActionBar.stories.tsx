import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button/Button";
import { UnsavedIndicator } from "../UnsavedIndicator/UnsavedIndicator";
import { ActionBar } from "./ActionBar";

const meta = {
  title: "Components/Layout/ActionBar",
  component: ActionBar,
  tags: ["approved"],
  parameters: { vizier: { id: "X4", replaces: null }, layout: "fullscreen" },
  args: {
    position: "bottom",
    start: <Button icon="chevron-left">Back</Button>,
    end: <Button variant="primary" trailingIcon="chevron-right">Continue to review</Button>,
  },
} satisfies Meta<typeof ActionBar>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The New agent wizard's footer. */
export const Bottom: Story = {};

/** The skill editor's top bar. */
export const Top: Story = {
  args: {
    position: "top",
    start: <span className="type-code text-muted">skills / <span className="text-fg">code-review</span></span>,
    end: (
      <>
        <UnsavedIndicator>Unsaved · draft kept in this browser</UnsavedIndicator>
        <Button>Discard</Button>
        <Button variant="primary" shortcut="Ctrl S">Save</Button>
      </>
    ),
  },
};
