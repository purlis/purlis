// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { FIRST_ASK, aDialogOffers, showsTheQuestion } from "./opening";

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

describe("a dialog that offers an answer, as a spec waits for it", () => {
  /** A dialog of `role` saying `title`, with buttons reading `buttons`. */
  function offering(role: string, title: string, buttons: string[]): HTMLElement {
    const dialog = document.createElement("div");
    dialog.setAttribute("role", role);
    dialog.append(title);
    for (const words of buttons) {
      const button = document.createElement("button");
      button.textContent = words;
      dialog.append(button);
    }
    document.body.append(dialog);
    return dialog;
  }

  it("is the picker once it takes the palette's place, and not before", () => {
    const palette = offering("dialog", "Palette", ["New tab"]);
    expect(aDialogOffers("dialog", "", "Start", false)).toBe(false);
    palette.remove();
    offering("dialog", "Start a chat", ["Start", "Cancel"]);
    expect(aDialogOffers("dialog", "", "Start", false)).toBe(true);
  });

  it("is the question about the chat named, not one asked before it", () => {
    const before = offering("alertdialog", "End chat one?", ["Close", "Cancel"]);
    expect(aDialogOffers("alertdialog", "End chat two", "Close", false)).toBe(false);
    before.remove();
    offering("alertdialog", "End chat two?", ["Close", "Cancel"]);
    expect(aDialogOffers("alertdialog", "End chat two", "Close", false)).toBe(true);
  });

  it("matches a button by part of its words only when asked to", () => {
    offering("alertdialog", "Close this project?", ["Close and end 2 chats"]);
    expect(aDialogOffers("alertdialog", "", "Close and end", false)).toBe(false);
    expect(aDialogOffers("alertdialog", "", "Close and end", true)).toBe(true);
  });

  it("is gone once the dialog has gone", () => {
    offering("alertdialog", "End chat one?", ["Close"]).remove();
    expect(aDialogOffers("alertdialog", "End chat one", "Close", false)).toBe(false);
  });
});

/** Every scenario source but this module's own and the vitest files: specs, helpers, benches. */
function scenarioSources(): { name: string; text: string }[] {
  const here = dirname(fileURLToPath(import.meta.url));
  const read = (dir: string, prefix: string) =>
    readdirSync(dir)
      .filter((name) => name.endsWith(".ts") && !name.endsWith(".test.ts"))
      .map((name) => ({
        name: `${prefix}${name}`,
        text: readFileSync(join(dir, name), "utf8"),
      }));
  return [
    ...read(here, ""),
    ...read(join(here, "specs"), "specs/"),
    ...read(join(here, "bench"), "bench/"),
  ].filter((one) => one.name !== "opening.ts");
}

describe("a scenario that opens a project through the trust gate", () => {
  it("never waits on the trust question's title itself", () => {
    // In any quotes: a template literal is the same wait.
    const title = new RegExp(`["'\`]${FIRST_ASK.replace("?", "\\?")}["'\`]`);
    const naming = scenarioSources().filter((one) => title.test(one.text));
    expect(
      naming.map((one) => one.name),
      "these wait on the trust question's title themselves; call " +
        "openThroughTheTrustQuestion() from opening.ts, which reads the page again at every look",
    ).toEqual([]);
  });

  it("never presses the question's Open project through a dialog found before it", () => {
    const pressing = scenarioSources().filter((one) =>
      /\$\(\s*['"`]\[role="dialog"\]['"`]\s*\)\s*\.\$\(\s*['"`]button=Open project['"`]/.test(
        one.text,
      ),
    );
    expect(
      pressing.map((one) => one.name),
      "these press Open project in whichever dialog came first; call " +
        "openThroughTheTrustQuestion() from opening.ts",
    ).toEqual([]);
  });
});
