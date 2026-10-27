// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import cases from "./invisible.cases.json";
import { hides, marked, markOf } from "./invisible";

describe("the window's one rule (SK-51, #3202)", () => {
  it("marks every text as the core does, line ends kept and marked", () => {
    // `invisible.cases.json` is the core's own answers (`unicoderule.rs`): a difference here is
    // the window drawing a character the core marks, or marking one the core reads as text.
    const wrong = cases.filter(
      (one) => marked(one.text, "kept") !== one.kept || marked(one.text, "marked") !== one.marked,
    );
    expect(wrong).toEqual([]);
    expect(cases.length).toBeGreaterThan(200);
  });

  it("writes the core's mark", () => {
    expect(marked("git commit", "marked")).toBe("git⟦U+00A0⟧commit");
    expect(marked("a\nb\tc", "marked")).toBe("a⟦U+000A⟧b⟦U+0009⟧c");
    expect(marked("a\nb\tc", "kept")).toBe("a\nb\tc");
    expect(markOf("\u{E0049}")).toBe("⟦U+E0049⟧");
  });

  it("reads a joiner inside a script or an emoji as text", () => {
    for (const text of [
      "\u{1F468}‍\u{1F469}‍\u{1F467}",
      "\u{1F3F3}️‍\u{1F308}",
      "❤️",
      "क्‍ष",
      "می‌خواهم",
      "#️⃣",
    ]) {
      expect(hides(text, "marked")).toBe(false);
    }
    expect(marked("a‍b", "marked")).toBe("a⟦U+200D⟧b");
  });

  it("marks a lone surrogate, which no Rust text can hold", () => {
    expect(marked("a\uD800b", "marked")).toBe("a⟦U+D800⟧b");
  });

  it("is idempotent", () => {
    const once = marked("a‮b\n\u{1F468}‍\u{1F469}", "marked");
    expect(marked(once, "marked")).toBe(once);
  });
});
