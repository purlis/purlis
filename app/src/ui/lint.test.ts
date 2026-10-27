// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **Every family's lint is found, and on** (`eslint.ui-rules.js`, #3387): each family's rules are
 * in its own folder (`src/ui/<family>/lint.js`), tested beside it (`lint.test.ts`), and the
 * window's lint (`eslint.config.js`) turns each on as an error on a screen, and off only in the
 * files the family says it is still off in.
 */
import { ESLint } from "eslint";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { LINTED_FAMILIES, purlisUi, stillOff } from "../../eslint.ui-rules.js";

const UI = join(process.cwd(), "src", "ui");
const eslint = new ESLint({ cwd: process.cwd() });

describe("the families' lint", () => {
  it("is found in each family's folder, and tested beside it", () => {
    expect(LINTED_FAMILIES.length).toBeGreaterThan(0);
    expect(Object.keys(purlisUi.rules).length).toBeGreaterThanOrEqual(LINTED_FAMILIES.length);
    for (const family of LINTED_FAMILIES)
      expect(existsSync(join(UI, family, "lint.test.ts")), family).toBe(true);
  });

  it("turns every family's rule on, as an error, on a screen", async () => {
    const config = await eslint.calculateConfigForFile("src/Planted.tsx");
    for (const name of Object.keys(purlisUi.rules)) {
      const level = [config.rules?.[`purlis/${name}`]].flat()[0];
      expect([2, "error"], name).toContain(level);
    }
  });

  it("turns a family's rule off in each file the family names", async () => {
    for (const { files, rules } of stillOff)
      for (const file of [files ?? []].flat(2) as string[]) {
        const config = await eslint.calculateConfigForFile(file);
        for (const name of Object.keys(rules ?? {}))
          expect([0, "off"], `${name} in ${file}`).toContain([config.rules?.[name]].flat()[0]);
      }
  });
});
