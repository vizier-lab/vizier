import type { Meta, StoryObj } from "@storybook/react-vite";
import { Attachment } from "./Attachment";

const meta = {
  title: "Components/Chat/Attachment",
  component: Attachment,
  tags: ["approved"],
  parameters: { vizier: { id: "D57", replaces: "webui/app/components/AttachmentChip.tsx" } },
  args: { name: "bench-old.csv", detail: "12 KB", state: "ready", onRemove: () => {} },
  argTypes: { state: { control: "inline-radio", options: ["ready", "uploading", "failed"] } },
} satisfies Meta<typeof Attachment>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Ready: Story = {};

export const Uploading: Story = { args: { state: "uploading", detail: "48%" } };

export const Failed: Story = { args: { state: "failed", detail: "over 25 MB" } };

/** On a sent message: no remove. */
export const Sent: Story = { args: { onRemove: undefined } };
