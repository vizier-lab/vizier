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
