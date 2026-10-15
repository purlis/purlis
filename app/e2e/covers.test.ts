// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { coveredIn } from "./covers";

/**
 * **A test that could not press or see an element says what covered it** (`covers.ts`): the
 * element's selector is read out of WebdriverIO's message, and only for those two failures.
 */
describe("a failure about an element it could not reach", () => {
  it("names the element's selector, for a press or a sight that never came", () => {
    expect(coveredIn('element ("button=New tab") still not clickable after 20000ms')).toBe(
      "button=New tab",
    );
    expect(coveredIn('element ("[role="dialog"]") still not displayed after 30000ms')).toBe(
      '[role="dialog"]',
    );
  });

  it("names none for any other failure", () => {
    expect(coveredIn("expect(received).toBe(expected)")).toBeUndefined();
    expect(coveredIn('element ("#x") still not existing after 20000ms')).toBeUndefined();
  });
});
