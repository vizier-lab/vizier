import { z } from "zod";
import { bool, defineComponent, label, tone } from "./define";

const format = z.enum(["number", "percent", "currency", "bytes", "duration"]);

export const data = [
  defineComponent({
    name: "Stat",
    group: "data",
    description: "One headline number with a label and an optional change.",
    props: {
      label,
      value: z.union([z.string().max(100), z.number()]),
      delta: z.union([z.string().max(100), z.number()]).optional().describe("change since before, e.g. +4.2%"),
      tone: tone.optional(),
      format: format.optional().describe("how a numeric value is shown"),
    },
    children: "none",
    events: [],
    example: { props: { label: "Replies", value: 1284, delta: "+4.2%", tone: "ok" } },
  }),
  defineComponent({
    name: "Table",
    group: "data",
    description: "Rows and columns. rows may come from state; selectable rows emit select.",
    props: {
      columns: z
        .array(z.object({ key: z.string().max(100), label, align: z.enum(["start", "center", "end"]).optional(), format: format.optional() }))
        .min(1)
        .max(40)
        .describe("{key,label,align?,format?}"),
      rows: z.array(z.record(z.string(), z.unknown())).max(5000).describe("an array of objects keyed by column key"),
      selectable: bool.optional(),
      maxRows: z.number().int().min(1).max(5000).optional(),
    },
    children: "none",
    events: ["select"],
    example: { props: { columns: [{ key: "name", label: "Name" }, { key: "score", label: "Score", align: "end" }], rows: [{ name: "Ada", score: 91 }, { name: "Lin", score: 84 }] } },
  }),
  defineComponent({
    name: "KeyValue",
    group: "data",
    description: "A list of label and value pairs.",
    props: { items: z.array(z.object({ label, value: z.union([z.string().max(2000), z.number(), z.boolean()]) })).max(200) },
    children: "none",
    events: [],
    example: { props: { items: [{ label: "Region", value: "eu-west" }, { label: "Plan", value: "Team" }] } },
  }),
  defineComponent({
    name: "Progress",
    group: "data",
    description: "A progress bar, value from 0 to 1.",
    props: { value: z.number().min(0).max(1), label: label.optional(), tone: tone.optional() },
    children: "none",
    events: [],
    example: { props: { value: 0.62, label: "Upload" } },
  }),
  defineComponent({
    name: "Chart",
    group: "data",
    description: "A bar, line, area or pie chart of labelled values. Give series to compare several.",
    props: {
      kind: z.enum(["bar", "line", "area", "pie"]),
      data: z.array(z.object({ label: z.string().max(200), value: z.number(), series: z.string().max(100).optional() })).max(2000),
      legend: bool.optional(),
      height: z.number().min(80).max(1000).optional(),
    },
    children: "none",
    events: ["press"],
    example: { props: { kind: "bar", data: [{ label: "Mon", value: 12 }, { label: "Tue", value: 19 }, { label: "Wed", value: 7 }] } },
  }),
];
