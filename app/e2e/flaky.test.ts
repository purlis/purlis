// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { flakesOfWorker } from "./flaky.js";

describe("a spec file that failed once and passed on its retry", () => {
  it("is named as a flake in the shared annotation format", () => {
    expect(
      flakesOfWorker(0, ["file:///repo/app/e2e/specs/onboarding.e2e.ts"], 0, "/repo/app", 1),
    ).toEqual(["::warning title=Flaky (passed on retry 1)%3A e2e/specs/onboarding.e2e.ts::failed"]);
  });

  it("says nothing when it passed first time", () => {
    expect(flakesOfWorker(0, ["/repo/app/e2e/specs/a.e2e.ts"], 1, "/repo/app", 1)).toEqual([]);
  });

  it("says nothing when it failed: the retry is not spent yet or the run is red", () => {
    expect(flakesOfWorker(1, ["/repo/app/e2e/specs/a.e2e.ts"], 0, "/repo/app", 1)).toEqual([]);
  });

  it("says nothing where specs are not retried", () => {
    expect(flakesOfWorker(0, ["/repo/app/e2e/specs/a.e2e.ts"], 0, "/repo/app", 0)).toEqual([]);
  });
});
