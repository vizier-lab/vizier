import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPageState, saveable } from "./state";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("page state", () => {
  it("seeds from the spec and overlays the saved state", () => {
    const { store } = createPageState({ initial: { a: 1, b: 2, _d: [1] }, saved: { a: 9 }, save: () => {} });
    expect(store.get("/a")).toBe(9);
    expect(store.get("/b")).toBe(2);
    expect(store.get("/_d")).toEqual([1]);
  });

  it("never lets a saved derived key in", () => {
    const { store } = createPageState({ initial: { _d: [1] }, saved: { _d: [2] }, save: () => {} });
    expect(store.get("/_d")).toEqual([1]);
  });

  it("saves only non-derived keys, once per interval", () => {
    const save = vi.fn();
    const { store } = createPageState({ initial: { n: 0, _d: 0 }, save });
    store.set("/n", 1);
    store.set("/n", 2);
    store.set("/_d", 5);
    expect(save).not.toHaveBeenCalled();
    vi.advanceTimersByTime(499);
    expect(save).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith({ n: 2 });
  });

  it("does not save when only a derived key changed", () => {
    const save = vi.fn();
    const { store } = createPageState({ initial: { n: 0 }, save });
    store.set("/_x", 1);
    vi.advanceTimersByTime(1000);
    expect(save).not.toHaveBeenCalled();
  });

  it("merges another tab's state without a local change or a save", () => {
    const save = vi.fn();
    const local = vi.fn();
    const page = createPageState({ initial: { n: 0 }, save });
    page.subscribeLocal(local);
    page.mergeRemote({ n: 7, _ignored: 1 });
    vi.advanceTimersByTime(1000);
    expect(page.store.get("/n")).toBe(7);
    expect(page.store.get("/_ignored")).toBeUndefined();
    expect(local).not.toHaveBeenCalled();
    expect(save).not.toHaveBeenCalled();
    page.store.set("/n", 8);
    expect(local).toHaveBeenCalledTimes(1);
  });

  it("flushes on demand and stops after dispose", () => {
    const save = vi.fn();
    const page = createPageState({ initial: { n: 0 }, save });
    page.store.set("/n", 3);
    page.flush();
    expect(save).toHaveBeenCalledWith({ n: 3 });
    page.dispose();
    page.store.set("/n", 4);
    vi.advanceTimersByTime(1000);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("strips derived keys from a snapshot", () => {
    expect(saveable({ a: 1, _b: 2 })).toEqual({ a: 1 });
  });
});
