import type { Meta, StoryObj } from "@storybook/react-vite";
import { VideoPlayer } from "./VideoPlayer";

const meta = {
  title: "Components/Chat/VideoPlayer",
  component: VideoPlayer,
  tags: ["approved"],
  parameters: { vizier: { id: "X46", replaces: null } },
  args: { title: "demo-filtered-search.mp4", caption: "demo-filtered-search.mp4 · 0:18 · 2.4 MB", onPlay: () => {} },
  decorators: [(Story) => <div className="w-396">{Story()}</div>],
} satisfies Meta<typeof VideoPlayer>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
