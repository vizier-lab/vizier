import type { Meta, StoryObj } from "@storybook/react-vite";
import { IconButton } from "../IconButton/IconButton";
import { FileRow } from "./FileRow";

const meta = {
  title: "Components/Lists/FileRow",
  component: FileRow,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X25", replaces: null } },
  args: { name: "checklist.md", size: "2 KB" },
  decorators: [(Story) => <div className="w-320">{Story()}</div>],
} satisfies Meta<typeof FileRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Session files, with download and remove. */
export const List: Story = {
  render: () => (
    <div className="flex flex-col">
      {[
        ["checklist.md", "2 KB"],
        ["examples/race.diff", "4 KB"],
        ["usage-october.csv", "12 KB"],
      ].map(([n, s]) => (
        <FileRow
          key={n}
          name={n}
          size={s}
          actions={
            <>
              <IconButton variant="ghost" size="sm" icon="download" aria-label={`Download ${n}`} />
              <IconButton variant="ghost" size="sm" icon="trash" aria-label={`Remove ${n}`} />
            </>
          }
        />
      ))}
    </div>
  ),
};
