import { z } from "zod";
import { bool, defineComponent, label, text, tone } from "./define";

export const textGroup = [
  defineComponent({
    name: "Heading",
    group: "text",
    description: "A heading, level 1 (largest) to 4.",
    props: { text: label, level: z.number().int().min(1).max(4).optional() },
    children: "none",
    events: [],
    example: { props: { text: "Weekly report", level: 2 } },
  }),
  defineComponent({
    name: "Text",
    group: "text",
    description: "A paragraph. Supports a markdown subset: bold, italic, code, lists and links (https, http, mailto only).",
    props: {
      text,
      tone: z.enum(["normal", "muted", "ok", "warn", "error"]).optional(),
      size: z.enum(["sm", "md", "lg"]).optional(),
    },
    children: "none",
    events: [],
    example: { props: { text: "All **12** checks passed." } },
  }),
  defineComponent({
    name: "Badge",
    group: "text",
    description: "A small tag for a status or keyword.",
    props: { text: label, tone: tone.optional() },
    children: "none",
    events: [],
    example: { props: { text: "live", tone: "ok" } },
  }),
  defineComponent({
    name: "Callout",
    group: "text",
    description: "A highlighted note with an optional title.",
    props: { text, title: label.optional(), tone: tone.optional() },
    children: "none",
    events: [],
    example: { props: { title: "Heads up", text: "This changes on the 1st.", tone: "warn" } },
  }),
  defineComponent({
    name: "Code",
    group: "text",
    description: "A block of code or preformatted text with a copy button.",
    props: { text, lang: z.string().max(40).optional(), wrap: bool.optional() },
    children: "none",
    events: [],
    example: { props: { text: "print('hi')", lang: "python" } },
  }),
];
