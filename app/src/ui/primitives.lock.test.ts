// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **One React Aria** (ADR 0104 §1, #2908): the component layer takes its hooks from `react-aria`
 * directly, for the targets that are not React Aria components (`ui/tooltip`). That pin is the
 * very version `react-aria-components` is built on, so the window holds one copy of its state:
 * the focus's modality, the tooltips' warm-up. A bump of one without the other fails here.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const read = (file: string) =>
  JSON.parse(readFileSync(join(process.cwd(), file), "utf8")) as {
    dependencies?: Record<string, string>;
    packages?: Record<string, { version?: string; dependencies?: Record<string, string> }>;
  };

describe("the primitives' hooks", () => {
  it("are pinned to the version react-aria-components is built on", () => {
    const pinned = read("package.json").dependencies?.["react-aria"];
    const lock = read("package-lock.json").packages ?? {};
    const builtOn = lock["node_modules/react-aria-components"]?.dependencies?.["react-aria"];
    expect(pinned).toBeDefined();
    expect(pinned).toBe(builtOn);
    expect(lock["node_modules/react-aria"]?.version).toBe(pinned);
    // No second copy nested under the components.
    expect(lock["node_modules/react-aria-components/node_modules/react-aria"]).toBeUndefined();
  });
});
