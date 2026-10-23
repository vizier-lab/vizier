import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button/Button";
import { Eyebrow } from "../Eyebrow/Eyebrow";
import { KeyValueList } from "../KeyValueList/KeyValueList";
import { SecretField } from "../SecretField/SecretField";
import { ToolList } from "../ToolList/ToolList";
import { IntegrationCard } from "./IntegrationCard";

const meta = {
  title: "Components/Integrations & settings/IntegrationCard",
  component: IntegrationCard,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X59", replaces: "webui/app/components/integrations.tsx" } },
  args: {
    kind: "discord",
    name: "Discord",
    handle: "Archivist#4821 · 3 servers",
    state: "reading",
    enabled: true,
    onEnabledChange: () => {},
    children: (
      <KeyValueList
        items={[
          { label: "Token", value: <SecretField masked="••••••••4f2a" onReplace={() => {}} /> },
          { label: "Slash commands", value: "9 synced · 2 min ago" },
          { label: "Silent reading", value: "off (0.0)" },
          { label: "Linked", value: "created here by ganesha · Sep 12" },
        ]}
      />
    ),
    actions: (
      <>
        <Button>Hide activity</Button>
        <Button variant="danger" icon="trash">Delete</Button>
      </>
    ),
  },
  decorators: [(Story) => <div className="w-630">{Story()}</div>],
} satisfies Meta<typeof IntegrationCard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Reading: Story = {};

/** The reader stopped: why, and the fix. */
export const Failed: Story = {
  args: {
    kind: "telegram",
    name: "Telegram",
    handle: "@archivist_bot",
    state: "failed",
    failure: {
      title: "Telegram rejected the token (401 Unauthorized).",
      message: "The reader stopped and will not retry until the token is replaced. Other channels and the agent keep running.",
    },
    children: undefined,
    actions: (
      <>
        <Button variant="primary" icon="key">Replace token</Button>
        <Button variant="danger" icon="trash">Delete</Button>
      </>
    ),
  },
};

/** An addon: settings and the tools it adds. */
export const Installed: Story = {
  args: {
    kind: "brave_search",
    name: "Brave Search",
    handle: "Web and news search through the Brave API",
    state: "installed",
    children: (
      <div className="flex flex-col gap-16">
        <SecretField label="API key" masked="••••••••••••b71c" onReplace={() => {}} />
        <div className="flex flex-col gap-8">
          <Eyebrow>Tools it adds</Eyebrow>
          <ToolList
            tools={[
              { name: "brave_search_web", description: "Search the web; returns titles, links and snippets" },
              { name: "brave_search_news", description: "Search recent news articles" },
            ]}
          />
        </div>
      </div>
    ),
    actions: <Button variant="danger" icon="unlink">Unlink from this agent</Button>,
  },
};

export const Disabled: Story = { args: { state: "disabled", enabled: false } };
