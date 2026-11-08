// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import {
  FIRST_ASK,
  aDialogOffers,
  pressTheCloseAnswer,
  showsTheQuestion,
  whatCameInsteadOfThePicker,
  whatThePickerOffers,
} from "./opening";

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

  it("is still up while the button pressed says it is working, by its Cancel", () => {
    // How `theDialogHasGone` asks: the pressed button's words change, Cancel's do not.
    const starting = offering("dialog", "Start a chat", ["Starting…", "Cancel"]);
    expect(aDialogOffers("dialog", "Start a chat", "Start", false)).toBe(false);
    expect(aDialogOffers("dialog", "Start a chat", "Cancel", false)).toBe(true);
    starting.remove();
    expect(aDialogOffers("dialog", "Start a chat", "Cancel", false)).toBe(false);
  });

  it("is gone once the dialog has gone", () => {
    offering("alertdialog", "End chat one?", ["Close"]).remove();
    expect(aDialogOffers("alertdialog", "End chat one", "Close", false)).toBe(false);
  });
});

/**
 * **The close question is answered by its main button, whatever its words are when pressed**
 * (#5609). `ClosingProject` counts the project's chats on every render: asked while a chat is
 * still ending, its button reads "Close and end 1 chat", and once that chat has gone the same
 * button reads "Close project". On the merge queue's macOS runner the chat went between the look
 * that saw the question and the press, and a press that looked for "Close and end" found nothing
 * for 20 s; the spec's `after` stopped there with this machine's project open and in front, and
 * ten spec files after it failed in it.
 */
describe("the question a project's close asks, as a spec answers it", () => {
  /** The question as `ClosingProject` draws it, with its main button reading `main`. */
  function theQuestion(main: string): HTMLButtonElement {
    const dialog = document.createElement("section");
    dialog.setAttribute("role", "alertdialog");
    dialog.append("Close project local-project?");
    const cancel = document.createElement("button");
    cancel.textContent = "Cancel";
    const answer = document.createElement("button");
    answer.textContent = main;
    dialog.append(cancel, answer);
    document.body.append(dialog);
    return answer;
  }

  /** Counts the presses `button` takes. */
  function counting(button: HTMLElement): () => number {
    let pressed = 0;
    button.addEventListener("click", () => pressed++);
    return () => pressed;
  }

  it("presses Close and end while the question counts a chat", () => {
    const pressed = counting(theQuestion("Close and end 1 chat"));
    expect(pressTheCloseAnswer()).toBe("Close and end 1 chat");
    expect(pressed()).toBe(1);
  });

  it("presses Close project once the chat it counted has ended under it", () => {
    const answer = theQuestion("Close and end 1 chat");
    const pressed = counting(answer);
    answer.textContent = "Close project";
    expect(pressTheCloseAnswer()).toBe("Close project");
    expect(pressed()).toBe(1);
  });

  it("presses nothing, and never Cancel, while no close question is up", () => {
    const other = document.createElement("section");
    other.setAttribute("role", "alertdialog");
    other.append("End chat one?");
    const cancel = document.createElement("button");
    cancel.textContent = "Cancel";
    other.append(cancel);
    document.body.append(other);
    const pressed = counting(cancel);
    expect(pressTheCloseAnswer()).toBe("");
    expect(pressed()).toBe(0);
  });
});

/**
 * **The picker's Start is chosen in one look** (#4736, #4979). The picker opens before its rows'
 * details and marks them `aria-busy` until they arrive; until then its button reads Start chat
 * even for a profile that asks for its approval. `pickAndStart` asked "is it checking?" and
 * "which button?" in two calls to the page, and on macOS the dialog was drawn between them about
 * once a run: the first found no dialog to be busy, the second found its Start chat, the press
 * waited for the details, and they made it an approval, which is a press of its own (ADR 0097
 * §6), so the picker never went. One look answers both about the same dialog.
 */
describe("what the picker offers to press, as a spec chooses it", () => {
  /** The picker as `StartChat` draws it: its rows `aria-busy` while `checking`. */
  function picker(checking: boolean, button: string): HTMLElement {
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    dialog.append("Start a chat");
    const rows = document.createElement("div");
    rows.setAttribute("aria-busy", String(checking));
    rows.append("Harness");
    dialog.append(rows);
    for (const words of ["Cancel", button]) {
      const pressed = document.createElement("button");
      pressed.textContent = words;
      dialog.append(pressed);
    }
    document.body.append(dialog);
    return dialog;
  }

  it("is nothing before the picker is drawn", () => {
    expect(whatThePickerOffers("Start a chat")).toBeNull();
  });

  it("is nothing while the picker is still checking its rows, though it reads Start chat", () => {
    // The look that comes right after the picker is drawn: nothing seen busy before it.
    picker(true, "Start chat");
    expect(whatThePickerOffers("Start a chat")).toBeNull();
  });

  it("is the approval once the rows are checked and the profile asks for one", () => {
    const checking = picker(true, "Start chat");
    expect(whatThePickerOffers("Start a chat")).toBeNull();
    checking.remove();
    picker(false, "Approve and start");
    expect(whatThePickerOffers("Start a chat")).toBe("Approve and start");
  });

  it("is Start chat once the rows are checked and nothing asks", () => {
    picker(false, "Start chat");
    expect(whatThePickerOffers("Start a chat")).toBe("Start chat");
  });
});

/**
 * **What came instead of the picker** (#5580). In a project where no harness can start, New chat
 * opens the Set up a harness tab and never draws the picker (FR-29). On macOS in the merge queue a
 * spec before the palette's left the window on such a project, and the palette's first test
 * waited 20 s for a Start that could never come, then said only "the picker never offered Start".
 */
describe("what a press of New chat opened in the picker's place", () => {
  /** The tab strip as `PlaneView` draws it, with `names` on it and `front` selected. */
  function strip(names: string[], front: string): void {
    const tabs = document.createElement("div");
    tabs.setAttribute("data-strip", "Tabs");
    for (const name of names) {
      const tab = document.createElement("div");
      tab.setAttribute("role", "tab");
      tab.setAttribute("aria-selected", String(name === front));
      const said = document.createElement("span");
      said.className = "tab-name";
      said.textContent = name;
      tab.append(said);
      tabs.append(tab);
    }
    document.body.append(tabs);
  }

  it("is the Set up a harness tab when it is the one in front", () => {
    strip(["New chat", "Set up a harness · forges-hub"], "Set up a harness · forges-hub");
    expect(whatCameInsteadOfThePicker()).toBe("Set up a harness · forges-hub");
  });

  it("is nothing while a chat's tab is in front", () => {
    strip(["Set up a harness · forges-hub", "New chat"], "New chat");
    expect(whatCameInsteadOfThePicker()).toBeNull();
  });

  it("is nothing while a picker is up in front of it, though still checking its rows", () => {
    // The setup tab's own Start a chat draws the picker over it (#4736: rows busy at first).
    strip(["Set up a harness · forges-hub"], "Set up a harness · forges-hub");
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    dialog.append("Start a chat");
    const rows = document.createElement("div");
    rows.setAttribute("aria-busy", "true");
    dialog.append(rows);
    document.body.append(dialog);
    expect(whatCameInsteadOfThePicker()).toBeNull();
  });

  it("is nothing with no strip drawn", () => {
    expect(whatCameInsteadOfThePicker()).toBeNull();
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
