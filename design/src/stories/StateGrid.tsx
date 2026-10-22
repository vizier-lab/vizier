/**
 * Story helper (not exported from the package): shows a component's states
 * side by side. Interaction states (hover, pressed, focus) are forced by
 * storybook-addon-pseudo-states on the first element in their cell, so the
 * component itself needs no test-only props.
 */
import type { ReactNode } from "react";

export type StateCell = {
  label: string;
  /** Forces a pseudo state on everything in the cell. */
  pseudo?: "hover" | "active" | "focus-visible" | "focus-within";
  node: ReactNode;
};

/** The `parameters.pseudo` that goes with a StateGrid. */
export const statePseudo = {
  pseudo: {
    hover: '[data-pseudo="hover"] *',
    active: '[data-pseudo="active"] *',
    focusVisible: '[data-pseudo="focus-visible"] :is(a, button, input, textarea, select, [tabindex])',
    focusWithin: '[data-pseudo="focus-within"] *',
  },
};

export function StateGrid({ rows }: { rows: { label?: string; cells: StateCell[] }[] }) {
  const cols = Math.max(...rows.map((r) => r.cells.length));
  const hasRowLabels = rows.some((r) => r.label);
  return (
    <div
      className="grid items-end gap-x-24 gap-y-20 text-fg"
      style={{ gridTemplateColumns: `${hasRowLabels ? "max-content " : ""}repeat(${cols}, max-content)` }}
    >
      {rows.map((row, i) => (
        <Row key={row.label ?? i} label={row.label} hasRowLabels={hasRowLabels} cells={row.cells} />
      ))}
    </div>
  );
}

function Row({ label, hasRowLabels, cells }: { label?: string; hasRowLabels: boolean; cells: StateCell[] }) {
  return (
    <>
      {hasRowLabels && <span className="self-center type-mono-sm text-muted">{label}</span>}
      {cells.map((c) => (
        <div key={c.label} className="flex flex-col gap-8">
          <span className="type-label uppercase text-muted">{c.label}</span>
          <div data-pseudo={c.pseudo} className="flex">
            {c.node}
          </div>
        </div>
      ))}
    </>
  );
}

/** The usual interaction columns. */
export function interactionCells(render: (label: string) => ReactNode, extra: StateCell[] = []): StateCell[] {
  return [
    { label: "Default", node: render("Default") },
    { label: "Hover", pseudo: "hover", node: render("Hover") },
    { label: "Pressed", pseudo: "active", node: render("Pressed") },
    { label: "Focus", pseudo: "focus-visible", node: render("Focus") },
    ...extra,
  ];
}
