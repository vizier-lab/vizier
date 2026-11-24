import { z } from "zod";
import { bool, defineComponent } from "./define";

export const control = [
  defineComponent({
    name: "Effect",
    group: "control",
    description:
      "Draws nothing; runs actions when the interface appears (on.load, once) and, with intervalMs, repeatedly (on.tick). Use load to compute a first value, tick for a simulation or animation.",
    props: {
      intervalMs: z.number().min(50).max(3_600_000).optional().describe("ms between ticks, at least 50; omit for no ticking"),
      running: bool.optional().describe("false pauses ticking; default true"),
    },
    children: "none",
    events: ["load", "tick"],
    example: { props: { intervalMs: 500 } },
  }),
];
