import type { Meta, StoryObj } from "@storybook/react-vite";
import { Table } from "../Table/Table";
import { ServerRow } from "./ServerRow";

const meta = {
  title: "Components/Integrations & settings/ServerRow",
  component: ServerRow,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X62", replaces: "webui/app/routes/integrations-mcp.tsx" } },
  args: { name: "github", transport: "remote", target: "https://api.githubcopilot.com/mcp", state: "connected", category: "github", tools: 34, enabled: true, onEnabledChange: () => {} },
  decorators: [
    (Story) => (
      <div className="w-630">
        <Table label="MCP servers" columns={[{ label: "Server", width: 220 }, { label: "Status", width: 160 }, { label: "Category", width: 120 }, { label: "Tools", width: 60 }, { label: "On" }]}>
          {Story()}
        </Table>
      </div>
    ),
  ],
} satisfies Meta<typeof ServerRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Connected: Story = {};

/** The MCP servers table. */
export const Table4: Story = {
  name: "Servers",
  render: () => (
    <>
      <ServerRow name="github" transport="remote" target="https://api.githubcopilot.com/mcp" state="connected" category="github" tools={34} enabled onEnabledChange={() => {}} />
      <ServerRow name="filesystem" transport="local" target="npx @modelcontextprotocol/server-filesystem" state="connected" category="filesystem" tools={11} enabled onEnabledChange={() => {}} />
      <ServerRow name="linear" transport="remote" target="https://mcp.linear.app/sse" state="reconnecting" detail="timed out listing tools · retry in 20s" category="linear" enabled onEnabledChange={() => {}} />
      <ServerRow name="scratchpad" transport="local" target="uvx scratch-mcp" state="disabled" category="scratchpad_mcp" enabled={false} onEnabledChange={() => {}} />
    </>
  ),
};
