import type { Meta, StoryObj } from "@storybook/react-vite";
import { ImageGallery } from "./ImageGallery";

// Placeholder flame graphs, bundled so the catalogue works offline.
import AFTER from "../../stories/flame-after.svg?url";
import BEFORE from "../../stories/flame-before.svg?url";

const meta = {
  title: "Components/Chat/ImageGallery",
  component: ImageGallery,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X40", replaces: null } },
  args: {
    images: [
      { src: BEFORE, name: "flame-before.png", size: "88 KB", alt: "Flame graph of the slow run" },
      { src: AFTER, name: "flame-after.png", size: "61 KB", alt: "Flame graph of the fixed run" },
    ],
  },
  decorators: [(Story) => <div className="w-400">{Story()}</div>],
} satisfies Meta<typeof ImageGallery>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Several: Story = {};

export const One: Story = { args: { images: [{ src: BEFORE, name: "flame-before.png", size: "88 KB", alt: "Flame graph of the slow run" }] } };
