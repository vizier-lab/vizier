import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button } from "../Button/Button";
import { KeyValueList } from "../KeyValueList/KeyValueList";
import { SlideOver, SlideOverPanel } from "./SlideOver";

const body = (
  <KeyValueList
    items={[
      { label: "Started", value: "Mon Oct 20, 09:00" },
      { label: "Took", value: "14 s" },
      { label: "Tokens", value: "412" },
    ]}
  />
);

const meta = {
  title: "Components/Surfaces & data/SlideOver",
  component: SlideOverPanel,
  tags: ["approved"],
  parameters: { vizier: { id: "D35", replaces: "webui/app/components/SlideOver.tsx" } },
  args: { title: "Run · Morning brief", onClose: () => {}, children: body, footer: <Button>Close</Button> },
  decorators: [(Story) => <div className="h-560">{Story()}</div>],
} satisfies Meta<typeof SlideOverPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** The real panel: slides over the page, Esc closes. */
export const Modal: Story = {
  render: function Render(args) {
    const [open, setOpen] = useState(false);
    return (
      <>
        <Button onClick={() => setOpen(true)}>Open run</Button>
        <SlideOver {...args} open={open} onClose={() => setOpen(false)} footer={<Button onClick={() => setOpen(false)}>Close</Button>} />
      </>
    );
  },
};

/**
 * The panel as it actually appears. A native dialog is sized by the browser
 * (fit-content), so this story fails if the panel ever loses its size.
 */
export const Open: Story = {
  render: function Render(args) {
    return <SlideOver {...args} open onClose={() => {}} />;
  },
  play: async ({ canvasElement }) => {
    const dialog = canvasElement.querySelector("dialog");
    if (!dialog) throw new Error("no dialog");
    const box = dialog.getBoundingClientRect();
    if (box.width < 200 || box.height < 200) {
      throw new Error(`the panel has no size: ${box.width}x${box.height}`);
    }
    const heading = canvasElement.querySelector("h2")?.getBoundingClientRect();
    if (!heading || heading.width < 100) throw new Error("the panel is not showing");
  },
};
