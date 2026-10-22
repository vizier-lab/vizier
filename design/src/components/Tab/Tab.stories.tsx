import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { Tab, TabList } from "./Tab";

const meta = {
  title: "Components/Shell/Tab",
  component: Tab,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D8", replaces: null } },
  args: { children: "Config", selected: true },
} satisfies Meta<typeof Tab>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  decorators: [(Story) => <TabList aria-label="Example">{Story()}</TabList>],
};

/** The Config page tabs, with the red Danger Zone. */
export const ConfigTabs: Story = {
  render: function Render() {
    const tabs = ["Identity", "Model", "Tools", "Memory", "Sharing", "Danger Zone"];
    const [current, setCurrent] = useState("Model");
    return (
      <TabList aria-label="Agent settings">
        {tabs.map((t) => (
          <Tab key={t} selected={t === current} danger={t === "Danger Zone"} onClick={() => setCurrent(t)}>
            {t}
          </Tab>
        ))}
      </TabList>
    );
  },
};

export const States: Story = {
  parameters: statePseudo,
  render: () => {
    const one = (tab: React.ReactNode) => (
      <TabList aria-label="State" className="border-b-0">
        {tab}
      </TabList>
    );
    return (
      <StateGrid
        rows={[
          {
            cells: interactionCells(() => one(<Tab>Tools</Tab>), [
              { label: "Selected", node: one(<Tab selected>Config</Tab>) },
              { label: "Danger", node: one(<Tab danger>Danger Zone</Tab>) },
              { label: "Disabled", node: one(<Tab disabled>Tools</Tab>) },
            ]),
          },
        ]}
      />
    );
  },
};
