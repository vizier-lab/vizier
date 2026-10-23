import type { Meta, StoryObj } from "@storybook/react-vite";
import { DiffView } from "./DiffView";

const meta = {
  title: "Components/Rich text/DiffView",
  component: DiffView,
  tags: ["approved"],
  parameters: { vizier: { id: "X52", replaces: "webui/app/components/VersionHistory.tsx" } },
  args: {
    title: "rev 40 → 41",
    lines: [
      { kind: "ctx", text: "## Lately" },
      { kind: "del", text: "• Ask Scout for web checks when convenient." },
      { kind: "add", text: "• Delegate release-note checks to Scout; it is faster and cheaper for web work." },
      { kind: "add", text: "• Keep benchmark numbers in comparison notes." },
    ],
    restoreLabel: "Restore rev 40",
    onRestore: () => {},
  },
  decorators: [(Story) => <div className="w-352">{Story()}</div>],
} satisfies Meta<typeof DiffView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Core's History panel. */
export const Default: Story = {};
