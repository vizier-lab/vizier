import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ProviderLogo } from "../ProviderLogo/ProviderLogo";
import { StateGrid, statePseudo } from "../../stories/StateGrid";
import { Select } from "./Select";

const PROTOCOLS = [
  { value: "openai", label: "OpenAI-compatible" },
  { value: "anthropic", label: "Anthropic" },
];

const meta = {
  title: "Components/Actions & inputs/Select",
  component: Select,
  tags: ["needs-review"],
  parameters: {
    vizier: { id: "D14", replaces: "webui/app/components/CustomSelect.tsx" },
    docs: { description: { component: "For short fixed lists. Open with click, ↓ or ↑; then ↑ ↓ move, Enter picks, Esc closes. Long or searchable lists use Picker." } },
  },
  args: { label: "Protocol", options: PROTOCOLS, value: "openai", onChange: () => {} },
  decorators: [(Story) => <div className="h-200 w-280">{Story()}</div>],
} satisfies Meta<typeof Select>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: function Render(args) {
    const [v, setV] = useState<string | undefined>(args.value);
    return <Select {...args} value={v} onChange={setV} />;
  },
};

export const Placeholder: Story = { args: { value: undefined, placeholder: "Choose a protocol" } };

/** The model select: provider logos, mono values. */
export const ModelSelect: Story = {
  render: function Render() {
    const [v, setV] = useState("anthropic/claude-sonnet-5-5");
    return (
      <Select
        label="Model"
        mono
        value={v}
        onChange={setV}
        options={[
          { value: "anthropic/claude-sonnet-5-5", label: "Anthropic · claude-sonnet-5-5", leading: <ProviderLogo provider="anthropic" name="Anthropic" size="sm" /> },
          { value: "anthropic/claude-haiku-5-5", label: "Anthropic · claude-haiku-5-5", leading: <ProviderLogo provider="anthropic" name="Anthropic" size="sm" /> },
          { value: "ollama/qwen3:32b", label: "Ollama · qwen3:32b", leading: <ProviderLogo provider="ollama" name="Ollama" size="sm" /> },
        ]}
      />
    );
  },
};

/** A role, compact, in a Users table row. */
export const BadgeLook: Story = {
  render: function Render() {
    const [v, setV] = useState("member");
    return (
      <Select
        look="badge"
        aria-label="Role for ana"
        value={v}
        onChange={setV}
        options={[
          { value: "admin", label: "admin" },
          { value: "member", label: "member" },
          { value: "viewer", label: "viewer" },
        ]}
      />
    );
  },
};

export const States: Story = {
  parameters: statePseudo,
  decorators: [(Story) => <div className="h-260">{Story()}</div>],
  render: () => {
    const s = (p: Partial<React.ComponentProps<typeof Select>> = {}) => (
      <div className="w-240">
        <Select label="Protocol" options={PROTOCOLS} value="openai" onChange={() => {}} {...p} />
      </div>
    );
    return (
      <StateGrid
        rows={[
          {
            cells: [
              { label: "Default", node: s() },
              { label: "Placeholder", node: s({ value: undefined, placeholder: "Choose a protocol" }) },
              { label: "Hover", pseudo: "hover", node: s() },
              { label: "Focus", pseudo: "focus-visible", node: s() },
            ],
          },
          {
            cells: [
              { label: "Error", node: s({ invalid: true, helper: "Pick a protocol." }) },
              { label: "Disabled", node: s({ disabled: true }) },
            ],
          },
        ]}
      />
    );
  },
};
