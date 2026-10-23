import type { Meta, StoryObj } from "@storybook/react-vite";
import { Status, statusStates } from "./Status";

const meta = {
  title: "Components/Feedback/Status",
  component: Status,
  tags: ["approved"],
  parameters: {
    vizier: { id: "D24", replaces: null },
    docs: {
      description: {
        component:
          "A dot and words. Agents: live and idle are a running agent (replying or not); starting is building; stopped is off. Task runs, MCP servers and integrations add their own states. Each state's shape and colour is in one table in `Status.tsx`.",
      },
    },
  },
  args: { state: "live", detail: "replying" },
  argTypes: {
    state: { control: "select", options: statusStates },
    font: { control: "inline-radio", options: ["mono", "mono-xs", "sans"] },
  },
} satisfies Meta<typeof Status>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** The six agent states, as on the Design System page. */
export const Agents: Story = {
  render: () => (
    <div className="flex flex-wrap items-center gap-24">
      <Status state="live" detail="replying" />
      <Status state="idle" detail="2h ago" />
      <Status state="failed" detail="retry in 40s" />
      <Status state="retrying" label="retrying in 20s" />
      <Status state="starting" detail="pulling model" />
      <Status state="stopped" />
    </div>
  ),
};

/** Every state, both fonts. */
export const All: Story = {
  render: () => (
    <div className="grid grid-cols-2 gap-x-48 gap-y-10">
      {statusStates.map((s) => (
        <div key={s} className="contents">
          <Status state={s} />
          <Status state={s} font="sans" />
        </div>
      ))}
    </div>
  ),
};
