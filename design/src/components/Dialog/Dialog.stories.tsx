import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button } from "../Button/Button";
import { Dialog, DialogPanel } from "./Dialog";

const meta = {
  title: "Components/Feedback/Dialog",
  component: DialogPanel,
  tags: ["approved"],
  parameters: { vizier: { id: "D28", replaces: "webui/app/components/providers/DeleteWarning.tsx" } },
  args: {
    title: "Restart Archivist?",
    message: "Its running turn stops and queued messages run after it starts again.",
    confirmLabel: "Restart",
    confirmIcon: "refresh",
    onConfirm: () => {},
    onCancel: () => {},
  },
} satisfies Meta<typeof DialogPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Danger, with type-to-confirm: the delete button enables once "scribe" is typed. */
export const Danger: Story = {
  args: {
    tone: "danger",
    title: "Delete Scribe?",
    message: "Its memory (2 bundles, 31 documents), 3 tasks, 12 sessions and its Telegram connection are deleted. This can't be undone.",
    confirmWord: "scribe",
    confirmLabel: "Delete Scribe",
    confirmIcon: "trash",
  },
};

/** The real modal: opens over a scrim, traps focus, Esc cancels. */
export const Modal: Story = {
  render: function Render(args) {
    const [open, setOpen] = useState(false);
    return (
      <>
        <Button onClick={() => setOpen(true)}>Restart Archivist</Button>
        <Dialog {...args} open={open} onCancel={() => setOpen(false)} onConfirm={() => setOpen(false)} />
      </>
    );
  },
};

/**
 * The dialog as it actually appears. A native dialog is sized by the browser
 * (fit-content), so this story fails if the dialog and its panel ever lose
 * their size again.
 */
export const Open: Story = {
  render: function Render(args) {
    return <Dialog {...args} open onCancel={() => {}} onConfirm={() => {}} />;
  },
  play: async ({ canvasElement }) => {
    const dialog = canvasElement.querySelector("dialog");
    if (!dialog) throw new Error("no dialog");
    const box = dialog.getBoundingClientRect();
    if (box.width < 200 || box.height < 200) {
      throw new Error(`the dialog has no size: ${box.width}x${box.height}`);
    }
    const panel = dialog.querySelector("[role='dialog'], h2")?.getBoundingClientRect();
    if (!panel || panel.width < 100 || panel.height < 40) {
      throw new Error("the dialog panel is not showing");
    }
  },
};
