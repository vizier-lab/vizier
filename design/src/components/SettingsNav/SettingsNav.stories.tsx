import type { Meta, StoryObj } from "@storybook/react-vite";
import { SettingsNav } from "./SettingsNav";

const meta = {
  title: "Components/Shell/SettingsNav",
  component: SettingsNav,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D6", replaces: "webui/app/routes/settingsRoot.tsx" }, layout: "fullscreen" },
  args: { active: "providers", hrefFor: (p) => `#${p}` },
  argTypes: { active: { control: "select", options: ["profile", "password", "api-keys", "providers", "users", "roles"] } },
  decorators: [(Story) => <div className="h-720">{Story()}</div>],
} satisfies Meta<typeof SettingsNav>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** For people who can't administer. */
export const AccountOnly: Story = { args: { active: "profile", showAdministration: false } };
