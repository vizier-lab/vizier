import { z } from "zod";
import { bool, defineComponent, inputBase } from "./define";

const options = z
  .array(z.union([z.string().max(200), z.object({ value: z.string().max(200), label: z.string().max(200) })]))
  .min(1)
  .max(500)
  .describe("strings, or {value,label}");

export const input = [
  defineComponent({
    name: "TextInput",
    group: "input",
    description: "A text field. Bind value with $bindState.",
    props: {
      ...inputBase,
      value: z.string().optional(),
      placeholder: z.string().max(300).optional(),
      multiline: bool.optional(),
      maxLength: z.number().int().min(1).max(100_000).optional(),
    },
    children: "none",
    events: ["change", "submit"],
    example: { props: { label: "Name", value: { $bindState: "/name" }, placeholder: "Your name" } },
  }),
  defineComponent({
    name: "NumberInput",
    group: "input",
    description: "A number field. Bind value with $bindState.",
    props: { ...inputBase, value: z.number().optional(), min: z.number().optional(), max: z.number().optional(), step: z.number().optional() },
    children: "none",
    events: ["change"],
    example: { props: { label: "Quantity", value: { $bindState: "/qty" }, min: 0, max: 99 } },
  }),
  defineComponent({
    name: "Slider",
    group: "input",
    description: "A slider between min and max. Bind value with $bindState.",
    props: { ...inputBase, value: z.number().optional(), min: z.number(), max: z.number(), step: z.number().optional() },
    children: "none",
    events: ["change"],
    example: { props: { label: "Roughness", value: { $bindState: "/rough" }, min: 0, max: 1, step: 0.05 } },
  }),
  defineComponent({
    name: "Select",
    group: "input",
    description: "A dropdown for a short list. Bind value with $bindState.",
    props: { ...inputBase, value: z.string().optional(), options },
    children: "none",
    events: ["change"],
    example: { props: { label: "Size", value: { $bindState: "/size" }, options: ["S", "M", "L"] } },
  }),
  defineComponent({
    name: "Checkbox",
    group: "input",
    description: "A tick box. Bind value (true/false) with $bindState.",
    props: { ...inputBase, value: bool.optional() },
    children: "none",
    events: ["change"],
    example: { props: { label: "Subscribe", value: { $bindState: "/sub" } } },
  }),
  defineComponent({
    name: "Switch",
    group: "input",
    description: "An on/off switch. Bind value (true/false) with $bindState.",
    props: { ...inputBase, value: bool.optional() },
    children: "none",
    events: ["change"],
    example: { props: { label: "Enabled", value: { $bindState: "/on" } } },
  }),
  defineComponent({
    name: "RadioGroup",
    group: "input",
    description: "One choice from a few. Bind value with $bindState.",
    props: { ...inputBase, value: z.string().optional(), options },
    children: "none",
    events: ["change"],
    example: { props: { label: "Plan", value: { $bindState: "/plan" }, options: ["Free", "Team"] } },
  }),
];
