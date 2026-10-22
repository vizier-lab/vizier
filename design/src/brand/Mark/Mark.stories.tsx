import type { Meta, StoryObj } from "@storybook/react-vite";
import { Mark } from "./Mark";

const meta = {
  title: "Brand/Mark",
  component: Mark,
  tags: ["needs-review"],
  parameters: {
    vizier: { id: "B1", replaces: null },
    docs: {
      description: {
        component:
          "A bold V on an 8×8 dot grid: 28 equal dots, strokes two dots thick. 8×8 is the chessboard, where the vizier began as a piece. Only the top-right dot changes: emerald when live. Static surfaces (docs, social cards) show idle.",
      },
    },
  },
  args: { state: "idle", grid: false, size: 64 },
  argTypes: {
    state: { control: "inline-radio", options: ["idle", "live"] },
    size: { control: { type: "range", min: 16, max: 240, step: 4 } },
  },
} satisfies Meta<typeof Mark>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Live: Story = { args: { state: "live" } };

export const WithGrid: Story = { args: { grid: true, state: "live", size: 160 } };

/** State × grid, at the sizes the screens use. */
export const Variants: Story = {
  render: () => (
    <div className="flex flex-wrap items-end gap-32">
      {([16, 24, 32, 64, 160] as const).map((size) => (
        <div key={size} className="flex items-end gap-12">
          <Mark size={size} />
          <Mark size={size} state="live" />
          {size >= 64 && <Mark size={size} state="live" grid />}
        </div>
      ))}
    </div>
  ),
};
