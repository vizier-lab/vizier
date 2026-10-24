// lint-tokens-exempt: the example palette is hex colours, which the Heightfield palette prop takes.
import { z } from "zod";
import { bool, colour, defineComponent, vec3 } from "./define";

const n = z.number().finite();
const place = {
  position: vec3.optional(),
  rotation: vec3.optional().describe("radians"),
  scale: vec3.optional(),
};
const look = {
  color: colour.optional(),
  flat: bool.optional().describe("flat (low-poly) shading"),
  wireframe: bool.optional(),
  id: z.string().max(100).optional().describe("reported by press"),
};
const light = {
  color: colour.optional(),
  intensity: n.min(0).max(100).optional(),
  position: vec3.optional(),
};

const part = { group: "surface" as const, parent: "Scene3D" as const, children: "none" as const };

export const surface3d = [
  defineComponent({
    name: "Scene3D",
    group: "surface",
    description:
      "A 3D view. Its children are 3D parts only (Box, Sphere, Cylinder, Plane, Group3D, lights, OrbitControls, Heightfield, Mesh). Needs at least a light; add OrbitControls so the person can look around.",
    props: {
      height: n.min(80).max(1200).optional().describe("px, default 360"),
      background: colour.optional(),
      camera: z.object({ position: vec3, fov: n.min(10).max(120).optional(), lookAt: vec3.optional() }).optional(),
      shadows: bool.optional(),
    },
    children: "three_d",
    events: [],
    example: { props: { height: 300, camera: { position: [4, 4, 4], fov: 45 } }, children: true },
  }),
  defineComponent({
    name: "Box",
    ...part,
    description: "A box. size is [width, height, depth].",
    props: { ...place, ...look, size: vec3.optional() },
    events: ["press"],
    example: { props: { position: [0, 0.5, 0], size: [1, 1, 1], color: "accent" } },
  }),
  defineComponent({
    name: "Sphere",
    ...part,
    description: "A ball; radius sets its size.",
    props: { ...place, ...look, radius: n.min(0.01).max(1000).optional(), segments: z.number().int().min(3).max(64).optional() },
    events: ["press"],
    example: { props: { position: [0, 1, 0], radius: 0.5, color: "ok" } },
  }),
  defineComponent({
    name: "Cylinder",
    ...part,
    description: "A cylinder.",
    props: { ...place, ...look, radius: n.min(0.01).max(1000).optional(), height: n.min(0.01).max(1000).optional(), segments: z.number().int().min(3).max(64).optional() },
    events: ["press"],
    example: { props: { position: [0, 0.5, 0], radius: 0.4, height: 1 } },
  }),
  defineComponent({
    name: "Plane",
    ...part,
    description: "A flat rectangle, like a floor. size is [width, depth]; it lies flat by default.",
    props: { ...place, ...look, size: z.tuple([n, n]).optional() },
    events: ["press"],
    example: { props: { size: [10, 10], color: "muted" } },
  }),
  defineComponent({
    name: "Group3D",
    group: "surface",
    parent: "Scene3D",
    description: "Moves, turns or scales its 3D children together.",
    props: place,
    children: "three_d",
    events: [],
    example: { props: { position: [0, 0, 0] }, children: true },
  }),
  defineComponent({
    name: "AmbientLight",
    ...part,
    description: "Light from everywhere, no shadows.",
    props: light,
    events: [],
    example: { props: { intensity: 0.4 } },
  }),
  defineComponent({
    name: "DirectionalLight",
    ...part,
    description: "Sunlight from a position, towards the origin.",
    props: light,
    events: [],
    example: { props: { position: [5, 10, 5], intensity: 1 } },
  }),
  defineComponent({
    name: "PointLight",
    ...part,
    description: "A lamp at a position, lighting all around it.",
    props: light,
    events: [],
    example: { props: { position: [0, 3, 0], intensity: 20 } },
  }),
  defineComponent({
    name: "OrbitControls",
    ...part,
    description: "Lets the person orbit, pan and zoom the view with the mouse or touch.",
    props: { autoRotate: bool.optional(), enableZoom: bool.optional(), target: vec3.optional() },
    events: [],
    example: { props: { autoRotate: false } },
  }),
  defineComponent({
    name: "Heightfield",
    ...part,
    description:
      "A terrain from a grid of heights. heights is resolution×resolution numbers in row-major order, usually from a logic function in state. flat (default) gives the low-poly look; palette colours by height.",
    props: {
      size: z.tuple([n.min(0.1), n.min(0.1)]).describe("[width, depth] in scene units"),
      resolution: z.number().int().min(2).max(192).describe("points per side, at most 192"),
      heights: z.array(n).max(192 * 192).describe("resolution² numbers, row-major, 0 to 1"),
      heightScale: n.optional().describe("scene units for a height of 1, default 1"),
      palette: z.array(z.object({ at: n.min(0).max(1), color: z.string().regex(/^#[0-9a-fA-F]{6}$/) })).min(1).max(12).optional().describe("colour stops by height, {at: 0–1, color: #rrggbb}"),
      flat: bool.optional().describe("flat shading, default true"),
      wireframe: bool.optional(),
      position: vec3.optional(),
    },
    events: ["press"],
    example: {
      props: {
        size: [10, 10],
        resolution: 3,
        heights: [0, 0.2, 0, 0.2, 1, 0.2, 0, 0.2, 0],
        heightScale: 2,
        palette: [{ at: 0, color: "#2b5d7a" }, { at: 1, color: "#f2f4f7" }],
      },
    },
  }),
  defineComponent({
    name: "Mesh",
    ...part,
    description: "A custom triangle mesh. vertices is x,y,z triples; indices is vertex-index triples (omit for consecutive vertices); colors is r,g,b (0–1) per vertex.",
    props: {
      ...place,
      vertices: z.array(n).min(9).max(60_000),
      indices: z.array(z.number().int().min(0)).max(120_000).optional(),
      colors: z.array(n.min(0).max(1)).max(60_000).optional(),
      flat: bool.optional(),
      id: z.string().max(100).optional(),
    },
    events: ["press"],
    example: { props: { vertices: [0, 0, 0, 1, 0, 0, 0, 1, 0], indices: [0, 1, 2] } },
  }),
];
