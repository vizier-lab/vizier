import type { Meta, StoryObj } from "@storybook/react-vite";
import { Attachment } from "../Attachment/Attachment";
import { MentionChip } from "../MentionChip/MentionChip";
import { UserMessage } from "./UserMessage";

const meta = {
  title: "Components/Chat/UserMessage",
  component: UserMessage,
  tags: ["approved"],
  parameters: { vizier: { id: "D43", replaces: "webui/app/components/chat_bubble.tsx" } },
  args: { name: "You", seed: "ganes", time: "14:02", children: "Compare the three vector databases we tested last week and file a summary in memory." },
  decorators: [(Story) => <div className="w-640">{Story()}</div>],
} satisfies Meta<typeof UserMessage>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const WithSkill: Story = { args: { skill: "code-review", children: "PR #418 — the queue refactor" } };

export const WithAttachments: Story = {
  args: {
    children: "What changed between these two runs?",
    attachments: (
      <>
        <Attachment name="bench-old.csv" detail="12 KB" />
        <Attachment name="bench-new.csv" detail="9 KB" />
      </>
    ),
  },
};

/** Someone else in a Workspace: on the left, avatar first. */
export const OtherPerson: Story = { args: { side: "left", name: "Maya", seed: "maya", children: "The billing lock is back." } };

/** It names the signed-in person: a lapis edge on the bubble, and the name as a chip. */
export const Mentioned: Story = {
  args: {
    side: "left",
    name: "Maya",
    seed: "maya",
    mentioned: true,
    children: (
      <>
        <MentionChip handle="ganes" tone="you" /> can you take the rollback?
      </>
    ),
  },
};

/** Sent while a turn runs: waits, outlined. */
/** Sent while the agent works: it goes into the turn in progress at the agent's next step (spec 062). */
export const Queued: Story = { args: { children: "Also check the retry path", queued: { agentName: "Archivist" } } };

/** Sent with Ctrl+Enter (or `/queue`) while the agent works: it runs as a new turn after this one. */
export const QueuedAsNewTurn: Story = { args: { children: "Then draft the release notes", queued: { agentName: "Archivist", mode: "queue" } } };

/** A Discord or Telegram conversation: who is connected to a Vizier user, and who is not. */
export const PlatformSenders: Story = {
  render: () => (
    <div className="flex flex-col gap-24">
      <UserMessage name="rin" sender="connected" time="14:02">
        Can you check the deploy?
      </UserMessage>
      <UserMessage name="Kai" seed="552013874411" sender="unregistered" time="14:03">
        me too
      </UserMessage>
    </div>
  ),
};
