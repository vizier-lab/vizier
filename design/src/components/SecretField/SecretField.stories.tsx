import type { Meta, StoryObj } from "@storybook/react-vite";
import { KeyValueList } from "../KeyValueList/KeyValueList";
import { SecretField } from "./SecretField";

const meta = {
  title: "Components/Actions & inputs/SecretField",
  component: SecretField,
  tags: ["approved"],
  parameters: { vizier: { id: "X14", replaces: null } },
  args: { label: "API key", masked: "••••••••••••7c1e", onReplace: () => {} },
  decorators: [(Story) => <div className="w-420">{Story()}</div>],
} satisfies Meta<typeof SecretField>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Addons: an inline label. */
export const Default: Story = {};

/** Channels: a value in a KeyValueList. */
export const InList: Story = {
  render: () => (
    <KeyValueList
      items={[
        { label: "Bot", value: "@archivist_bot" },
        { label: "Token", value: <SecretField masked="••••Q2aa" onReplace={() => {}} /> },
      ]}
    />
  ),
};
