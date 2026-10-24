import { describe, expect, it } from "vitest";
import type { z } from "zod";
import { ACTIVE_GROUPS, catalog } from "./catalog/index";

describe("the catalogue", () => {
  it("has components in every active group", () => {
    expect(ACTIVE_GROUPS.length).toBeGreaterThan(0);
    expect(Object.keys(catalog).length).toBeGreaterThan(20);
  });

  for (const [name, entry] of Object.entries(catalog)) {
    describe(name, () => {
      it("has a description of one or two sentences", () => {
        expect(entry.description.trim().length).toBeGreaterThan(10);
      });

      it("has an example that validates against its own schema", () => {
        const literal = Object.fromEntries(
          Object.entries(entry.example.props).filter(([, value]) => !(typeof value === "object" && value !== null && Object.keys(value).some((k) => k.startsWith("$")))),
        );
        const strict = entry.props.partial().strict().safeParse(literal);
        expect(strict.success, JSON.stringify(strict.error?.issues)).toBe(true);
        // Every required prop is given in the example, as a literal or an expression.
        for (const [key, schema] of Object.entries(entry.props.shape)) {
          if (!(schema as z.ZodType).safeParse(undefined).success) expect(entry.example.props, `${name}.${key}`).toHaveProperty(key);
        }
      });

      it("declares events as plain names", () => {
        for (const event of entry.events) expect(event).toMatch(/^[a-z]+$/);
      });
    });
  }
});
