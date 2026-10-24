import type { Meta, StoryObj } from "@storybook/react-vite";
import { Sigil } from "./Sigil";
import { sigilCells, SIGIL_MAX_LIT, SIGIL_MIN_LIT } from "./sigilCells";

const AGENTS = [
  "Archivist",
  "Scout",
  "Herald",
  "Treasurer",
  "Scribe",
  "Chamberlain",
  "Steward",
  "Cartographer",
  "Envoy",
  "Sentinel",
  "Alchemist",
  "Warden",
];

const meta = {
  title: "Brand/Sigil",
  component: Sigil,
  tags: ["approved"],
  parameters: {
    vizier: { id: "B3", replaces: "webui/app/components/avatar.tsx" },
    docs: {
      description: {
        component: `An agent's seal: a mirror-symmetric 5×5 dot pattern drawn from its id, so it looks the same everywhere.

**Rule** (\`sigilCells\` in \`sigil.ts\`): hash the id with 32-bit FNV-1a; read 15 bits into the left three columns row by row; mirror them onto the right two. If fewer than ${SIGIL_MIN_LIT} or more than ${SIGIL_MAX_LIT} dots are lit, rehash with "#" appended. While the agent is live, the lit dot nearest the top right turns emerald, as on the mark.`,
      },
    },
  },
  args: { agentId: "Archivist", size: "md", live: false, label: "Archivist" },
  argTypes: { size: { control: "inline-radio", options: ["sm", "md", "lg", "xl"] } },
} satisfies Meta<typeof Sigil>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Live: Story = { args: { live: true } };

/** Sm 22 in chat, Md 34 in the switcher and cards, Lg 64 on the new-agent preview, Xl 86 on the config identity. */
export const Sizes: Story = {
  render: (args) => (
    <div className="flex items-end gap-16">
      {(["sm", "md", "lg", "xl"] as const).map((size) => (
        <Sigil key={size} {...args} size={size} />
      ))}
    </div>
  ),
};

/** Twelve agents. Each id always gives the same pattern; the check below fails the story if it doesn't. */
export const Agents: Story = {
  render: (args) => (
    <ul className="grid grid-cols-6 gap-16">
      {AGENTS.map((id) => (
        <li key={id} className="flex flex-col items-center gap-8">
          <Sigil {...args} agentId={id} label={id} size="lg" />
          <span className="type-small text-muted">{id}</span>
        </li>
      ))}
    </ul>
  ),
  play: async () => {
    for (const id of AGENTS) {
      const a = sigilCells(id).join();
      const b = sigilCells(id).join();
      if (a !== b) throw new Error(`${id}: sigil is not stable`);
      const cells = sigilCells(id);
      const lit = cells.filter(Boolean).length;
      if (lit < SIGIL_MIN_LIT || lit > SIGIL_MAX_LIT) throw new Error(`${id}: ${lit} lit cells`);
      for (let r = 0; r < 5; r++) {
        if (cells[r * 5] !== cells[r * 5 + 4] || cells[r * 5 + 1] !== cells[r * 5 + 3]) {
          throw new Error(`${id}: row ${r} is not mirrored`);
        }
      }
    }
  },
};
