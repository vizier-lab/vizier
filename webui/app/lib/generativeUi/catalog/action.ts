import { z } from "zod";
import { bool, defineComponent, label, tone } from "./define";

export const action = [
  defineComponent({
    name: "Button",
    group: "action",
    description: "A button. Wire on.press to an action; selected highlights it (a chosen board square).",
    props: {
      label,
      variant: z.enum(["primary", "secondary", "ghost", "danger"]).optional(),
      tone: tone.optional(),
      disabled: bool.optional(),
      selected: bool.optional(),
      size: z.enum(["sm", "md", "lg"]).optional(),
      square: bool.optional().describe("a centred square that fills its grid cell, for boards and keypads"),
    },
    children: "none",
    events: ["press"],
    example: { props: { label: "Save", variant: "primary" } },
  }),
];
