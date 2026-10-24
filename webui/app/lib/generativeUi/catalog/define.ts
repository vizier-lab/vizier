import { z } from "zod";

/** The groups of the catalogue, in the order the agent's index lists them. */
export const GROUPS = ["layout", "text", "data", "media", "input", "action", "surface", "control"] as const;
export type Group = (typeof GROUPS)[number];

/** What a component may hold: nothing, anything 2D, or only 3D parts (inside `Scene3D`). */
export type ChildRule = "none" | "any" | "three_d";

export type ComponentDef = {
  name: string;
  group: Group;
  /** One line for the agent's index; the first sentence is the summary. */
  description: string;
  /** Literal values only: an expression in place of a value is accepted without a type check. */
  props: z.ZodObject<z.ZodRawShape>;
  children: ChildRule;
  /** Events the component emits, for `on`. */
  events: readonly string[];
  /** A minimal valid element: its props, and whether it is shown with a child. */
  example: { props: Record<string, unknown>; children?: boolean };
};

/** One catalogue entry. Adding a component is one of these plus one registry entry. */
export function defineComponent<S extends z.ZodRawShape>(def: Omit<ComponentDef, "props"> & { props: S }): ComponentDef {
  return { ...def, props: z.object(def.props) };
}

// Shared prop shapes. Closed enums only: there is no free-form style, class or HTML.

export const gap = z.enum(["none", "sm", "md", "lg"]).describe("space between children");
export const align = z.enum(["start", "center", "end", "stretch"]);
export const tone = z.enum(["default", "accent", "ok", "warn", "error"]);
export const bool = z.boolean();
export const text = z.string().max(20_000);
export const label = z.string().max(500);

/** A validation rule for an input; `type` names a json-render check. */
export const checks = z
  .array(
    z.object({
      type: z.enum(["required", "email", "minLength", "maxLength", "pattern", "min", "max", "url", "numeric"]),
      message: z.string().max(300),
      args: z.record(z.string(), z.unknown()).optional(),
    }),
  )
  .describe("rules checked by validateForm and shown under the field");

/** A coordinate triple for 3D placement. */
export const vec3 = z.tuple([z.number(), z.number(), z.number()]);

/** A colour from the closed token list, or #rrggbb. */
export const colour = z
  .union([z.enum(["ink", "muted", "accent", "ok", "warn", "error", "surface", "series-1", "series-2", "series-3", "series-4", "series-5", "series-6"]), z.string().regex(/^#[0-9a-fA-F]{6}$/)])
  .describe("a colour token (ink, muted, accent, ok, warn, error, surface, series-1…6) or #rrggbb");

/** The props every input takes. */
export const inputBase = {
  label: label.describe("the field's label (required)"),
  disabled: bool.optional(),
  checks: checks.optional(),
};
