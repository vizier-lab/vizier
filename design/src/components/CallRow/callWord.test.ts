import { describe, expect, it } from "vitest";
import { callWord } from "./CallRow";

describe("callWord", () => {
  it("names every state in plain English", () => {
    expect(callWord("listening")).toBe("Listening");
    expect(callWord("speaking")).toBe("Speaking");
    expect(callWord("reconnecting")).toBe("Reconnecting");
    expect(callWord("ended")).toBe("Call ended");
    expect(callWord("failed")).toBe("Call failed");
    expect(callWord("connecting")).not.toBe(callWord("reconnecting"));
  });
});
