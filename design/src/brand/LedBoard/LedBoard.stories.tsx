import type { Meta, StoryObj } from "@storybook/react-vite";
import { Wordmark } from "../Wordmark/Wordmark";
import { LedBoard } from "./LedBoard";

const meta = {
  title: "Brand/LedBoard",
  component: LedBoard,
  tags: ["needs-review"],
  parameters: {
    layout: "fullscreen",
    vizier: { id: "B4", replaces: "webui/app/routes/login.tsx" },
    docs: {
      description: {
        component:
          "The mark drawn large on a field of faint dots, each mark cell as 2×2 dots. It fills its container, so the field grows with the panel. Used on the left panel of Login and Onboarding.",
      },
    },
  },
  args: { state: "live", placement: "upper" },
  argTypes: {
    state: { control: "inline-radio", options: ["idle", "live"] },
    placement: { control: "inline-radio", options: ["upper", "center"] },
  },
  decorators: [
    (Story) => (
      <div className="h-640 w-full bg-ground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof LedBoard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Idle: Story = { args: { state: "idle" } };

/** As on Login: the board fills the left panel, with the wordmark and tagline over its lower part. */
export const LoginPanel: Story = {
  render: (args) => (
    <div className="relative size-full">
      <LedBoard {...args} />
      <div className="absolute bottom-48 left-48 flex flex-col gap-12">
        <Wordmark state="live" size={44} />
        <p className="type-body text-fg">A digital steward for Your Majesty.</p>
      </div>
    </div>
  ),
};
