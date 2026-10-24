import { z } from "zod";
import { colour, defineComponent } from "./define";

const n = z.number().finite();

const shape = z.object({
  id: z.string().max(100).optional().describe("reported by press"),
  kind: z.enum(["rect", "circle", "line", "path", "text", "image"]),
  x: n.optional(),
  y: n.optional(),
  w: n.optional(),
  h: n.optional(),
  r: n.optional(),
  x1: n.optional(),
  y1: n.optional(),
  x2: n.optional(),
  y2: n.optional(),
  d: z.string().max(10_000).optional().describe("path data, like M0 0 L10 10"),
  fill: colour.optional(),
  stroke: colour.optional(),
  strokeWidth: n.optional(),
  text: z.string().max(500).optional(),
  file: z.string().max(255).optional().describe("image: a session file name, drawn in x,y,w,h"),
  size: n.optional().describe("text size"),
});

export const surface = [
  defineComponent({
    name: "Canvas2D",
    group: "surface",
    description:
      "A 2D drawing of shapes (rect, circle, line, path, text) in logical width×height units, scaled to fit. Shapes may come from state; press reports {x, y, id}, so a board or a map is clickable.",
    props: {
      width: z.number().min(10).max(4000),
      height: z.number().min(10).max(4000),
      background: colour.optional(),
      shapes: z.array(shape).max(5000).describe("rect {x,y,w,h}, circle {x,y,r}, line {x1,y1,x2,y2}, path {d}, text {x,y,text,size}, image {file,x,y,w,h}"),
    },
    children: "none",
    events: ["press", "move"],
    example: {
      props: {
        width: 200,
        height: 100,
        background: "surface",
        shapes: [
          { id: "box", kind: "rect", x: 10, y: 10, w: 80, h: 40, fill: "accent" },
          { kind: "circle", x: 150, y: 50, r: 20, fill: "ok" },
        ],
      },
    },
  }),
];
