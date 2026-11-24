import type { Meta, StoryObj } from "@storybook/react-vite";
import { DayTimeline } from "./DayTimeline";
import { MonthGrid, type MonthGridDay } from "./MonthGrid";
import { ScheduleChip, type ScheduleChipState } from "./ScheduleChip";

const meta = {
  title: "Components/Data/Schedule",
  component: ScheduleChip,
  tags: ["approved"],
  parameters: { vizier: { id: "D61", replaces: null } },
} satisfies Meta<typeof ScheduleChip>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Every state a chip can wear: how a run ended, what is still ahead, what passed unrecorded, a paused task, a repeat. */
export const Chips: Story = {
  args: { state: "answered", title: "weekly-memory-review", time: "09:00" },
  render: function Render() {
    const rows: [ScheduleChipState, string, string, string?][] = [
      ["answered", "09:00", "weekly-memory-review"],
      ["running", "12:00", "paper-digest"],
      ["no-response", "18:00", "journal-sweep"],
      ["interrupted", "12:00", "paper-digest"],
      ["upcoming", "18:00", "journal-sweep"],
      ["past", "12:00", "paper-digest"],
      ["paused", "02:00", "quarterly-archive"],
    ];
    return (
      <div className="flex w-220 flex-col gap-6">
        {rows.map(([state, time, title]) => (
          <ScheduleChip key={state} state={state} time={time} title={title} />
        ))}
        <ScheduleChip state="answered" title="release-watch" repeat="4×" />
      </div>
    );
  },
};

const chip = (state: ScheduleChipState, time: string, title: string, repeat?: string) => (
  <ScheduleChip key={`${time}${title}`} state={state} time={time} title={title} repeat={repeat} />
);

function week(from: number, today?: number, outside?: (n: number) => boolean): MonthGridDay[] {
  return Array.from({ length: 7 }, (_, i) => {
    const day = from + i;
    return {
      key: `2026-10-${String(day).padStart(2, "0")}`,
      day,
      name: `${day} October`,
      today: day === today,
      outside: outside?.(day),
      chips: [chip("answered", "", "release-watch", "4×"), chip(day < 30 ? "answered" : "upcoming", "12:00", "paper-digest")],
      more: day === 22 ? 2 : undefined,
    };
  });
}

/** Weeks of day cells; a day with more than fit shows a count. */
export const Month: Story = {
  args: Chips.args,
  render: function Render() {
    return (
      <div className="w-[1058px]">
        <MonthGrid label="October 2026" weekdays={["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]} weeks={[week(19), week(26, 30)]} />
      </div>
    );
  },
};

/** Blocks at their moment; two share the row, a fourth folds into a count. */
export const Day: Story = {
  args: Chips.args,
  render: function Render() {
    const block = (state: ScheduleChipState, time: string, title: string) => (
      <div className="flex size-full items-center rounded-md border border-border-strong bg-raised px-10">
        <ScheduleChip state={state} time={time} title={title} className="bg-transparent" />
      </div>
    );
    return (
      <div className="h-420 w-[760px]">
        <DayTimeline
          label="Friday 30 October"
          caption="Fri 30 · times in your timezone"
          now={{ minutes: 12 * 60 + 4, label: "12:04" }}
          blocks={[
            { key: "a", minutes: 12 * 60, node: block("answered", "12:00", "release-watch") },
            { key: "b", minutes: 12 * 60, node: block("running", "12:00", "paper-digest") },
            { key: "c", minutes: 12 * 60, node: block("upcoming", "12:00", "journal-sweep") },
            { key: "d", minutes: 12 * 60, node: block("upcoming", "12:00", "backup-check") },
            { key: "e", minutes: 15 * 60 + 30, node: block("upcoming", "15:30", "invoice-reminder") },
          ]}
        />
      </div>
    );
  },
};
