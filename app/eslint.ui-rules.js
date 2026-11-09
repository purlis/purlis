// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The component layer's lint** (#2904, spec #2897, ADR 0104): a screen draws a control from
 * its family in `src/ui/<family>`, never by hand.
 *
 * **One rule per family**, so each family's ticket turns its own rule into an error as it moves
 * every call site (ADR 0104 §5-6). **Each family's rules are in its own folder**,
 * `src/ui/<family>/lint.js` (#3387, operator ruling 2026-10-26: two family pull requests never
 * touch the same lines): it exports `rules`, by name (`purlis/<name>`), and may export `off`, the
 * files each of its rules is still off in until their lanes move them. This module finds every
 * such file, in the families' names' order, and `eslint.config.js` turns every rule found on as an
 * error. A pull request that adds a family's rule adds its folder's `lint.js` and its
 * `lint.test.ts` beside it, and no line here. What the rules are made of is
 * `src/ui/familyLint.js`.
 *
 * The primitives library itself is refused outside `src/ui` by `no-restricted-imports`, in
 * `eslint.config.js`: one wrapper per primitive, and screens never import it.
 */

import { existsSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

/** The families' folder. */
const UI = join(dirname(fileURLToPath(import.meta.url)), "src", "ui");

/** Every family with a lint of its own, by name. */
export const LINTED_FAMILIES = readdirSync(UI, { withFileTypes: true })
  .filter((entry) => entry.isDirectory() && existsSync(join(UI, entry.name, "lint.js")))
  .map((entry) => entry.name)
  .sort();

const lints = await Promise.all(
  LINTED_FAMILIES.map(async (family) => ({
    family,
    ...(await import(/* @vite-ignore */ pathToFileURL(join(UI, family, "lint.js")).href)),
  })),
);

/** Every family's rules, by name; two families naming one rule is a mistake, and throws. */
function everyRule() {
  const rules = {};
  for (const { family, rules: own = {} } of lints)
    for (const [name, rule] of Object.entries(own)) {
      if (name in rules) throw new Error(`purlis/${name} is named twice, again by ui/${family}`);
      rules[name] = rule;
    }
  return rules;
}

export const purlisUi = { meta: { name: "purlis-ui" }, rules: everyRule() };

/** The files each family's rule is still off in (a family's `off`), as config objects. */
export const stillOff = lints.flatMap(({ off = {} }) =>
  Object.entries(off).map(([name, files]) => ({ files, rules: { [`purlis/${name}`]: "off" } })),
);

/** The words of an answer that grants, which no menu row says (`menuModel.test.ts`). */
export { GRANT_WORDS } from "./src/ui/menu/lint.js";
