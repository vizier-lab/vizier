import type { Meta, StoryObj } from "@storybook/react-vite";
import { SigilPicker } from "./SigilPicker";

const meta = {
  title: "Components/Actions & inputs/SigilPicker",
  component: SigilPicker,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X22", replaces: "webui/app/components/AvatarCropModal.tsx" } },
  args: { agentId: "archivist", onUpload: () => {}, onReset: () => {} },
} satisfies Meta<typeof SigilPicker>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Generated: Story = {};
