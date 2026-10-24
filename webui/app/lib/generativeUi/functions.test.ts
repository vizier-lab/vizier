import { describe, expect, it } from "vitest";
import { COMPUTED_FUNCTIONS as f, FUNCTION_NAMES } from "./functions";

describe("$computed functions", () => {
  it("offers the contracted set", () => {
    expect(FUNCTION_NAMES).toEqual("add sub mul div mod min max abs round floor eq not and or len at concat join slice includes range".split(" "));
  });

  it("does arithmetic", () => {
    expect(f.add({ a: 2, b: 3 })).toBe(5);
    expect(f.sub({ a: 2, b: 3 })).toBe(-1);
    expect(f.mul({ a: 2, b: 3 })).toBe(6);
    expect(f.div({ a: 6, b: 3 })).toBe(2);
    expect(f.mod({ a: 7, b: 2 })).toBe(1);
    expect(f.min({ a: 7, b: 2 })).toBe(2);
    expect(f.max({ a: 7, b: 2 })).toBe(7);
    expect(f.abs({ value: -4 })).toBe(4);
    expect(f.round({ value: 2.567, digits: 2 })).toBe(2.57);
    expect(f.floor({ value: 2.9 })).toBe(2);
  });

  it("works on lists and text", () => {
    expect(f.len({ value: "abc" })).toBe(3);
    expect(f.at({ list: [1, 2, 3], index: -1 })).toBe(3);
    expect(f.concat({ a: "x", b: "y" })).toBe("xy");
    expect(f.concat({ a: [1], b: [2] })).toEqual([1, 2]);
    expect(f.join({ list: ["a", "b"], separator: "-" })).toBe("a-b");
    expect(f.slice({ value: [1, 2, 3, 4], start: 1, end: 3 })).toEqual([2, 3]);
    expect(f.includes({ list: [1, 2], value: 2 })).toBe(true);
    expect(f.range({ count: 3, start: 1 })).toEqual([1, 2, 3]);
  });

  it("does logic", () => {
    expect(f.eq({ a: 1, b: 1 })).toBe(true);
    expect(f.not({ value: 0 })).toBe(true);
    expect(f.and({ a: 1, b: 0 })).toBe(false);
    expect(f.or({ a: 1, b: 0 })).toBe(true);
  });

  it("returns null for bad arguments and never throws", () => {
    expect(f.add({ a: "x", b: 1 })).toBeNull();
    expect(f.div({ a: 1, b: 0 })).toBeNull();
    expect(f.mod({ a: 1, b: 0 })).toBeNull();
    expect(f.at({ list: "no", index: 0 })).toBeNull();
    expect(f.range({ count: 1e9 })).toBeNull();
    expect(f.len({ value: 3 })).toBeNull();
    expect(() => f.add(undefined as never)).not.toThrow();
    expect(f.add(undefined as never)).toBeNull();
  });
});
