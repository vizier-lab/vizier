// lint-tokens-exempt: the scheme story has to supply a palette that isn't the page's own.
import { useEffect } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Avatar } from "./Avatar";

const meta = {
  title: "Components/Surfaces & data/Avatar",
  component: Avatar,
  tags: ["approved"],
  parameters: { vizier: { id: "D40", replaces: "webui/app/components/avatar.tsx" } },
  args: { name: "ganesha", size: "md" },
  argTypes: { size: { control: "inline-radio", options: ["sm", "md", "lg"] } },
} satisfies Meta<typeof Avatar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Sm in lists, Md in menus, Lg on the profile header. */
export const Sizes: Story = {
  render: () => (
    <div className="flex items-end gap-16">
      <Avatar name="ganesha" size="sm" />
      <Avatar name="ganesha" size="md" />
      <Avatar name="ganesha" size="lg" />
    </div>
  ),
};

const SCHEME = { "--vz-live": "#50fa7b", "--vz-ink": "#bd93f9", "--vz-text-soft": "#f8f8f2", "--vz-chart-4": "#ff79c6", "--vz-chart-5": "#8be9fd", "--vz-border": "#44475a" };

function WithScheme() {
  useEffect(() => {
    const { style } = document.documentElement;
    for (const [name, value] of Object.entries(SCHEME)) style.setProperty(name, value);
    return () => {
      for (const name of Object.keys(SCHEME)) style.removeProperty(name);
    };
  }, []);
  return <Avatar name="ganesha" size="lg" />;
}

/** Today's fixed colours, as long as no scheme has painted the page. */
export const FixedColors: Story = { args: { size: "lg" } };

/** A scheme painted on the page recolours the face and keeps its shape. */
export const SchemeColors: Story = { render: () => <WithScheme /> };
