import type { Meta, StoryObj } from "@storybook/react-vite";
import { FileTypeTag } from "./FileTypeTag";

const meta = {
  title: "Components/Chat/FileTypeTag",
  component: FileTypeTag,
  tags: ["approved"],
  parameters: { vizier: { id: "X39", replaces: null } },
  args: { file: "usage-october.csv" },
} satisfies Meta<typeof FileTypeTag>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Types: Story = {
  render: () => (
    <div className="flex gap-8">
      {["data.csv", "notes.md", "report.pdf", "graph.png", "budget.xlsx", "export.zip"].map((f) => (
        <FileTypeTag key={f} file={f} />
      ))}
    </div>
  ),
};
