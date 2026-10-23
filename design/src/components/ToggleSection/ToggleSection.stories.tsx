import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Select } from "../Select/Select";
import { TextField } from "../TextField/TextField";
import { ToggleSection } from "./ToggleSection";

const meta = {
  title: "Components/Actions & inputs/ToggleSection",
  component: ToggleSection,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X17", replaces: "webui/app/routes/agent-new.tsx" } },
  args: { title: "Text-to-Speech", description: "Speak replies aloud when asked.", checked: true, onCheckedChange: () => {} },
  decorators: [(Story) => <div className="w-720">{Story()}</div>],
} satisfies Meta<typeof ToggleSection>;

export default meta;
type Story = StoryObj<typeof meta>;

/** On: its settings show. Toggle it. */
export const On: Story = {
  render: function Render(args) {
    const [on, setOn] = useState(args.checked);
    return (
      <ToggleSection {...args} checked={on} onCheckedChange={setOn}>
        <div className="grid grid-cols-2 gap-16">
          <Select
            label="Provider"
            value="elevenlabs"
            onChange={() => {}}
            options={[
              { value: "elevenlabs", label: "ElevenLabs" },
              { value: "openai", label: "OpenAI" },
            ]}
          />
          <TextField label="Voice" mono defaultValue="rachel" />
        </div>
      </ToggleSection>
    );
  },
};

export const Off: Story = { args: { title: "Shell", description: "Run commands in a sandbox.", checked: false } };
