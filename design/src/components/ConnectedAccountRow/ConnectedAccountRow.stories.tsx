import type { Meta, StoryObj } from "@storybook/react-vite";
import { ConnectedAccountRow } from "./ConnectedAccountRow";

const meta = {
  title: "Components/Integrations & settings/ConnectedAccountRow",
  component: ConnectedAccountRow,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X94", replaces: null } },
  args: {
    platform: "discord",
    state: "connected",
    handle: "rin_dev",
    accountId: "381926457021",
    since: "Connected 27 Nov 2026",
    onUnlink: () => {},
    onRemove: () => {},
  },
  decorators: [(Story) => <div className="flex w-760 max-w-full flex-col gap-8 rounded-xl border border-border bg-surface p-16">{Story()}</div>],
} satisfies Meta<typeof ConnectedAccountRow>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Proven with a code. ⋯ holds Unlink. */
export const Connected: Story = {};

/** A Telegram account with no handle. */
export const WithoutHandle: Story = { args: { platform: "telegram", handle: null, accountId: "55120" } };

/** Typed in before accounts could be proven: it identifies nobody until confirmed. */
export const Unverified: Story = { args: { platform: "telegram", state: "unverified", handle: "rin", accountId: "55120" } };

export const Removing: Story = { args: { ...Unverified.args, busy: true } };

export const NotConnected: Story = { args: { platform: "telegram", state: "none", handle: null, accountId: null, since: undefined } };

/** Settings › Profile. */
export const Profile: Story = {
  render: () => (
    <>
      <ConnectedAccountRow platform="discord" state="connected" handle="rin_dev" accountId="381926457021" since="Connected 27 Nov 2026" onUnlink={() => {}} />
      <ConnectedAccountRow platform="telegram" state="none" />
    </>
  ),
};
