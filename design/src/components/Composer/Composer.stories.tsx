import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Attachment } from "../Attachment/Attachment";
import { SwitchRow } from "../SwitchRow/SwitchRow";
import { VoiceRecorder } from "../VoiceRecorder/VoiceRecorder";
import { StateGrid, statePseudo } from "../../stories/StateGrid";
import { Composer } from "./Composer";

const SKILLS = [
  { name: "code-review", description: "Review a diff for bugs, then for clarity" },
  { name: "compare-tables", description: "Turn benchmark output into a table" },
  { name: "checkpoint", description: "Save checkpoint with handover" },
];

const CONTEXT = { used: 156000, window: 200000, detail: "claude-sonnet-5-5 · checkpoints automatically" };

const meta = {
  title: "Components/Chat/Composer",
  component: Composer,
  tags: ["approved"],
  parameters: {
    vizier: { id: "D46", replaces: null },
    docs: { description: { component: "Enter sends, Shift+Enter breaks the line. Type / for the skill menu: ↑ ↓ move, Tab completes, Enter runs. Send stays disabled until there is text or an attachment; while the agent works it becomes Stop." } },
  },
  args: { agentName: "Archivist", value: "", onValueChange: () => {}, onSend: () => {}, skills: SKILLS, context: CONTEXT },
  decorators: [(Story) => <div className="flex min-h-260 w-720 flex-col justify-end">{Story()}</div>],
} satisfies Meta<typeof Composer>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Try it: type, or type "/". */
export const Default: Story = {
  render: function Render(args) {
    const [v, setV] = useState("");
    const [busy, setBusy] = useState(false);
    return <Composer {...args} value={v} onValueChange={setV} busy={busy} onSend={() => { setV(""); setBusy(true); }} onStop={() => setBusy(false)} />;
  },
};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  decorators: [(Story) => <div className="w-720">{Story()}</div>],
  render: (args) => {
    const c = (p: Partial<React.ComponentProps<typeof Composer>> = {}) => (
      <div className="w-640">
        <Composer {...args} skills={[]} {...p} />
      </div>
    );
    return (
      <StateGrid
        rows={[
          { cells: [{ label: "Idle", node: c() }] },
          { cells: [{ label: "Busy", node: c({ busy: true, onStop: () => {} }) }] },
          { cells: [{ label: "Focus", pseudo: "focus-within", node: c() }] },
          { cells: [{ label: "Filled", node: c({ value: "Compare LanceDB and sqlite-vec on our bench, then summarise" }) }] },
          { cells: [{ label: "Recording", node: c({ recording: true }) }] },
          { cells: [{ label: "Disabled", node: c({ disabledReason: "Reconnecting… you can send once the connection is back" }) }] },
        ]}
      />
    );
  },
};

/** The skill menu, filtering as you type. */
export const SkillsOpen: Story = { args: { value: "/co" } };

export const WithAttachments: Story = {
  args: {
    value: "What changed between these two runs?",
    attachments: (
      <>
        <Attachment name="bench-old.csv" detail="12 KB" onRemove={() => {}} />
        <Attachment name="bench-new.csv" detail="9 KB" onRemove={() => {}} />
        <Attachment name="flame.png" detail="88 KB" onRemove={() => {}} />
      </>
    ),
  },
};

/** Recorded: listen back, then send or discard; ask for a spoken reply. */
export const VoiceRecorded: Story = {
  args: {
    voice: <VoiceRecorder mode="recorded" elapsed={12} onDiscard={() => {}} />,
    below: <SwitchRow label="Ask for a spoken reply" checked onCheckedChange={() => {}} />,
  },
};

/** The agent can't take messages right now. */
/** An agent without Speech-to-Text: no mic, the rest of the tools stay put. */
export const WithoutMic: Story = { args: { mic: false } };

export const AgentUnavailable: Story = { args: { disabledReason: "Archivist can't answer until it restarts" } };

/** Mobile: the token count as text. */
export const MobileTokens: Story = { args: { context: undefined, tokenText: "21k / 200k" }, decorators: [(Story) => <div className="w-375">{Story()}</div>] };

/** A project chat: typing "@" opens the file menu; ↑↓ move, Tab or Enter mention, Esc closes. */
export const MentioningAFile: Story = {
  args: {
    value: "Make ps match @agent",
    suggest: {
      label: "Files",
      query: "agent",
      items: [
        { path: "crates/vizier/src/cli/agent.rs", kind: "file" },
        { path: "crates/vizier-core/src/agents/mod.rs", kind: "file" },
        { path: "crates/vizier-core/src/agents/", kind: "dir" },
      ],
      onPick: () => {},
      onDismiss: () => {},
    },
  },
  decorators: [(Story) => <div className="flex h-240 items-end">{Story()}</div>],
};
