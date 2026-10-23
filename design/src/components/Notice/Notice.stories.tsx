import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button/Button";
import { TextLink } from "../TextLink/TextLink";
import { Notice } from "./Notice";

const meta = {
  title: "Components/Feedback/Notice",
  component: Notice,
  tags: ["approved"],
  parameters: { vizier: { id: "D25", replaces: null } },
  args: {
    tone: "failed",
    title: "Scribe couldn't start",
    children: 'Model "llama3.3:70b" not found on Ollama. Messages wait in line until it starts.',
    actions: <Button size="sm">Retry now</Button>,
  },
  argTypes: { tone: { control: "inline-radio", options: ["failed", "retrying", "info"] } },
  decorators: [(Story) => <div className="w-520">{Story()}</div>],
} satisfies Meta<typeof Notice>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Failed: Story = {};

export const Retrying: Story = {
  args: { tone: "retrying", title: "Reconnecting to linear", children: "Timed out listing tools · retry in 20s. Its tools are left out until then." },
};

export const Info: Story = {
  args: { tone: "info", title: "Memory updated", children: "infra/vector-dbs/summary.md · rev 3", actions: undefined },
};

/** Several text actions, as in a failed chat turn. */
export const SeveralActions: Story = {
  args: {
    title: "The model stopped answering",
    children: "anthropic · claude-sonnet-5-5 returned 529 (overloaded) three times.",
    actions: (
      <>
        <TextLink>Retry</TextLink>
        <TextLink>Switch model</TextLink>
      </>
    ),
  },
};

/** A limit: amber, with the alert icon. */
export const Limit: Story = {
  args: { tone: "retrying", icon: "alert", title: "Stopped at the 30 s script limit", children: "The script was still running. Split the work, or raise the limit in Config.", actions: undefined },
};

/** Compact, inside the activity trail. */
export const Compact: Story = {
  args: { compact: true, title: undefined, children: "memory_write failed: bundle “infra” is read-only", actions: <TextLink>Retry</TextLink> },
};

/** In chat, in place of a reply: a dot, the problem, and the action that fixes it. */
export const ChatError: Story = {
  args: {
    marker: "dot",
    tone: "retrying",
    title: "Rate limited",
    children: "anthropic asked to wait. Retried 4 times over 90 s, then gave up.",
    actions: (
      <>
        <TextLink>Retry</TextLink>
        <TextLink>Switch model</TextLink>
      </>
    ),
  },
};
