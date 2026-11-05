// lint-tokens-exempt: a story has to supply a palette that isn't the page's own.
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemeTile, type ThemeTileColors } from "./ThemeTile";

const DRACULA: ThemeTileColors = {
  frame: "#21222c",
  content: "#282a36",
  line: "#44475a",
  code: ["#ff79c6", "#f1fa8c", "#50fa7b", "#bd93f9"],
};

const LATTE: ThemeTileColors = {
  frame: "#e6e9ef",
  content: "#eff1f5",
  line: "#ccd0da",
  code: ["#8839ef", "#40a02b", "#1e66f5", "#fe640b"],
};

const meta = {
  title: "Components/Actions & inputs/ThemeTile",
  component: ThemeTile,
  tags: ["approved"],
  parameters: { vizier: { id: "D64", replaces: null } },
  args: { name: "Dracula", detail: "Dark only", colors: DRACULA },
  decorators: [(Story) => <div className="w-128"><Story /></div>],
} satisfies Meta<typeof ThemeTile>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Selected: Story = { args: { selected: true } };

/** A long name truncates, and tiles keep to the grid they are given. */
export const Grid: Story = {
  decorators: [(Story) => <div className="w-420"><Story /></div>],
  render: () => (
    <div className="grid gap-8" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(112px, 1fr))" }}>
      <ThemeTile name="Dracula" detail="Dark only" colors={DRACULA} selected />
      <ThemeTile name="Catppuccin" detail="Light and dark" colors={LATTE} />
      <ThemeTile name="A very long custom theme name indeed" detail="Custom" colors={LATTE} />
    </div>
  ),
};
