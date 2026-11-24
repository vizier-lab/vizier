// lint-tokens-exempt: a story has to supply a palette that isn't the page's own.
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button";
import { CodeBlock } from "../CodeBlock";
import { ThemeScope } from "./ThemeScope";

const VARS = {
  "--vz-panel": "#282a36",
  "--vz-surface": "#343746",
  "--vz-border": "#44475a",
  "--vz-text": "#f8f8f2",
  "--vz-text-soft": "#f8f8f2",
  "--vz-text-faint": "#9aa5ce",
  "--vz-ink": "#bd93f9",
  "--vz-border-strong": "#44475a",
  "--vz-ground": "#282a36",
  "--vz-code-keyword": "#ff79c6",
  "--vz-code-string": "#f1fa8c",
  "--vz-code-number": "#bd93f9",
  "--vz-code-comment": "#9aa5ce",
  "--vz-code-function": "#50fa7b",
  "--vz-code-punctuation": "#f8f8f2",
};

const meta = {
  title: "Components/Surfaces & data/ThemeScope",
  component: ThemeScope,
  tags: ["approved"],
  parameters: { vizier: { id: "D63", replaces: null } },
  args: { vars: VARS, appearance: "dark" },
} satisfies Meta<typeof ThemeScope>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Real components inside the scope draw in its colours, whatever the page around them shows. */
export const Default: Story = {
  render: (args) => (
    <ThemeScope {...args} className="flex w-420 flex-col gap-12 rounded-lg bg-panel p-16">
      <CodeBlock
        language="python"
        code={'# note\nreturn "hits", 5\n'}
        tokens={[
          [{ text: "# note", tone: "comment" }],
          [
            { text: "return", tone: "keyword" },
            { text: " " },
            { text: '"hits"', tone: "string" },
            { text: ",", tone: "punctuation" },
            { text: " " },
            { text: "5", tone: "number" },
          ],
        ]}
      />
      <div>
        <Button variant="primary">Send</Button>
      </div>
    </ThemeScope>
  ),
};
