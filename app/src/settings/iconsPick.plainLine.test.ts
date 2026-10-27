// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { plainLine } from "./iconsPick";

describe("an extension's name as one plain line (SK-51, #3202)", () => {
  it("drops what the one rule hides, keeps white space a space, and an emoji whole", () => {
    expect(plainLine("Sol‮arized​")).toBe("Solarized");
    expect(plainLine("Dark Pro\nTheme")).toBe("Dark Pro Theme");
    expect(plainLine("Fam \u{1F468}‍\u{1F469}")).toBe("Fam \u{1F468}‍\u{1F469}");
  });
});
