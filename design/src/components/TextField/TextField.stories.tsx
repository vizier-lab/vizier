import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, statePseudo } from "../../stories/StateGrid";
import { TextField } from "./TextField";

const meta = {
  title: "Components/Actions & inputs/TextField",
  component: TextField,
  tags: ["approved"],
  parameters: {
    vizier: { id: "D12", replaces: null },
    docs: { description: { component: "Text entry focuses emerald: typing is live. Every other control focuses with the neutral ring." } },
  },
  args: { label: "Name", defaultValue: "Archivist", helper: "Shown to people who chat with it." },
  decorators: [(Story) => <div className="w-280">{Story()}</div>],
} satisfies Meta<typeof TextField>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Placeholder: Story = { args: { defaultValue: undefined, placeholder: "e.g. Archivist" } };

export const Invalid: Story = {
  args: { label: "Confirm password", type: "password", defaultValue: "hunter22", invalid: true, helper: "Passwords do not match." },
};

/** Screen extensions: mono value, a unit, a locked id, a password, the Lg login field. */
export const Extensions: Story = {
  decorators: [(Story) => <div className="flex w-320 flex-col gap-20">{Story()}</div>],
  render: () => (
    <>
      <TextField label="Model" mono defaultValue="qwen3:32b" />
      <TextField label="Context window" mono defaultValue="200000" suffix="tokens" inputMode="numeric" />
      <TextField label="Agent ID" mono locked defaultValue="archivist" helper="Set when the agent was created." />
      <TextField label="Password" type="password" defaultValue="correct horse" />
      <TextField label="Username" size="lg" defaultValue="ganesha" autoComplete="username" />
    </>
  ),
};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  decorators: [(Story) => <div>{Story()}</div>],
  render: () => {
    const f = (props: Partial<React.ComponentProps<typeof TextField>> = {}) => (
      <div className="w-240">
        <TextField label="Name" defaultValue="Archivist" helper="Shown to people who chat with it." {...props} />
      </div>
    );
    return (
      <StateGrid
        rows={[
          {
            cells: [
              { label: "Default", node: f() },
              { label: "Placeholder", node: f({ defaultValue: undefined, placeholder: "e.g. Archivist" }) },
              { label: "Hover", pseudo: "hover", node: f() },
              { label: "Focus", pseudo: "focus-within", node: f() },
            ],
          },
          {
            cells: [
              { label: "Error", node: f({ invalid: true, helper: "Names must be unique." }) },
              { label: "Disabled", node: f({ disabled: true }) },
              { label: "Read-only", node: f({ label: "Agent ID", defaultValue: "archivist", readOnly: true, helper: "Set when the agent was created." }) },
              { label: "Locked", node: f({ label: "Agent ID", defaultValue: "archivist", locked: true, mono: true, helper: "Set when the agent was created." }) },
            ],
          },
        ]}
      />
    );
  },
};
