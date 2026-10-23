import type { Meta, StoryObj } from "@storybook/react-vite";
import { ConversationRow } from "./ConversationRow";

const meta = {
  title: "Components/Actions & inputs/ConversationRow",
  component: ConversationRow,
  tags: ["approved"],
  parameters: { vizier: { id: "X80" } },
  args: { kind: "webui", title: "vector-db-review", place: "WebUI · topic 3 of 5", time: "14:02" },
  decorators: [(Story) => <div className="w-360 rounded-xl border border-border bg-surface px-14">{Story()}</div>],
} satisfies Meta<typeof ConversationRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const WebUI: Story = {};

export const Live: Story = { args: { live: true } };

export const Discord: Story = { args: { kind: "discord", title: "#releases", place: "Discord · Vizier Lab", time: "13:40" } };

export const Telegram: Story = { args: { kind: "telegram", title: "DM · ana", place: "Telegram", time: "11:05" } };

/** The matching part of the title is bold. */
export const Matched: Story = { args: { title: "vector-db-review", query: "db" } };

export const Selected: Story = { args: { selected: true, onSelect: () => {} } };

/** A long title is cut, never wrapped. */
export const LongTitle: Story = { args: { title: "A very long conversation title that keeps going past the edge of the picker", time: "Yesterday" } };
