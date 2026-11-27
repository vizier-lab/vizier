import type { Meta, StoryObj } from "@storybook/react-vite";
import { ConnectConfirm, connectConfirmTitle } from "./ConnectConfirm";

const account = { platform: "discord", accountId: "381926457021", handle: "rin_dev", displayName: "Rin" } as const;

const meta = {
  title: "Components/Integrations & settings/ConnectConfirm",
  component: ConnectConfirm,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X95", replaces: null } },
  args: { state: "ready", account, username: "rin", onConfirm: () => {}, onCancel: () => {}, onSignOut: () => {}, onRetry: () => {} },
  // The /connect page holds it like this; in Settings a Dialog does.
  render: (args) => (
    <div className="flex w-440 max-w-full flex-col gap-16 rounded-xl border border-border bg-surface p-24">
      <h1 className="type-heading text-fg">{connectConfirmTitle(args)}</h1>
      <ConnectConfirm {...args} />
    </div>
  ),
} satisfies Meta<typeof ConnectConfirm>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Ready: Story = {};

/** The person already has another account on this platform. */
export const Replaces: Story = {
  args: { replaces: { platform: "discord", accountId: "771002", handle: "rin_old" } },
};

export const Connecting: Story = { args: { busy: true } };

/** The confirmation was refused; the person can try again. */
export const ConfirmFailed: Story = { args: { message: "Too many attempts. Try again later." } };

export const Loading: Story = { args: { state: "loading" } };

/** Unknown, expired and used codes read the same. */
export const Unavailable: Story = { args: { state: "unavailable" } };

export const LinkedElsewhere: Story = {
  args: { state: "elsewhere", message: "This Discord account is already connected to another Vizier user." },
};

export const Failed: Story = { args: { state: "failed", message: "The server could not be reached." } };
