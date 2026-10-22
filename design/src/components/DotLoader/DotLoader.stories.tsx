import type { Meta, StoryObj } from "@storybook/react-vite";
import { toneModes } from "../../tokens/tokens";
import { DotLoader } from "./DotLoader";

const meta = {
  title: "Components/Feedback/DotLoader",
  component: DotLoader,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D32", replaces: "webui/app/components/dot_loader.tsx" } },
  args: { tone: "muted", label: "Loading" },
  argTypes: { tone: { control: "select", options: toneModes } },
} satisfies Meta<typeof DotLoader>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Every tone. Inverse sits on ink, as inside a primary button. */
export const Tones: Story = {
  render: () => (
    <div className="flex items-center gap-24">
      {toneModes.map((tone) => (
        <span key={tone} className={tone === "inverse" ? "flex rounded-sm bg-ink p-8" : "flex p-8"}>
          <DotLoader tone={tone} label={`Loading, ${tone}`} />
        </span>
      ))}
    </div>
  ),
};
