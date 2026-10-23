import type { Meta, StoryObj } from "@storybook/react-vite";
import { Badge } from "../Badge/Badge";
import { Breadcrumb } from "./Breadcrumb";

const meta = {
  title: "Components/Layout/Breadcrumb",
  component: Breadcrumb,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X9", replaces: null } },
  args: { segments: [{ label: "infra", href: "#" }, { label: "vector-dbs", href: "#" }, { label: "summary.md" }] },
} satisfies Meta<typeof Breadcrumb>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Memory: a document's path. */
export const Path: Story = {};

/** Skill editor: a back link and a badge. */
export const BackLink: Story = { args: { segments: undefined, back: { label: "Skills", href: "#" }, trailing: <Badge>custom</Badge> } };
