import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ImageCropper } from "../ImageCropper/ImageCropper";
import { SigilPicker } from "./SigilPicker";

const meta = {
  title: "Components/Actions & inputs/SigilPicker",
  component: SigilPicker,
  tags: ["approved"],
  parameters: { vizier: { id: "X22", replaces: "webui/app/components/AvatarCropModal.tsx" } },
  args: { agentId: "archivist", onUpload: () => {}, onReset: () => {} },
} satisfies Meta<typeof SigilPicker>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Generated: Story = {};

/** Choosing a picture opens the cropper; the saved crop replaces the sigil, and "Sigil" goes back. */
export const Upload: Story = {
  render: function Render(args) {
    const [file, setFile] = useState<File>();
    const [saved, setSaved] = useState<string>();
    return (
      <>
        <SigilPicker {...args} imageSrc={saved} onUpload={setFile} onReset={() => setSaved(undefined)} />
        {file && (
          <ImageCropper
            file={file}
            onCancel={() => setFile(undefined)}
            onCrop={(blob) => {
              setSaved(URL.createObjectURL(blob));
              setFile(undefined);
            }}
          />
        )}
      </>
    );
  },
};
