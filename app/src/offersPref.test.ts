// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { loadOffersOff, OFFERS } from "./offersPref";

/**
 * **Stop offers from <extension>, kept** (#4769, prototype `view-open-beside` Q3 A): the person's
 * preference, in the layout file under `offers`, until Settings › Extensions turns that
 * extension's offers back on. No capability: it only holds notes back.
 */
describe("the extensions whose offers are stopped (#4769)", () => {
  it("is none where the file says nothing", () => {
    expect(loadOffersOff({})).toEqual({ value: [], said: [] });
    expect(loadOffersOff(null)).toEqual({ value: [], said: [] });
  });

  it("reads the ids the file keeps", () => {
    expect(loadOffersOff({ offers: { off: ["statistics", "todos"] } })).toEqual({
      value: ["statistics", "todos"],
      said: [],
    });
  });

  it("says what it put right, and keeps the ids it could read", () => {
    const loaded = loadOffersOff({ offers: { off: ["statistics", 3] } });
    expect(loaded.value).toEqual(["statistics"]);
    expect(loaded.said).toHaveLength(1);
    expect(loadOffersOff({ offers: "all" }).value).toEqual([]);
    expect(loadOffersOff({ offers: "all" }).said).toHaveLength(1);
  });

  it("writes nothing while every extension may offer", () => {
    OFFERS.forget();
    expect(OFFERS.written()).toBeUndefined();
    OFFERS.set(["statistics"]);
    expect(OFFERS.written()).toEqual({ off: ["statistics"] });
    OFFERS.forget();
  });
});
