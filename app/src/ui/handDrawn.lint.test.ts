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
import { LINTED_FAMILIES } from "../../eslint.ui-rules.js";

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
    // A bare button, and the strip's controls by the words they wore before the tabs family's
    // one class (#3012): a crumb, a chip's hand and counts, a closer.
    `<button type="button" className="bare">x</button>`,
    `<button type="button" className="crumb own">x</button>`,
    `<button type="button" className="tab-tasks-hand">x</button>`,
    `<button type="button" className="tab-tasks-counts">x</button>`,
    `<button type="button" className="closer">x</button>`,
    // The other families' controls by the words views gave them, before each family's one
    // class (#3523): a list's, a card's and the title bar's.
    `<button type="button" className="vault-secret">x</button>`,
    `<button type="button" className="needs-you-mark rolled-up">x</button>`,
    `<button type="button" className="welcome-door">x</button>`,
    `<button type="button" className="welcome-link repo-card">x</button>`,
    `<button type="button" className="plane-root">x</button>`,
    // The card family's old class for a card pressed whole, now its own member (#3655).
    `<button type="button" className="ui-card-control welcome-door">x</button>`,
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

  it.each([
    // The strip's own controls, by the tabs family's class (#3012).
    `<button type="button" className="ui-tabs-control closer">x</button>`,
  ])("lets another family's control through by that family's class: %s", async (code) => {
    expect(await broken(`export const X = () => ${code};`)).toEqual([]);
  });

  it("names, in each family's rawButtons, only that family's own classes (#3523)", async () => {
    for (const family of LINTED_FAMILIES) {
      const { rawButtons } = (await import(`./${family}/lint.js`)) as {
        rawButtons?: { classes?: string[] };
      };
      // A family's own class is `ui-<family>-…`; the list's row is the list's `ui-row` (DS-12).
      for (const name of rawButtons?.classes ?? [])
        expect(name, family).toMatch(
          family === "list" ? /^ui-(?:list-|row$)/ : new RegExp(`^ui-${family}-`),
        );
    }
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
