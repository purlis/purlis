// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **The primitives library is one library at one version** (ADR 0104 §1, #2909). The menu family
 * reads React Aria's menu state from `react-stately` itself (`ui/menu/ContextMenu.tsx`), so the
 * window depends on it directly: pinned to exactly the version React Aria Components pins, so
 * an upgrade of one alone fails here rather than drawing menus from two copies.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const app = (file: string) => JSON.parse(readFileSync(join(process.cwd(), file), "utf8"));

describe("react-stately", () => {
  it("is pinned to the version React Aria Components pins", () => {
    const own = app("package.json").dependencies["react-stately"];
    const lock = app("package-lock.json").packages;
    const aria = lock["node_modules/react-aria-components"].dependencies["react-stately"];
    expect(own).toBe(aria);
    expect(lock["node_modules/react-stately"].version).toBe(own);
  });
});
