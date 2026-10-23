import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { StateGrid, statePseudo } from "../../stories/StateGrid";
import { Textarea } from "./Textarea";

const DESC = "Review a diff for bugs, then for clarity. Point at lines, never rewrite whole files.";

const meta = {
  title: "Components/Actions & inputs/Textarea",
  component: Textarea,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D13", replaces: null } },
  args: { label: "Description", defaultValue: DESC, maxLength: 500, helper: "Shown in /skills and the slash menu." },
  decorators: [(Story) => <div className="w-320">{Story()}</div>],
} satisfies Meta<typeof Textarea>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: function Render(args) {
    const [v, setV] = useState(String(args.defaultValue ?? ""));
    return <Textarea {...args} defaultValue={undefined} value={v} onChange={(e) => setV(e.target.value)} />;
  },
};

/** MCP arguments and shell environment. */
export const Mono: Story = {
  args: { label: "Arguments", mono: true, maxLength: undefined, helper: "One per line.", defaultValue: "--port\n8931\n--headless" },
};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  decorators: [(Story) => <div>{Story()}</div>],
  render: () => {
    const f = (props: Partial<React.ComponentProps<typeof Textarea>> = {}) => (
      <div className="w-280">
        <Textarea label="Description" defaultValue={DESC} maxLength={500} rows={3} helper="Shown in /skills and the slash menu." {...props} />
      </div>
    );
    return (
      <StateGrid
        rows={[
          {
            cells: [
              { label: "Default", node: f() },
              { label: "Placeholder", node: f({ defaultValue: undefined, placeholder: "e.g. Review a diff for bugs" }) },
              { label: "Hover", pseudo: "hover", node: f() },
            ],
          },
          {
            cells: [
              { label: "Focus", pseudo: "focus-within", node: f() },
              { label: "Error", node: f({ defaultValue: DESC.repeat(7), helper: "Too long." }) },
              { label: "Disabled", node: f({ disabled: true }) },
            ],
          },
        ]}
      />
    );
  },
};
