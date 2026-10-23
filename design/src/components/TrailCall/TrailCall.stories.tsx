import type { Meta, StoryObj } from "@storybook/react-vite";
import { TrailCall } from "./TrailCall";

const meta = {
  title: "Components/Chat/TrailCall",
  component: TrailCall,
  tags: ["approved"],
  parameters: { vizier: { id: "D51", replaces: null } },
  args: { name: "execute_python", intent: "compare benchmark notes, render table", duration: "12.6s", state: "done" },
  argTypes: { state: { control: "inline-radio", options: ["done", "running", "failed", "limit", "lookup"] } },
  decorators: [(Story) => <ul className="w-480 rounded-lg bg-surface p-12">{Story()}</ul>],
} satisfies Meta<typeof TrailCall>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Done: Story = {};

export const Running: Story = { args: { state: "running" } };

export const Failed: Story = { args: { state: "failed", duration: "0.6s" } };

export const Limit: Story = { args: { state: "limit", intent: "re-run p99 on all 2.1M passages", duration: "30.0s" } };

/** A lookup: quieter, no time. */
export const Lookup: Story = { args: { state: "lookup", name: "list_tool_functions", intent: "memory · 8 functions" } };
