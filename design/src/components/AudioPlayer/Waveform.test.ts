import { describe, expect, it } from "vitest";
import { resampleLevels, tailLevels } from "./Waveform";

describe("a waveform's bars", () => {
  it("stretches a recording's levels to the bars that fit, keeping both ends", () => {
    const bars = resampleLevels([0, 1], 5);
    expect(bars).toEqual([0, 0.25, 0.5, 0.75, 1]);
    expect(resampleLevels([0.3, 0.6, 0.9], 3)).toEqual([0.3, 0.6, 0.9]);
    expect(resampleLevels([], 4)).toEqual([]);
  });

  it("draws a live microphone newest at the right, silence before it", () => {
    expect(tailLevels([0.5, 0.7], 4)).toEqual([0, 0, 0.5, 0.7]);
    expect(tailLevels([0.1, 0.2, 0.3, 0.4, 0.5], 3)).toEqual([0.3, 0.4, 0.5]);
    expect(tailLevels([0.4], 0)).toEqual([]);
  });
});
