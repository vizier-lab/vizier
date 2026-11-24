import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, within } from "storybook/test";
import { FilterTabs } from "../FilterTabs/FilterTabs";
import { StackedBarChart, type ChartSeries } from "./StackedBarChart";

const SERIES: ChartSeries[] = [
  { key: "chat", label: "Chat", colour: "ink" },
  { key: "background", label: "Background", colour: "chart-4" },
  { key: "scheduled", label: "Scheduled", colour: "chart-5" },
  { key: "dreams", label: "Dreams", colour: "chart-1" },
  { key: "compaction", label: "Compaction", colour: "chart-2" },
  { key: "vision", label: "Vision", colour: "chart-3" },
];

const LABELS: Record<number, string> = { 0: "Sep 21", 5: "Sep 26", 10: "Oct 1", 15: "Oct 6", 20: "Oct 11", 25: "Oct 16", 29: "today" };

let x = 11;
const rnd = () => ((x = (x * 9301 + 49297) % 233280), x / 233280);
const days = Array.from({ length: 30 }, (_, i) => ({
  label: LABELS[i],
  values: {
    chat: 0.25 + rnd() * 0.45,
    background: rnd() * 0.08,
    scheduled: rnd() * 0.06,
    dreams: 0.02 + rnd() * 0.03,
    compaction: i % 9 === 3 ? 0.06 : 0,
    vision: i >= 17 ? rnd() * 0.05 : 0,
  },
}));

const meta = {
  title: "Components/Visualisation/StackedBarChart",
  component: StackedBarChart,
  tags: ["approved"],
  parameters: { vizier: { id: "X70", replaces: "webui/app/components/UsageDailyChart.tsx" } },
  args: { title: "Daily cost", series: SERIES, days, annotation: { index: 17, label: "Oct 8 · vision model switched on" } },
  decorators: [(Story) => <div className="w-1010 pt-16">{Story()}</div>],
} satisfies Meta<typeof StackedBarChart>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Usage. */
export const Default: Story = {
  play: async ({ canvasElement }) => {
    // Regression: the stack must take the bar's height, or every segment's percentage
    // collapses to zero and the chart draws empty. offsetHeight ignores the grow-in transform.
    const list = within(canvasElement).getByRole("list", { name: "Daily cost" });
    const segment = list.querySelector("li > div > span") as HTMLElement | null;
    expect(segment).not.toBeNull();
    expect(segment!.offsetHeight).toBeGreaterThan(0);
  },
};

/** With a measure switch on the title row. */
export const WithAction: Story = {
  args: {
    action: <FilterTabs aria-label="Measure" value="cost" onChange={fn()} options={[{ value: "cost", label: "Cost" }, { value: "tokens", label: "Tokens" }]} />,
  },
};
