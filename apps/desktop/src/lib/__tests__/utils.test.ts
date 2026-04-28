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
    // prettier-plugin-tailwindcss 不会重排不同 cn() 参数之间的顺序
    expect(cn("p-2", "p-4")).toBe("p-4");
    expect(cn("text-base", "text-sm")).toBe("text-sm");
  });

  it("preserves non-conflicting utilities while resolving conflicts", () => {
    // tailwind-merge 保留输入顺序、丢弃被覆盖的冲突项
    expect(cn("flex p-2", "items-center p-4")).toBe("flex items-center p-4");
  });
});
