import { describe, it, expect } from "vitest";
import { cn } from "../utils";

describe("cn", () => {
  it("joins basic class names with a space", () => {
    expect(cn("a", "b")).toBe("a b");
  });

  it("filters falsy values", () => {
    expect(cn("a", false, undefined, null, "", "b")).toBe("a b");
  });

  it("supports conditional object syntax", () => {
    expect(cn("base", { active: true, hidden: false })).toBe("base active");
  });

  it("dedupes conflicting Tailwind utilities, last wins", () => {
    expect(cn("p-2", "p-4")).toBe("p-4");
    expect(cn("text-sm text-base")).toBe("text-base");
  });

  it("preserves non-conflicting utilities while resolving conflicts", () => {
    expect(cn("flex p-2", "p-4 items-center")).toBe("flex p-4 items-center");
  });
});
