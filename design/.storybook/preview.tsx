import type { Decorator, Preview } from "@storybook/react-vite";
import { DecoratorHelpers } from "@storybook/addon-themes";
import { Controls, Description, Primary, Stories, Subtitle, Title } from "@storybook/addon-docs/blocks";
import { useEffect } from "react";
import { SourceBlock } from "./blocks/Source";
import "../src/styles.css";

const { initializeThemeState, pluckThemeFromContext } = DecoratorHelpers;

// Dark and Light set data-theme on <html>; System removes it so the OS decides (research R5).
const THEMES = { dark: "dark", light: "light", system: "" } as const;
initializeThemeState(Object.keys(THEMES), "system");

const withTheme: Decorator = (Story, context) => {
  const theme = (pluckThemeFromContext(context) || "system") as keyof typeof THEMES;
  const value = THEMES[theme] ?? "";
  useEffect(() => {
    const html = document.documentElement;
    if (value) html.setAttribute("data-theme", value);
    else html.removeAttribute("data-theme");
  }, [value]);
  return <Story />;
};

// Every catalogue entry outside Foundations and Icons names its inventory id
// (contracts/story-metadata.md); a story without one fails `npm test`.
const requireInventoryId: Decorator = (Story, context) => {
  const title = context.title;
  const exempt = title.startsWith("Foundations/") || title.startsWith("Icons/");
  if (!exempt && !context.parameters.vizier?.id) {
    throw new Error(`${title}: parameters.vizier.id is missing`);
  }
  return <Story />;
};

const preview: Preview = {
  decorators: [withTheme, requireInventoryId],
  initialGlobals: { theme: "system" },
  parameters: {
    a11y: { test: "error" },
    layout: "padded",
    controls: { expanded: true },
    docs: {
      page: () => (
        <>
          <Title />
          <Subtitle />
          <SourceBlock />
          <Description />
          <Primary />
          <Controls />
          <Stories />
        </>
      ),
    },
    options: {
      storySort: {
        order: ["Welcome", "Foundations", ["Colour", "Type", "Size & spacing", "Elevation", "Tone"], "Icons", "Brand", "Components"],
      },
    },
  },
  tags: ["autodocs"],
};

export default preview;
