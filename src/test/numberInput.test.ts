import { describe, it, expect } from "vitest";
import { normalizeIntegerRaw, normalizeDecimalRaw } from "../numberInput";

describe("normalizeIntegerRaw", () => {
  it("keeps transient prefixes", () => {
    expect(normalizeIntegerRaw("")).toBe("");
    expect(normalizeIntegerRaw("-")).toBe("-");
  });

  it("strips everything from the first non-digit", () => {
    expect(normalizeIntegerRaw("1.5")).toBe("1");
    expect(normalizeIntegerRaw("12abc")).toBe("12");
    expect(normalizeIntegerRaw("1e5")).toBe("1");
    expect(normalizeIntegerRaw("1,5")).toBe("1");
    expect(normalizeIntegerRaw("0.")).toBe("0");
  });

  it("strips leading zeros", () => {
    expect(normalizeIntegerRaw("007")).toBe("7");
    expect(normalizeIntegerRaw("00")).toBe("0");
    expect(normalizeIntegerRaw("0")).toBe("0");
  });

  it("preserves the sign", () => {
    expect(normalizeIntegerRaw("-5")).toBe("-5");
    expect(normalizeIntegerRaw("-05")).toBe("-5");
    expect(normalizeIntegerRaw("-1.5")).toBe("-1");
  });
});

describe("normalizeDecimalRaw", () => {
  it("keeps transient prefixes", () => {
    expect(normalizeDecimalRaw("")).toBe("");
    expect(normalizeDecimalRaw("-")).toBe("-");
    expect(normalizeDecimalRaw(".")).toBe(".");
    expect(normalizeDecimalRaw("-.")).toBe("-.");
  });

  it("keeps one dot and strips the rest", () => {
    expect(normalizeDecimalRaw("1.5")).toBe("1.5");
    expect(normalizeDecimalRaw("1.2.3")).toBe("1.23");
    expect(normalizeDecimalRaw("0.50")).toBe("0.50");
    expect(normalizeDecimalRaw(".5")).toBe(".5");
    expect(normalizeDecimalRaw("-.5")).toBe("-.5");
  });

  it("strips non-digit, non-dot characters", () => {
    expect(normalizeDecimalRaw("1e5")).toBe("15");
    expect(normalizeDecimalRaw("1,5")).toBe("15");
    expect(normalizeDecimalRaw("a1.5b")).toBe("1.5");
  });

  it("strips leading zeros", () => {
    expect(normalizeDecimalRaw("007")).toBe("7");
    expect(normalizeDecimalRaw("00.5")).toBe("0.5");
    expect(normalizeDecimalRaw("0")).toBe("0");
  });

  it("preserves the sign", () => {
    expect(normalizeDecimalRaw("-1.5")).toBe("-1.5");
    expect(normalizeDecimalRaw("-00.5")).toBe("-0.5");
  });
});
