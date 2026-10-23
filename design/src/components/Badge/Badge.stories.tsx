import type { Meta, StoryObj } from "@storybook/react-vite";
import { Badge } from "./Badge";

const meta = {
  title: "Components/Feedback/Badge",
  component: Badge,
  tags: ["approved"],
  parameters: { vizier: { id: "D23", replaces: null } },
  args: { children: "custom", tone: "neutral" },
  argTypes: { tone: { control: "inline-radio", options: ["neutral", "rank", "live", "failed", "retrying"] } },
} satisfies Meta<typeof Badge>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Tones: Story = {
  render: () => (
    <div className="flex flex-wrap items-center gap-8">
      <Badge>custom</Badge>
      <Badge tone="rank">Owner</Badge>
      <Badge tone="live">live</Badge>
      <Badge tone="failed">failed</Badge>
      <Badge tone="retrying">retrying</Badge>
    </div>
  ),
};

/** As drawn on screens: tags and keywords, provenance, grants, versions. */
export const InUse: Story = {
  render: () => (
    <div className="flex flex-wrap items-center gap-8">
      <Badge>vector-db</Badge>
      <Badge>review</Badge>
      <Badge>built-in</Badge>
      <Badge>custom</Badge>
      <Badge tone="rank">granted</Badge>
      <Badge tone="rank">superadmin</Badge>
      <Badge tone="live">Complete</Badge>
      <Badge>agent · v1.2.0</Badge>
    </div>
  ),
};
