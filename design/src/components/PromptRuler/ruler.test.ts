import { describe, expect, it } from "vitest";
import { STEP, indexAt, marksAround, wheelSteps } from "./ruler";

describe("marksAround", () => {
  it("shows every prompt of a short conversation, centred, at one strength", () => {
    const { marks, windowed } = marksAround(9, 7, 7, 10);
    expect(windowed).toBe(false);
    expect(marks.map((m) => m.index)).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8]);
    expect(marks.map((m) => m.offset)).toEqual([-4, -3, -2, -1, 0, 1, 2, 3, 4]);
    expect(marks.filter((m) => m.kind === "current").map((m) => m.index)).toEqual([7]);
    expect(new Set(marks.filter((m) => m.kind !== "current").map((m) => m.opacity))).toEqual(new Set([0.5]));
    expect(marks.some((m) => m.number !== undefined)).toBe(false);
  });

  it("shows a window of a long conversation, fading and numbering every tenth", () => {
    const { marks, windowed } = marksAround(1240, 411, 411, 10);
    expect(windowed).toBe(true);
    expect(marks).toHaveLength(21);
    expect(marks[0].index).toBe(401);
    expect(marks.find((m) => m.index === 411)).toMatchObject({ offset: 0, kind: "current" });
    expect(marks.find((m) => m.index === 409)?.number).toBe(410);
    expect(marks[0].opacity).toBeLessThan(marks[9].opacity);
  });

  it("stays full at either end, the current mark moving towards it", () => {
    const top = marksAround(1240, 0, 0, 10);
    expect(top.marks).toHaveLength(21);
    expect(top.marks[0]).toMatchObject({ index: 0, kind: "current", offset: -10 });
    // Nothing above the first prompt, so that edge does not fade; the far one does.
    expect(top.marks[1].opacity).toBe(0.7);
    expect(top.marks[20].opacity).toBeLessThan(0.7);

    const bottom = marksAround(60, 59, 59, 10);
    expect(bottom.marks).toHaveLength(21);
    expect(bottom.marks.at(-1)).toMatchObject({ index: 59, kind: "current", offset: 10 });
  });

  it("keeps the prompt being read long while the ruler is browsed elsewhere", () => {
    const { marks } = marksAround(1240, 300, 411, 10);
    expect(marks.some((m) => m.kind === "current")).toBe(false);
    expect(marksAround(1240, 405, 411, 10).marks.find((m) => m.index === 411)?.kind).toBe("current");
  });

  it("is empty with no prompts", () => {
    expect(marksAround(0, 0, 0, 10).marks).toEqual([]);
  });
});

describe("indexAt", () => {
  it("finds the mark under a point", () => {
    const { marks } = marksAround(60, 30, 30, 10);
    expect(indexAt(marks, 0)).toBe(30);
    expect(indexAt(marks, STEP * 2 + 3)).toBe(32);
    expect(indexAt(marks, -STEP * 3)).toBe(27);
    expect(indexAt(marks, STEP * 40)).toBeNull();
  });
});

describe("wheelSteps", () => {
  it("counts a mouse notch as one prompt", () => {
    expect(wheelSteps(0, 3, 1).steps).toBe(1);
    expect(wheelSteps(0, -100, 0).steps).toBe(-2);
  });

  it("sums a trackpad's small deltas before stepping", () => {
    let carry = 0;
    let steps = 0;
    for (let i = 0; i < 10; i += 1) {
      const r = wheelSteps(carry, 4, 0);
      carry = r.carry;
      steps += r.steps;
    }
    expect(steps).toBe(0);
    expect(wheelSteps(carry, 12, 0).steps).toBe(1);
  });
});
