import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button/Button";
import { Segment } from "../Segment/Segment";
import { Step } from "../Step/Step";
import { UnsavedIndicator } from "../UnsavedIndicator/UnsavedIndicator";
import { PageHeader } from "./PageHeader";

const meta = {
  title: "Components/Shell/PageHeader",
  component: PageHeader,
  tags: ["approved"],
  parameters: { vizier: { id: "D7", replaces: null } },
  args: {
    title: "Tasks",
    meta: "312 tasks · 287 active · next run Mon 09:00",
    actions: <Button variant="primary" icon="plus">New task</Button>,
  },
  decorators: [(Story) => <div className="w-900">{Story()}</div>],
} satisfies Meta<typeof PageHeader>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Core: unsaved edits and their buttons. */
export const WithUnsaved: Story = {
  args: {
    title: "Core",
    meta: "Archivist · rev 14 · edited 2h ago",
    actions: (
      <>
        <UnsavedIndicator />
        <Button>Discard</Button>
        <Button variant="primary" shortcut="Ctrl S">Save</Button>
      </>
    ),
  },
};

/** Usage: a range and a breakdown in the right slot. */
export const WithRange: Story = {
  args: {
    title: "Usage",
    meta: "Since Oct 1 · all agents",
    actions: (
      <div role="group" aria-label="Range" className="inline-flex rounded-md border border-border-strong bg-surface p-2">
        <Segment selected>7 days</Segment>
        <Segment>30 days</Segment>
        <Segment>Custom</Segment>
      </div>
    ),
  },
};

/** New agent: the stepper in the right slot. */
export const WithStepper: Story = {
  args: {
    title: "New agent",
    meta: undefined,
    actions: (
      <ol className="flex items-center gap-12">
        <Step number={1} label="Config" state="done" />
        <Step number={2} label="System Prompt" state="done" />
        <Step number={3} label="Tools" state="current" />
        <Step number={4} label="Review" />
      </ol>
    ),
  },
};

/** Settings pages: smaller title with a description. */
export const Settings: Story = {
  args: {
    size: "settings",
    title: "Providers",
    meta: undefined,
    description: "Keys and accounts for the model providers your agents use. Saved keys are never shown again.",
    actions: <Button icon="plus">Add custom provider</Button>,
  },
};
