import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { Icon } from "../../icons/Icon";
import { Badge } from "../Badge/Badge";
import { ListItem } from "../ListItem/ListItem";
import { HighlightMatch, Picker, PickerAction } from "../Picker/Picker";
import { manyAgents, recentAgents } from "../Picker/fixtures";
import { Status } from "../Status/Status";
import { StateGrid, statePseudo } from "../../stories/StateGrid";
import { AgentSwitcher } from "./AgentSwitcher";

const meta = {
  title: "Components/Shell/AgentSwitcher",
  component: AgentSwitcher,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D4", replaces: null } },
  args: { agentId: "archivist", name: "Archivist", state: "live", detail: "replying" },
  decorators: [(Story) => <div className="w-212">{Story()}</div>],
} satisfies Meta<typeof AgentSwitcher>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Collapsed: Story = { args: { collapsed: true } };

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  decorators: [(Story) => <div>{Story()}</div>],
  render: (args) => (
    <StateGrid
      rows={[false, true].map((collapsed) => ({
        label: collapsed ? "collapsed" : "expanded",
        cells: [
          { label: "Default", node: <div className={collapsed ? "" : "w-212"}><AgentSwitcher {...args} collapsed={collapsed} /></div> },
          { label: "Hover", pseudo: "hover" as const, node: <div className={collapsed ? "" : "w-212"}><AgentSwitcher {...args} collapsed={collapsed} /></div> },
          { label: "Open", node: <div className={collapsed ? "" : "w-212"}><AgentSwitcher {...args} collapsed={collapsed} aria-expanded /></div> },
        ],
      }))}
    />
  ),
};

/** Click it: the open state is the agent Picker. */
export const Open: Story = {
  decorators: [(Story) => <div className="h-480 w-212">{Story()}</div>],
  render: function Render(args) {
    const [q, setQ] = useState("");
    const [open, setOpen] = useState(true);
    const all = manyAgents.filter((a) => a.name.toLowerCase().includes(q.toLowerCase()));
    const shown = q ? all : all.slice(0, 3);
    return (
      <Picker
        label="Agents"
        open={open}
        onOpenChange={setOpen}
        trigger={<AgentSwitcher {...args} />}
        search={{ placeholder: "Search agents", total: 48, value: q, onChange: setQ }}
        sections={q ? [{ items: shown }] : [{ label: "Recent", items: recentAgents }, { label: "All · A–Z", items: shown }]}
        getKey={(a) => a.id}
        renderItem={(a, { query }) => (
          <ListItem
            shape="bare"
            className="w-full"
            leading={<Sigil agentId={a.id} live={a.state === "live"} size="sm" />}
            title={<HighlightMatch text={a.name} query={query} />}
            subtitle={<Status state={a.state} detail={a.detail} font="mono-xs" dot={false} />}
            trailing={
              <>
                {a.owner && <Badge tone="rank">Owner</Badge>}
                {a.id === "archivist" && <Icon name="check" size={14} title="Current agent" />}
              </>
            }
          />
        )}
        onSelect={() => {}}
        remainder={q ? undefined : "42 more · type to filter"}
        footer={<PickerAction icon="plus">Create agent</PickerAction>}
      />
    );
  },
};
