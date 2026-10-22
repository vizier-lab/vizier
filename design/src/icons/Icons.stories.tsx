import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { toneModes, type ToneMode } from "../tokens/tokens";
import { Icon } from "./Icon";
import { iconNames, lucideNames } from "./map";

type Args = { tone: ToneMode | "inherit"; size: number };

const meta = {
  title: "Icons/All",
  parameters: {
    docs: {
      description: {
        component:
          "The icon vocabulary. Each design name maps to one Lucide icon in `src/icons/map.ts`, drawn at 17px with a 1.7px stroke at every size. Use `<Icon name tone size>`.",
      },
    },
  },
  args: { tone: "inherit", size: 17 },
  argTypes: {
    tone: { control: "select", options: ["inherit", ...toneModes] },
    size: { control: { type: "range", min: 12, max: 48, step: 1 } },
  },
} satisfies Meta<Args>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Searchable grid of every icon, with its design and Lucide names. */
export const All: Story = {
  render: function Render({ tone, size }) {
    const [query, setQuery] = useState("");
    const q = query.trim().toLowerCase();
    const shown = iconNames.filter((n) => !q || n.includes(q) || lucideNames[n].includes(q));
    return (
      <div className="flex flex-col gap-16">
        <label className="flex items-center gap-8 type-ui text-muted">
          <Icon name="search" />
          <span className="sr-only">Filter icons</span>
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={`Filter ${iconNames.length} icons`}
            className="h-32 w-240 rounded-md border border-border-strong bg-raised px-10 text-fg type-ui"
          />
        </label>
        <ul className="grid grid-cols-[repeat(auto-fill,minmax(132px,1fr))] gap-8">
          {shown.map((name) => (
            <li
              key={name}
              className="flex flex-col items-center gap-8 rounded-md border border-border px-8 py-12 text-fg"
            >
              <Icon name={name} tone={tone === "inherit" ? undefined : tone} size={size} />
              <span className="type-mono-sm text-fg">{name}</span>
              <span className="type-mono-xs text-muted">{lucideNames[name]}</span>
            </li>
          ))}
        </ul>
      </div>
    );
  },
};

/** Every icon in every tone. Also guards against a Lucide rename: a missing glyph fails the render. */
export const Tones: Story = {
  render: ({ size }) => (
    <table className="border-separate border-spacing-y-4 text-fg">
      <thead>
        <tr>
          <th className="pr-16 text-left type-label uppercase text-muted">icon</th>
          {toneModes.map((t) => (
            <th key={t} className="px-8 type-label uppercase text-muted">
              {t}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {iconNames.map((name) => (
          <tr key={name}>
            <td className="pr-16 type-mono-sm">{name}</td>
            {toneModes.map((t) => (
              <td key={t} className={t === "inverse" ? "bg-ink px-8 text-center" : "px-8 text-center"}>
                <Icon name={name} tone={t} size={size} title={`${name}, ${t}`} />
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  ),
};
