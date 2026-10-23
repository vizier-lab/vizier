import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { StateGrid, statePseudo } from "../../stories/StateGrid";
import { SearchField } from "./SearchField";

const meta = {
  title: "Components/Actions & inputs/SearchField",
  component: SearchField,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D15", replaces: null } },
  args: { value: "", onValueChange: () => {}, "aria-label": "Search agents", placeholder: "Search 48 agents" },
  decorators: [(Story) => <div className="w-280">{Story()}</div>],
} satisfies Meta<typeof SearchField>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: function Render(args) {
    const [q, setQ] = useState("");
    return <SearchField {...args} value={q} onValueChange={setQ} />;
  },
};

/** With a count, as the Picker header and on long lists. */
export const WithCount: Story = {
  render: function Render(args) {
    const [q, setQ] = useState("pine");
    return <SearchField {...args} aria-label="Search memory" placeholder="Search 1,024 memories" value={q} onValueChange={setQ} count={q ? "38 of 1,024" : "1,024"} />;
  },
};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  decorators: [(Story) => <div>{Story()}</div>],
  render: () => {
    const f = (value: string, extra: Partial<React.ComponentProps<typeof SearchField>> = {}) => (
      <div className="w-220">
        <SearchField value={value} onValueChange={() => {}} aria-label="Search agents" placeholder="Search 48 agents" {...extra} />
      </div>
    );
    return (
      <StateGrid
        rows={[
          {
            cells: [
              { label: "Empty", node: f("") },
              { label: "Hover", pseudo: "hover", node: f("") },
              { label: "Focus", pseudo: "focus-within", node: f("") },
              { label: "Filled", node: f("pinecone") },
              { label: "Disabled", node: f("", { disabled: true }) },
            ],
          },
        ]}
      />
    );
  },
};
