import type { Meta, StoryObj } from "@storybook/react-vite";
import { InteractivePage } from "./InteractivePage";

const meta = {
  title: "Components/Chat/InteractivePage",
  component: InteractivePage,
  tags: ["approved"],
  parameters: { vizier: { id: "X47", replaces: "webui/app/components/client/InteractivePage.tsx" } },
  args: {
    title: "Benchmark explorer",
    onFullscreen: () => {},
    children: (
      <div className="flex h-160 items-end gap-16 px-16 pt-16 pb-12">
        {[
          ["Qdrant", "h-40"],
          ["sqlite-vec", "h-64"],
          ["pgvector", "h-100"],
        ].map(([n, h]) => (
          <div key={n} className="flex flex-1 flex-col items-center gap-6">
            <div className={`w-full rounded-xs bg-chart-4 ${h}`} />
            <span className="type-mono-xs text-muted">{n}</span>
          </div>
        ))}
      </div>
    ),
  },
  decorators: [(Story) => <div className="w-396">{Story()}</div>],
} satisfies Meta<typeof InteractivePage>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The body here stands in for the sandboxed iframe. */
export const Default: Story = {};
