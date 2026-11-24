import type { Meta, StoryObj } from "@storybook/react-vite";
import { Status } from "../Status/Status";
import { KeyValueList } from "./KeyValueList";

const meta = {
  title: "Components/Layout/KeyValueList",
  component: KeyValueList,
  tags: ["approved"],
  parameters: { vizier: { id: "X3", replaces: null } },
  args: {
    items: [
      { label: "Schedule", value: "Mondays 09:00 (cron)" },
      { label: "Requested by", value: "ganesha" },
      { label: "Next run", value: "Mon Oct 27, 09:00" },
    ],
  },
  decorators: [(Story) => <div className="w-330">{Story()}</div>],
} satisfies Meta<typeof KeyValueList>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Values that are a status, a link and an action. */
export const RichValues: Story = {
  args: {
    items: [
      { label: "Reader", value: <Status state="running" label="Reading" font="sans" /> },
      { label: "Bot", value: <a href="#" className="text-fg underline-offset-2 hover:underline">@archivist_bot</a> },
      { label: "Token", value: <button type="button" className="type-ui-medium text-fg hover:underline">Replace</button> },
    ],
  },
};
