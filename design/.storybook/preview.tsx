import type { Decorator, Preview } from "@storybook/react-vite";
import { DecoratorHelpers } from "@storybook/addon-themes";
import { Controls, Description, Primary, Stories, Subtitle, Title } from "@storybook/addon-docs/blocks";
import { useEffect } from "react";
import { addons } from "storybook/preview-api";
import { GLOBALS_UPDATED, SET_GLOBALS } from "storybook/internal/core-events";
import { SourceBlock } from "./blocks/Source";
import { MotionProvider } from "../src/lib/Motion";
import "../src/styles.css";
import "./docs.css";

const { initializeThemeState, pluckThemeFromContext } = DecoratorHelpers;

// Dark and Light set data-theme on <html>; System removes it so the OS decides (research R5).
const THEMES = { dark: "dark", light: "light", system: "" } as const;
initializeThemeState(Object.keys(THEMES), "system");

function applyTheme(theme: string | undefined) {
  const value = THEMES[(theme || "system") as keyof typeof THEMES] ?? "";
  const html = document.documentElement;
  if (value) html.setAttribute("data-theme", value);
  else html.removeAttribute("data-theme");
}

// MDX pages render no story, so no decorator runs there: follow the toolbar from its events too.
const channel = addons.getChannel();
const onGlobals = ({ globals }: { globals: { theme?: string } }) => applyTheme(globals.theme);
channel.on(SET_GLOBALS, onGlobals);
channel.on(GLOBALS_UPDATED, onGlobals);

const withTheme: Decorator = (Story, context) => {
  const theme = pluckThemeFromContext(context);
  useEffect(() => applyTheme(theme), [theme]);
  return <Story />;
};

// Motion behaves as it does in the app: reduced-motion preference honoured.
const withMotion: Decorator = (Story) => (
  <MotionProvider>
    <Story />
  </MotionProvider>
);

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

// SC-007: a critical axe violation fails `npm test`. Lesser findings (contrast
// on text/faint, for one) stay warnings in the Accessibility panel, where the
// a11y addon reports everything. Only runs under Vitest.
const failOnCriticalA11y: Preview["afterEach"] = async ({ title, name, parameters, viewMode }) => {
  if (import.meta.env.VITEST_STORYBOOK !== "false" || viewMode !== "story" || parameters.a11y?.test === "off") return;
  const { default: axe } = await import("axe-core");
  const result = await axe.run(
    { include: document.body, exclude: [".sb-wrapper", "#storybook-docs", "#storybook-highlights-root"] },
    { resultTypes: ["violations"] },
  );
  const critical = result.violations.filter((v) => v.impact === "critical");
  if (critical.length) {
    const lines = critical.map((v) => `${v.id}: ${v.help} (${v.nodes.map((n) => n.target.join(" ")).join(", ")})`);
    throw new Error(`${title} › ${name}: critical accessibility violations\n${lines.join("\n")}`);
  }
};

const preview: Preview = {
  decorators: [withTheme, withMotion, requireInventoryId],
  afterEach: failOnCriticalA11y,
  initialGlobals: { theme: "system" },
  parameters: {
    a11y: { test: "todo" },
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
