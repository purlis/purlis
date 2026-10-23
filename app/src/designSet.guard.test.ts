// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * **The window's views take their form controls from the set** (DS-3, #626 line 2; D-626-5).
 *
 * The settings set (`settings/components.tsx`, ADR 0037 as amended) is the one house set, and
 * `oldFormClasses.test.ts` keeps the classes it replaced from coming back. This is the other
 * half: what "a view uses only the set" means, checked.
 *
 * **What the set means for a view (D-626-5).** A box, a pick or a tick that is part of a form is
 * a `Field` or a `Choice` in a `SettingRow`, so it has the set's label, help, keyboard and look.
 * The set does not draw buttons: a `<button>` is native HTML that already does the job
 * (`docs/ui-primitives.md`, *The rule*), and a form's or a question's buttons are the caller's,
 * in `SettingActions` or a dialog's footer. So the guard is about the three native form
 * controls, `<input>`, `<select>` and `<textarea>`: outside the set, a file draws one only
 * where it is named below, with how many and why.
 *
 * **Not a form row** (`STANDS`) is the one kind of entry, exact about the file and about the
 * count: a control that is the surface itself rather than a row of a form and that no family
 * draws yet. A box a person types in, a filter, a reply or a secret's value among them, is the
 * field family's (`ui/field`, #2905) and is never native here.
 *
 * **There is no debt any more** (#626 line 2). The list of form controls that should have been
 * a `Field` or a `Choice` was paid off file by file: the Chats list's state chips are the set's
 * `chips`, the dispatch question's two radios a `Choice` radio, the grant table's selects and
 * the limits table's boxes the set's controls in a table's cell or a sentence (a control with
 * no row, `RowIds.label`), the folder box a `Field`, and the host a sandbox block did not name a
 * `Field` in a form row under its Notice. So a native form control outside the set is either the
 * surface itself, named below with why, or a failure here: the set is how a form control is drawn.
 *
 * An exact count and not an upper bound, so an entry cannot outlive what it names: a file that
 * draws fewer than its entry says, or none, fails here until the entry is lowered or dropped.
 */

const SRC = join(process.cwd(), "src");

/** The set itself: the one file that draws the native controls for everybody else. */
const THE_SET = "settings/components.tsx";

/**
 * Native form controls that are the surface itself, not a row of a form. A box a person types
 * in is the field family's (`ui/field`, #2905) wherever it stands, a filter, a reply, a secret's
 * value, so none is named here; what is left is a control no family draws yet.
 */
const STANDS: Record<string, { count: number; why: string }> = {
  "settings/SettingsTab.tsx": {
    count: 1,
    why: "the colour well, which no Field kind draws, in the row's control slot",
  },
};

/** Every file under `dir` whose name `keep` takes. */
function files(dir: string, keep: (name: string) => boolean): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return files(path, keep);
    return keep(entry.name) ? [path] : [];
  });
}

/** Every component file the window is drawn from: not tests, which render controls freely. */
// A story draws what it holds a family to (a popover's filter field), as a test does: it is no
// view of the window's.
const views = () =>
  files(SRC, (name) => name.endsWith(".tsx") && !/\.(test|stories)\.tsx$/.test(name));

/** How many native form controls `source` draws, comments left out: a doc comment that names
 *  `<input>` draws nothing. */
export function nativeControls(source: string): number {
  const code = source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:])\/\/.*$/gm, "$1");
  return [...code.matchAll(/<(input|select|textarea)(?=[\s/>])/g)].length;
}

/** Each file whose native form controls differ from what {@link STANDS} names for it, either
 *  way, given what each view draws: a file over its entry, and an entry over its file, which is
 *  stale. */
function offTheList(drawn: ReadonlyMap<string, number>): string[] {
  const files = new Set([...drawn.keys(), ...Object.keys(STANDS)]);
  return [...files].flatMap((file) => {
    const count = drawn.get(file) ?? 0;
    const named = STANDS[file]?.count ?? 0;
    return count !== named ? [`${file}: ${count} native form controls, ${named} named`] : [];
  });
}

describe("the views take their form controls from the set (#626, D-626-5)", () => {
  it("draw a native input, select or textarea only where it is named, and as many as named", () => {
    const drawn = new Map(
      views()
        .map((path) => [relative(SRC, path), nativeControls(readFileSync(path, "utf8"))] as const)
        // The set, and the component layer, where a family wraps its native element (#2905).
        .filter(([file]) => file !== THE_SET && !file.startsWith("ui/")),
    );
    expect(offTheList(drawn)).toEqual([]);
  });

  it("fails an entry its file no longer bears out, so none goes stale (#626)", () => {
    // A box moved onto the set, still named: the file draws none of what its entry says.
    expect(offTheList(new Map([["settings/SettingsTab.tsx", 0]]))).toContain(
      "settings/SettingsTab.tsx: 0 native form controls, 1 named",
    );
    // A named file the views no longer have at all.
    expect(
      offTheList(new Map()).some((line) => line.startsWith("settings/SettingsTab.tsx: 0")),
    ).toBe(true);
    // And one drawn where nothing names it.
    expect(offTheList(new Map([["Fresh.tsx", 1]]))).toContain(
      "Fresh.tsx: 1 native form controls, 0 named",
    );
  });

  it("name only files that exist, each with a reason", () => {
    for (const [file, { count, why }] of Object.entries(STANDS)) {
      expect(existsSync(join(SRC, file)), file).toBe(true);
      expect(count, file).toBeGreaterThan(0);
      expect(why.length, file).toBeGreaterThan(20);
    }
  });

  it("finds the set drawing them, so a reader that found nothing would fail", () => {
    expect(nativeControls(readFileSync(join(SRC, THE_SET), "utf8"))).toBeGreaterThan(0);
  });

  it("counts what a view draws and not what a comment names", () => {
    expect(nativeControls(`<input type="text" />`)).toBe(1);
    expect(nativeControls(`<select\n  value={v}>`)).toBe(1);
    expect(nativeControls(`<textarea>`)).toBe(1);
    expect(nativeControls(`/** an <input> here */ <p />`)).toBe(0);
    expect(nativeControls(`// an <input> here\n<p />`)).toBe(0);
    expect(nativeControls(`<inputs />`)).toBe(0);
    expect(nativeControls(`const url = "https://x/<input"; <p />`)).toBe(0);
  });
});
