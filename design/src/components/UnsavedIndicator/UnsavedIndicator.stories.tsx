import type { Meta, StoryObj } from "@storybook/react-vite";
import { UnsavedIndicator } from "./UnsavedIndicator";

const meta = {
  title: "Components/Layout/UnsavedIndicator",
  component: UnsavedIndicator,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X8", replaces: null } },
} satisfies Meta<typeof UnsavedIndicator>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Core. */
export const Default: Story = {};

/** Skill editor, where the draft survives a reload. */
export const DraftKept: Story = { args: { children: "Unsaved · draft kept in this browser" } };
