// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **The hand-drawn buttons and the old field wrapper cannot come back** (#3129): the window's own
 * lint (`eslint.config.js`, as `npm run lint` runs it on a screen) refuses each pattern the last
 * of them used, now that the button and field families draw them.
 */
import { ESLint } from "eslint";
import { describe, expect, it } from "vitest";

const eslint = new ESLint({ cwd: process.cwd() });

/** The rules `code` breaks, written in a screen of the window. */
async function broken(code: string): Promise<string[]> {
  const [result] = await eslint.lintText(code, { filePath: "src/Planted.tsx" });
  return result.messages.map((message) => message.ruleId ?? message.message);
}

describe("the window's lint on a screen (#3129)", () => {
  it.each([
    // A pane line's chips and ways, each a class a raw button wore as another family's.
    `<button type="button" className="pane-brief">x</button>`,
    `<button type="button" className="pane-harness">x</button>`,
    `<button type="button" className="pane-skills">x</button>`,
    `<button type="button" className="task-end">Stop</button>`,
    // The strip's fresh mark, its gear and its offers.
    `<button type="button" className="plane-updated fresh-mark">x</button>`,
    `<button type="button" className="gear">x</button>`,
    `<button type="button" className="strip-offer">x</button>`,
  ])("refuses a raw button drawn by hand: %s", async (code) => {
    expect(await broken(`export const X = () => ${code};`)).toContain("purlis/button-family");
  });

  it.each([
    `<span className="ui-field ui-field-range">x</span>`,
    `<input type="range" className="ui-field" />`,
    `<Select className={["ui-field", "wide"].join(" ")} />`,
  ])("refuses the old field wrapper: %s", async (code) => {
    expect(await broken(`export const X = () => ${code};`)).toContain("purlis/field-family");
  });

  it("lets the families' members through", async () => {
    expect(
      await broken(`import { Button, IconButton } from "./ui/button/Button";
export const X = () => (
  <>
    <Button size="sm" className="task-end">Stop</Button>
    <IconButton size="sm" className="pane-brief" icon={null} aria-label="Brief" />
  </>
);`),
    ).toEqual([]);
  });
});
