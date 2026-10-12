// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { FORGE_VARIABLE, THE_RUNS_FORGE, THE_RUNS_TREE, theRunsEnvironment } from "./harness.js";
import { holdThePush, letThePushGo, thePushIsHeld } from "./standInForge.js";

/**
 * The scenario's half of the stand-in forge (#1819), checked here because the spec that uses it
 * first runs in CI: a spelling the app does not read would leave the spec with no owners to pick.
 */

const here = dirname(fileURLToPath(import.meta.url));
const createproject = () =>
  readFileSync(join(here, "..", "src-tauri", "src", "createproject.rs"), "utf8");

afterEach(letThePushGo);

describe("the stand-in forge", () => {
  it("is named by the variable the e2e build reads, in every launcher's environment", () => {
    expect(createproject()).toContain(`pub const FORGE: &str = "${FORGE_VARIABLE}";`);
    expect(createproject()).toContain('dir.join("hold-before-push")');
    expect(theRunsEnvironment(join(THE_RUNS_TREE, "plane"))[FORGE_VARIABLE]).toBe(THE_RUNS_FORGE);
  });

  it("holds the push until it is let go", () => {
    holdThePush();
    expect(thePushIsHeld()).toBe(true);
    letThePushGo();
    expect(thePushIsHeld()).toBe(false);
    expect(existsSync(join(THE_RUNS_FORGE, "hold-before-push"))).toBe(false);
  });
});
