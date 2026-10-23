import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useState } from "react";
import { ImageCropper } from "./ImageCropper";

/** A wide gradient picture, made in the browser, so the story needs no asset. */
function useSamplePicture(width: number, height: number) {
  const [file, setFile] = useState<File>();
  useEffect(() => {
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const tokens = getComputedStyle(document.documentElement);
    const g = ctx.createLinearGradient(0, 0, width, height);
    g.addColorStop(0, tokens.getPropertyValue("--vz-rank") || "gray");
    g.addColorStop(1, tokens.getPropertyValue("--vz-live") || "silver");
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, width, height);
    ctx.fillStyle = tokens.getPropertyValue("--vz-text") || "white";
    ctx.beginPath();
    ctx.arc(width / 2, height / 2, Math.min(width, height) / 4, 0, Math.PI * 2);
    ctx.fill();
    canvas.toBlob((b) => b && setFile(new File([b], "sample.png", { type: "image/png" })), "image/png");
  }, [width, height]);
  return file;
}

const meta = {
  title: "Components/Actions & inputs/ImageCropper",
  component: ImageCropper,
  tags: ["approved"],
  parameters: { vizier: { id: "X79" } },
  args: { file: new File([], "none.png"), onCrop: () => {}, onCancel: () => {} },
} satisfies Meta<typeof ImageCropper>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Drag to move, the slider or the wheel to zoom, arrow keys (Shift for bigger steps) to nudge, + and − to zoom. */
export const Landscape: Story = {
  render: function Render(args) {
    const file = useSamplePicture(1200, 800);
    return file ? <ImageCropper {...args} file={file} /> : <div />;
  },
};

export const Portrait: Story = {
  render: function Render(args) {
    const file = useSamplePicture(600, 900);
    return file ? <ImageCropper {...args} file={file} /> : <div />;
  },
};
