import type { Meta, StoryObj } from "@storybook/react-vite";
import { colorTokens } from "../tokens/tokens";
import { ColourTable, readVars } from "./blocks";

const meta = {
  title: "Foundations/Colour",
  component: ColourTable,
  parameters: { layout: "padded" },
  tags: ["!autodocs"],
} satisfies Meta<typeof ColourTable>;

export default meta;

/** Every colour token, both themes side by side. Fails if any token is empty in either theme. */
export const Tokens: StoryObj<typeof meta> = {
  play: async () => {
    for (const theme of ["dark", "light"] as const) {
      const values = readVars(colorTokens.map((t) => t.cssVar), theme);
      const empty = colorTokens.filter((t) => !values[t.cssVar]).map((t) => t.cssVar);
      if (empty.length) throw new Error(`Empty in ${theme}: ${empty.join(", ")}`);
    }
  },
};
