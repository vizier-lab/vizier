import type { Meta, StoryObj } from "@storybook/react-vite";
import { NightStrip, type Night } from "./NightStrip";

const LABELS: Record<number, string> = { 0: "Sep 21", 5: "Sep 26", 10: "Oct 1", 15: "Oct 6", 20: "Oct 11", 25: "Oct 16", 29: "today" };
const M = [1, 6, 5, 7, 6, 5, 2, 6, 5, 0, 7, 5, 5, 2, 2, 6, 5, 1, 7, 6, 2, 6, 0, 1, 1, 6, 2, 2, 2, 2];
const CORE = new Set([1, 2, 3, 4, 5, 7, 8, 10, 11, 12, 15, 16, 18, 19, 21, 25]);

const nights: Night[] = M.map((m, i) => ({ memories: m, core: CORE.has(i), label: LABELS[i] }));

const meta = {
  title: "Components/Visualisation/NightStrip",
  component: NightStrip,
  tags: ["approved"],
  parameters: { vizier: { id: "X69", replaces: "webui/app/routes/agent-dream.tsx" } },
  args: { nights },
  decorators: [(Story) => <div className="w-1010">{Story()}</div>],
} satisfies Meta<typeof NightStrip>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Dreams. */
export const Default: Story = {};
