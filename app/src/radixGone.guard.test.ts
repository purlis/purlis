// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { sourceFiles, SRC, withoutComments } from "./guards.testkit";

/**
 * **Radix is gone** (#2930, spec #2897, ADR 0104): every family is on React Aria, wrapped once in
 * `src/ui`, and the last Radix package, `react-roving-focus`, left with the settings nav and the
 * Inbox. Nothing the window is built from imports a `@radix-ui/*` package, and none is a
 * dependency, so one cannot come back by a lockfile alone.
 */

const APP = join(SRC, "..");

describe("Radix (#2930)", () => {
  it("is imported by no file the window is built from, its families and stories included", () => {
    // A test may name a Radix import in a fixture a lint or a guard must refuse; it imports none.
    const built = (name: string) => /\.(?:tsx?|jsx?)$/.test(name) && !/\.test\.tsx?$/.test(name);
    const importing = sourceFiles(built).filter((file) =>
      /from\s+["']@radix-ui\//.test(withoutComments(readFileSync(join(SRC, file), "utf8"), "ts")),
    );
    expect(importing).toEqual([]);
  });

  it("is no dependency of the app", () => {
    const manifest = JSON.parse(readFileSync(join(APP, "package.json"), "utf8")) as Record<
      string,
      Record<string, string> | undefined
    >;
    const named = ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"]
      .flatMap((field) => Object.keys(manifest[field] ?? {}))
      .filter((name) => name.startsWith("@radix-ui/"));
    expect(named).toEqual([]);
    expect(readFileSync(join(APP, "package-lock.json"), "utf8")).not.toContain(
      '"node_modules/@radix-ui/',
    );
  });
});
