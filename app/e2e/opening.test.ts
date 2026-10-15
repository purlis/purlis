// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { FIRST_ASK, showsTheQuestion } from "./opening";

/**
 * **The trust question is waited for by what it says, read again at every look** (#2126).
 *
 * Making a project ends in two dialogs, one after the other: New project, still up while it says
 * "Creating…", and then the trust question in its place. A wait that took "the dialog" when it
 * began could take the first, and keep looking at it after it had gone: the driver inside the app
 * hands a script a node that has left the page without calling it stale, so WebdriverIO never
 * finds the selector again and reports "still not displayed" for 30 s while the question is up.
 * `showsTheQuestion` runs in the page and looks again each time it is asked, so there is no node
 * to hold.
 */

afterEach(() => {
  document.body.replaceChildren();
});

/** A dialog drawn the way a Radix dialog is: its title is what `aria-labelledby` names. */
function aDialog(id: string, title: string, body = ""): HTMLElement {
  const dialog = document.createElement("div");
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-labelledby", id);
  const heading = document.createElement("h2");
  heading.id = id;
  heading.textContent = title;
  dialog.append(heading);
  if (body !== "") {
    const said = document.createElement("p");
    said.textContent = body;
    dialog.append(said);
  }
  document.body.append(dialog);
  return dialog;
}

describe("the trust question, as a spec waits for it", () => {
  it("is not the New project dialog that comes before it", () => {
    aDialog("new-project", "New project", "Creating…");
    expect(showsTheQuestion(FIRST_ASK)).toBe(false);
  });

  it("is seen once it takes the New project dialog's place", () => {
    const making = aDialog("new-project", "New project", "Creating…");
    expect(showsTheQuestion(FIRST_ASK)).toBe(false);
    making.remove();
    aDialog("approve-plane", FIRST_ASK);
    expect(showsTheQuestion(FIRST_ASK)).toBe(true);
  });

  it("is seen behind another dialog drawn before it", () => {
    aDialog("other", "Something else");
    aDialog("approve-plane", FIRST_ASK);
    expect(showsTheQuestion(FIRST_ASK)).toBe(true);
  });

  it("is not seen once it has gone", () => {
    aDialog("approve-plane", FIRST_ASK).remove();
    expect(showsTheQuestion(FIRST_ASK)).toBe(false);
  });
});

/** Every scenario source but this module's own and the vitest files. */
function scenarioSources(): { name: string; text: string }[] {
  const here = dirname(fileURLToPath(import.meta.url));
  const read = (dir: string, prefix: string) =>
    readdirSync(dir)
      .filter((name) => name.endsWith(".ts") && !name.endsWith(".test.ts"))
      .map((name) => ({
        name: `${prefix}${name}`,
        text: readFileSync(join(dir, name), "utf8"),
      }));
  return [...read(here, ""), ...read(join(here, "specs"), "specs/")].filter(
    (one) => one.name !== "opening.ts",
  );
}

describe("a scenario that opens a project through the trust gate", () => {
  it("waits for the question through `openThroughTheTrustQuestion`, never on a dialog it held", () => {
    const holding = scenarioSources().filter((one) => one.text.includes(`"${FIRST_ASK}"`));
    expect(
      holding.map((one) => one.name),
      "these wait for the trust question by its words themselves; call " +
        "openThroughTheTrustQuestion() from opening.ts, which reads the page again at every look",
    ).toEqual([]);
  });
});
