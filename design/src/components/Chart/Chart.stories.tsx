import type { Meta, StoryObj } from "@storybook/react-vite";
import { Chart } from "./Chart";

const meta = {
  title: "Components/Visualisation/Chart",
  component: Chart,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X87", replaces: null } },
  args: {
    kind: "bar",
    title: "Replies per day",
    height: 200,
    data: [
      { label: "Mon", value: 12 },
      { label: "Tue", value: 19 },
      { label: "Wed", value: 7 },
      { label: "Thu", value: 24 },
      { label: "Fri", value: 15 },
    ],
  },
  argTypes: { kind: { control: "inline-radio", options: ["bar", "line", "area", "pie"] } },
  decorators: [(Story) => <div className="w-480">{Story()}</div>],
} satisfies Meta<typeof Chart>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Bar: Story = {};

export const Line: Story = { args: { kind: "line" } };

export const Area: Story = { args: { kind: "area" } };

export const Pie: Story = { args: { kind: "pie" } };

const two = ["Mon", "Tue", "Wed", "Thu"].flatMap((label, i) => [
  { label, value: 4 + i * 3, series: "Replies" },
  { label, value: 10 - i * 2, series: "Errors" },
]);

export const TwoSeries: Story = { args: { kind: "bar", data: two } };

export const TwoSeriesLine: Story = { args: { kind: "line", data: two } };

export const Pressable: Story = { args: { onPress: () => {} } };
