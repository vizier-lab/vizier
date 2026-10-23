import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { Button, type ButtonVariant } from "./Button";

const VARIANTS: ButtonVariant[] = ["primary", "secondary", "danger", "destructive", "ghost", "dashed"];
const LABELS: Record<ButtonVariant, { label: string; busy: string; icon: "plus" | "history" | "trash" }> = {
  primary: { label: "New agent", busy: "Saving…", icon: "plus" },
  secondary: { label: "History", busy: "Testing…", icon: "history" },
  danger: { label: "Delete", busy: "Deleting…", icon: "trash" },
  destructive: { label: "Delete Scribe", busy: "Deleting…", icon: "trash" },
  ghost: { label: "Cancel", busy: "Loading…", icon: "plus" },
  dashed: { label: "Show all 48 agents", busy: "Loading…", icon: "plus" },
};

const meta = {
  title: "Components/Actions & inputs/Button",
  component: Button,
  tags: ["needs-review"],
  parameters: {
    vizier: { id: "D10", replaces: null },
    docs: {
      description: {
        component:
          "Primary for the one main action on a surface; Secondary for the rest; Danger for a destructive action that still asks; Destructive for the confirming click itself; Ghost for low-weight actions (Cancel); Dashed for “show more” rows. While loading, the dot loader replaces the icon and the label says what is happening.",
      },
    },
  },
  args: { variant: "primary", size: "md", icon: "plus", children: "New agent" },
  argTypes: {
    variant: { control: "select", options: VARIANTS },
    size: { control: "inline-radio", options: ["sm", "md", "lg"] },
  },
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Variant × size. */
export const Variants: Story = {
  render: () => (
    <div className="flex flex-col gap-16">
      {VARIANTS.map((v) => (
        <div key={v} className="flex items-center gap-12">
          <Button variant={v} icon={LABELS[v].icon}>
            {LABELS[v].label}
          </Button>
          <Button variant={v} size="sm" icon={LABELS[v].icon}>
            {LABELS[v].label}
          </Button>
          <Button variant={v}>{LABELS[v].label}</Button>
        </div>
      ))}
    </div>
  ),
};

/** Every variant in every state, Md then Sm. */
export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  render: () => (
    <div className="flex flex-col gap-32">
      {(["md", "sm"] as const).map((size) => (
        <StateGrid
          key={size}
          rows={VARIANTS.map((v) => ({
            label: `${v} · ${size}`,
            cells: interactionCells(
              () => (
                <Button variant={v} size={size} icon={LABELS[v].icon}>
                  {LABELS[v].label}
                </Button>
              ),
              [
                {
                  label: "Disabled",
                  node: (
                    <Button variant={v} size={size} icon={LABELS[v].icon} disabled>
                      {LABELS[v].label}
                    </Button>
                  ),
                },
                {
                  label: "Loading",
                  node: (
                    <Button variant={v} size={size} loading>
                      {LABELS[v].busy}
                    </Button>
                  ),
                },
              ],
            ),
          }))}
        />
      ))}
    </div>
  ),
};

/** Sign in and Create account: Lg, full width. */
export const LargeBlock: Story = {
  render: () => (
    <div className="flex w-372 flex-col gap-12">
      <Button variant="primary" size="lg" block type="submit">
        Sign in
      </Button>
    </div>
  ),
};

/** Extensions drawn on screens: a key hint, a trailing icon, the dashed "show all" row. */
export const Extensions: Story = {
  render: () => (
    <div className="flex w-640 flex-col items-start gap-16">
      <Button variant="primary" shortcut="Ctrl S">
        Save
      </Button>
      <Button variant="primary" trailingIcon="chevron-right">
        Continue to review
      </Button>
      <Button variant="dashed" block>
        Show all 48 agents <span className="type-small text-faint">sorted by last active</span>
      </Button>
    </div>
  ),
};
