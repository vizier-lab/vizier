import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Icon } from "../../icons/Icon";
import { EditorToolbar, type EditorMode } from "./EditorToolbar";

const meta = {
  title: "Components/Rich text/EditorToolbar",
  component: EditorToolbar,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X50", replaces: "webui/app/components/MarkdownEditor.tsx" } },
  args: { blockType: "Paragraph", onCommand: () => {}, count: "1,284 chars" },
  decorators: [(Story) => <div className="w-680 rounded-xl border border-border bg-surface">{Story()}</div>],
} satisfies Meta<typeof EditorToolbar>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Core. */
export const Compact: Story = {};

/** The skill editor. */
export const Full: Story = {
  decorators: [(Story) => <div className="w-1040">{Story()}</div>],
  render: function Render(args) {
    const [mode, setMode] = useState<EditorMode>("rich");
    return (
      <EditorToolbar
        {...args}
        look="full"
        count={undefined}
        active={["bold"]}
        blockType={
          <span className="inline-flex items-center gap-6 rounded-sm border border-border-strong px-8 py-4 type-ui-medium text-fg">
            Heading 2 <Icon name="chevron-down" size={12} />
          </span>
        }
        mode={mode}
        onModeChange={setMode}
        onFullscreen={() => {}}
      />
    );
  },
};
