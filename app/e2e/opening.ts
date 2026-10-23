// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser } from "@wdio/globals";

/**
 * Opening a chat, as the operator does it since M1.2: press, pick, start.
 *
 * **No harness starts until somebody picks a profile** (ADR 0022), so every spec that wants
 * a session comes through here. That is also what keeps the rule from being quietly
 * removed: take the picker out of the path and every scenario spec fails at once.
 *
 * The plane these run in declares one profile, approved on its first use by the picker
 * scenario. Whichever spec gets there first pays the approval; the rest see `Start`. Asking
 * for whichever button is present keeps the specs independent of their order, which matters
 * because WebdriverIO's Tauri service keeps ONE app process for the whole run.
 */
export async function pickAndStart(): Promise<void> {
  // The picker can take the palette's place, so it is looked for by its buttons at every look,
  // and nothing found before it is held (#2126).
  let pressing = "";
  await browser.waitUntil(
    async () => {
      for (const words of ["Approve and start", "Start"])
        if (await browser.execute(aDialogOffers, "dialog", PICKER, words, false)) {
          pressing = words;
          return true;
        }
      return false;
    },
    { timeout: 20_000, timeoutMsg: "the picker never offered Start" },
  );
  const start = await $(`button=${pressing}`);
  await start.waitForClickable({ timeout: 20_000 });
  await start.click();
  await theDialogHasGone("dialog", PICKER, "the picker never went");
}

/** The picker's title (`StartChat`). */
const PICKER = "Start a chat";

/**
 * Whether the page shows a `role` dialog whose text holds `text` and which offers a button
 * reading `button` (or, `partly`, holding it), looked for afresh. **Run in the page**, by
 * `browser.execute`, so it closes over nothing (#2126): a dialog that takes another's place is
 * found, and one that has gone is gone, where a held node would answer about the first forever.
 */
export function aDialogOffers(
  role: string,
  text: string,
  button: string,
  partly: boolean,
): boolean {
  for (const dialog of document.querySelectorAll(`[role="${role}"]`)) {
    if (!(dialog.textContent ?? "").includes(text)) continue;
    const offered = [...dialog.querySelectorAll("button")].some((one) => {
      const said = (one.textContent ?? "").trim();
      return partly ? said.includes(button) : said === button;
    });
    if (!offered) continue;
    const seen = dialog as Element & { checkVisibility?: (options?: object) => boolean };
    if (
      typeof seen.checkVisibility !== "function" ||
      seen.checkVisibility({ opacityProperty: true, visibilityProperty: true })
    )
      return true;
  }
  return false;
}

/**
 * Waits until no `role` dialog saying `text` is left, asking the page each time. **By its Cancel**,
 * which each of these dialogs keeps while it works: the button that was pressed changes its words
 * ("Starting…") while the dialog is still up, so it cannot say the dialog went.
 */
async function theDialogHasGone(role: string, text: string, timeoutMsg: string): Promise<void> {
  try {
    await browser.waitUntil(
      async () => !(await browser.execute(aDialogOffers, role, text, "Cancel", false)),
      { timeout: 20_000 },
    );
  } catch {
    // Said with what the dialog reads, so a red run says why it stayed.
    const says = await browser.execute(
      (wanted: string) =>
        [...document.querySelectorAll(`[role="${wanted}"]`)]
          .map((one) => (one.textContent ?? "").replace(/\s+/g, " ").trim())
          .join(" | ")
          .slice(0, 2000),
      role,
    );
    throw new Error(`${timeoutMsg}. It reads: ${says}`);
  }
}

/**
 * The picker's harness rows, asked for by what they MEAN rather than by what they are made of.
 *
 * **One definition, used by every spec that waits for the picker to be ready.** It was three
 * copies of `input[type="radio"][name="profile"]`, which said "these rows are native inputs" —
 * a fact about the markup and not about the app. The primitives migration made them
 * `button[role="radio"]` and every copy became a selector that could never match, each one
 * costing a spec its full 20-second timeout before it failed.
 *
 * `role` is what a screen reader reads and what the operator gets; it survives whatever draws
 * it. Scoped to the harness group by the words that name it, because the persona rows are
 * radios too and "the first radio on the page" is a different kind of accident waiting. The
 * group is a settings row's choice (DS-3d), named by `aria-labelledby` pointing at the row's
 * "Harness" label — so the group is found by what that label SAYS, never by an id, which the
 * row draws with `useId` and which a spec cannot know.
 */
export async function harnessRowCount(): Promise<number> {
  return browser.execute(() => {
    const group = Array.from(document.querySelectorAll('[role="radiogroup"]')).find((one) => {
      const named = (one.getAttribute("aria-labelledby") ?? "")
        .split(" ")
        .map((id) => document.getElementById(id)?.textContent?.trim() ?? "")
        .join(" ");
      return named === "Harness";
    });
    return group ? group.querySelectorAll('[role="radio"]').length : 0;
  });
}

/** Waits until the picker has drawn the harness it is offering. */
export async function harnessRowsDrawn(): Promise<void> {
  await browser.waitUntil(async () => (await harnessRowCount()) > 0, {
    timeout: 20_000,
    timeoutMsg: "the picker drew no harness rows in 20 seconds",
  });
}

/**
 * Where the keyboard is, named the way somebody looking for that control would name it.
 *
 * **A DOM read and not a gesture, which is what makes it usable here at all.** A scenario
 * cannot press a button by keyboard — a synthesised `Enter` carries no activation
 * (charter-app#176) — but where the focus IS after a key is a property of the document, and
 * reading it costs nothing the harness cannot do.
 *
 * It names a control by its role and its accessible words, in the same vocabulary
 * `Modals.keyboard.test.tsx` uses, so the real engine's answer and jsdom's can be read side by
 * side. A radio row and a checkbox have their words in a `<label for>` rather than inside them,
 * which is why that is consulted before the element's own text: without it, half the picker
 * comes back as `checkbox ""`.
 */
export async function whereTheKeyboardIs(): Promise<string> {
  return browser.execute(() => {
    const on = document.activeElement;
    if (!on || on === document.body) return "(nothing)";
    const labelled = on.id ? document.querySelector(`label[for="${CSS.escape(on.id)}"]`) : null;
    const named = on.getAttribute("aria-label") ?? labelled?.textContent ?? on.textContent ?? "";
    const role = on.getAttribute("role") ?? on.tagName.toLowerCase();
    return `${role} ${JSON.stringify(named.trim().slice(0, 40))}`;
  });
}

/** Presses a button by the words on it, or by its accessible name. */
export async function pressOnly(name: string): Promise<void> {
  const labelled = await $(`button[aria-label="${name}"]`);
  if (await labelled.isExisting()) {
    await labelled.click();
    return;
  }
  const button = await $(`button=${name}`);
  await button.waitForClickable({ timeout: 20_000 });
  await button.click();
}

/** Presses a button that starts a chat, and answers the picker it opens. */
export async function pressAndStart(name: string): Promise<void> {
  await pressOnly(name);
  await pickAndStart();
}

/**
 * Ending a chat, as the operator does it since the pane controls landed: press, then answer
 * **Close** — the dialog's answer that ends it (ADR 0064 added Smart close beside it).
 *
 * **Nothing ends a chat without asking** — the operator's *"closing session should ask
 * confirmation"*, and it is asked in one place (`PlaneView`'s `run`), so a tab's `×`, a
 * pane's `×` and the palette's rows all come through here. That is also what keeps the rule
 * from being quietly removed: take the question out and every spec below hangs waiting for a
 * dialog that never comes.
 *
 * It is `role="alertdialog"` rather than `dialog`: Radix's `AlertDialog`, which is the
 * primitive for a question the operator did not go looking for.
 */
export async function answerTheAsk(ending: string): Promise<void> {
  // The question is about the row that was pressed; its title says which. Looked for by that,
  // afresh at every look: `endEveryChat` asks one after another, and a node held from the last
  // question is not this one (#2126).
  await browser.waitUntil(
    async () => browser.execute(aDialogOffers, "alertdialog", ending, "Close", false),
    { timeout: 20_000, timeoutMsg: `the question about ${ending} was never asked` },
  );
  const answer = await $('[role="alertdialog"]').$("button=Close");
  await answer.waitForClickable({ timeout: 20_000 });
  await answer.click();
  await theDialogHasGone("alertdialog", ending, `the question about ${ending} never went`);
}

/** Ends the chat a control named `name` belongs to, and answers the question it asks. */
export async function endChat(name: string): Promise<void> {
  await pressOnly(name);
  await answerTheAsk(name);
}

/**
 * Ends every chat the strip is drawing, and the ones it is not.
 *
 * **The strip collapses rather than scrolling** (ADR 0039, as amended), so at fifty
 * chats it draws a handful and hides the rest. This terminates because closing a drawn tab
 * gives the strip room for a hidden one — the hidden tabs flow onto the strip as the drawn
 * ones go — and it is bounded so that a close which stops taking is a failure and not a hang.
 */
export async function endEveryChat(most = 200): Promise<void> {
  for (let pressed = 0; pressed < most; pressed++) {
    const closer = await $('button[aria-label^="End chat "]');
    if (!(await closer.isExisting())) return;
    const name = (await closer.getAttribute("aria-label")) ?? "";
    await closer.click();
    await answerTheAsk(name);
  }
  throw new Error(`${most} presses did not end every chat`);
}

/**
 * Closes a project by its `×` (a selector for it), and answers the question it asks when it has chats open
 * (`ClosingProject`, charter-app#239). A project with nothing open closes without one, so the
 * question is waited for briefly and not required.
 */
export async function closeProject(closer: string): Promise<void> {
  await $(closer).click();
  // Looked for afresh, by its answer, at every look (#2126).
  const asked = await browser
    .waitUntil(
      async () => browser.execute(aDialogOffers, "alertdialog", "", "Close and end", true),
      { timeout: 3_000 },
    )
    .then(() => true)
    .catch(() => false);
  if (asked) {
    const answer = await $('[role="alertdialog"]').$("button*=Close and end");
    await answer.waitForClickable({ timeout: 20_000 });
    await answer.click();
    await theDialogHasGone("alertdialog", "", "the question about closing the project never went");
  }
  await everyCloseHasLetGo();
}

/**
 * Waits until every close this window asked has let go of its project (#1949): the tab goes
 * as the close is asked, and the core ends its chats behind the window, one close at a time.
 * The next spec then starts on a core with nothing of a close still under way.
 */
export async function everyCloseHasLetGo(): Promise<void> {
  await browser.waitUntil(
    async () => (await $('[data-strip="Projects"]').getAttribute("data-letting-go")) === "0",
    {
      timeout: 60_000,
      timeoutMsg: "a project this window closed never finished letting go",
    },
  );
}

/** The trust question's title for a project this machine has approved nothing about (ADR 0035). */
export const FIRST_ASK = "Open this project?";

/**
 * Whether the page shows a dialog titled `title`, looked for afresh. **Run in the page**, by
 * `browser.execute`, so it closes over nothing: what it needs is its argument and `document`.
 *
 * A dialog's title is what its `aria-labelledby` names, as a Radix dialog draws it. A dialog
 * mid-fade is shown once its opacity is past zero; `checkVisibility` says so where the engine has
 * it.
 */
export function showsTheQuestion(title: string): boolean {
  for (const dialog of document.querySelectorAll('[role="dialog"]')) {
    const named = (dialog.getAttribute("aria-labelledby") ?? "")
      .split(" ")
      .map((id) => document.getElementById(id)?.textContent?.trim() ?? "")
      .join(" ");
    if (named !== title) continue;
    const seen = dialog as Element & { checkVisibility?: (options?: object) => boolean };
    if (
      typeof seen.checkVisibility !== "function" ||
      seen.checkVisibility({ opacityProperty: true, visibilityProperty: true })
    )
      return true;
  }
  return false;
}

/**
 * Waits for the trust question and answers it **Open project** (ADR 0035): the end of every
 * path that opens a project this machine has approved nothing about.
 *
 * **Waited for by its words, read again at every look** (#2126). Making a project ends in two
 * dialogs, one in the other's place: New project, still up while it says "Creating…", then this
 * question. A wait that took `$('[role="dialog"]')` when it began could take the first and keep
 * asking about it after it had gone: the driver inside the app hands a script a node that has
 * left the page without calling it stale, so WebdriverIO never looks for the selector again, and
 * the spec waited out 30 s with the question on screen. A look done by one `browser.execute`
 * holds no node (`reading.ts` is the same lesson for lists).
 */
export async function openThroughTheTrustQuestion({
  within = 30_000,
  ifAsked = false,
}: {
  /** How long the question is waited for. */
  within?: number;
  /** A project this machine may already have approved: no question is not a failure. */
  ifAsked?: boolean;
} = {}): Promise<void> {
  const asked = await browser
    .waitUntil(async () => browser.execute(showsTheQuestion, FIRST_ASK), {
      timeout: within,
      timeoutMsg: `the trust question ("${FIRST_ASK}") was never drawn`,
    })
    .then(() => true)
    .catch((why: unknown) => {
      if (ifAsked) return false;
      throw why;
    });
  if (!asked) return;
  // Found in the question, and only now that it is up: the opener's path box has an Open project
  // of its own.
  await $('[role="dialog"][aria-labelledby="approve-plane"]').$("button=Open project").click();
}

/**
 * Puts away whatever dialog a spec left up, so the next one starts on the window itself and not
 * behind a modal: Escape, a few times, while one is shown.
 */
export async function noDialogIsLeftUp(): Promise<void> {
  for (let press = 0; press < 3; press++) {
    const shown = await $('[role="dialog"], [role="alertdialog"]')
      .isDisplayed()
      .catch(() => false);
    if (!shown) return;
    await browser.keys("Escape");
    await browser.pause(200);
  }
}

/**
 * **Shows a view of a side** (#1673, #1678): presses its icon on the activity bar unless it is
 * already the open one, since a press of the open view puts the side away. The left side opens
 * on Chats, the right on the Inbox, and the Panel's right half on Memory (#1738), so a spec about
 * the explorer's rows shows the Explorer first and puts Chats back after, leaving the window as
 * the next spec expects it; a spec about another right-hand panel puts Memory back the same way.
 * A view in a half of the Panel is pressed on its Lower icon, wherever it lives (#1738, #1739):
 * the left half opens on Repos, so a spec about Changes puts Repos back after.
 */
export async function showView(
  name:
    | "Chats"
    | "Explorer"
    | "Search"
    | "Changes"
    | "Repos"
    | "Inbox"
    | "Todos"
    | "Memory"
    | "Personas"
    | "Sessions"
    | "Vaults",
): Promise<void> {
  // On whichever bar holds it: a sidebar's, or the Lower icons of its half of the Panel.
  const tab = await $(`.activity-bar [role="tab"][aria-label="${name}"]`);
  await tab.waitForExist({ timeout: 30_000 });
  if ((await tab.getAttribute("aria-selected")) !== "true") await tab.click();
  await browser.waitUntil(async () => (await tab.getAttribute("aria-selected")) === "true", {
    timeout: 10_000,
    timeoutMsg: `the ${name} view never opened`,
  });
}
