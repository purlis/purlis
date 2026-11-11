// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { basename } from "node:path";
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
 *
 * `within` is how long the picker may take to offer its button. Twenty seconds holds every
 * spec; the stress run's two hundred chats pass more (#5586, #5615).
 *
 * `withoutTheSandbox` ticks the picker's {@link START_WITHOUT_THE_SANDBOX} first, for this one
 * chat, and presses the start that box turns the button to.
 */
export async function pickAndStart(within = 20_000, withoutTheSandbox = false): Promise<void> {
  // The picker can take the palette's place, so it is looked for by its buttons at every look,
  // and nothing found before it is held (#2126).
  let pressing = "";
  let instead: string | null = null;
  const words = withoutTheSandbox ? STARTS_WITHOUT_THE_SANDBOX : STARTS;
  await browser.waitUntil(
    async () => {
      // The box is ticked once the rows' details are in, and the press waits for the words it
      // turns the button to: a tick is a render away from what the button says.
      if (withoutTheSandbox) await browser.execute(tickStartWithoutTheSandbox, PICKER);
      // Not while the rows' details are read (#4736): until they arrive the button says Start
      // chat even for a profile that asks for its approval, and a press then shows the approval
      // instead of going, so it is chosen once the picker knows which it is. In one look, so the
      // dialog cannot be drawn between "is it checking?" and "which button?" (#4979).
      pressing = (await browser.execute(whatThePickerOffers, PICKER, words)) ?? "";
      if (pressing !== "") return true;
      // A project where nothing can start draws no picker at all (#5580): said at once.
      instead = await browser.execute(whatCameInsteadOfThePicker);
      return instead !== null;
    },
    { timeout: within, timeoutMsg: "the picker never offered Start" },
  );
  if (instead !== null)
    throw new Error(
      `the picker was never drawn: nothing can start in the project in front, so New chat ` +
        `opened ${instead} in its place (FR-29). A spec before this one may have left the ` +
        `window on that project (#5580).`,
    );
  const start = await $(`button=${pressing}`);
  // `within` covers the press and the picker going too: two hundred chats in, each step of an
  // open takes about twenty seconds on macOS (#5615), and a twenty-second wait here failed
  // main's stress run at chat 182 (#5586).
  await start.waitForClickable({ timeout: within });
  await start.click();
  await theDialogHasGone("dialog", PICKER, "the picker never went", within);
}

/** The picker's title (`StartChat`). */
const PICKER = "Start a chat";

/** The picker's box for starting this one chat without the sandbox (ADR 0067 §7, ruling V78 a). */
export const START_WITHOUT_THE_SANDBOX = "Start without the sandbox";

/** What the picker's start button says for a sandboxed chat: an approval first, or a start. */
export const STARTS: readonly string[] = ["Approve and start", "Start chat"];

/** What it says once {@link START_WITHOUT_THE_SANDBOX} is ticked (`StartChatApproval.ts`). */
export const STARTS_WITHOUT_THE_SANDBOX: readonly string[] = [
  "Approve and start without the sandbox",
  START_WITHOUT_THE_SANDBOX,
];

/**
 * Ticks the {@link START_WITHOUT_THE_SANDBOX} box of the shown dialog saying `text`, once its rows'
 * details are in, and answers whether it is ticked. **Run in the page**, as
 * {@link whatThePickerOffers} is. The box is the checkbox family's (#2907): a native checkbox in
 * its own label, ticked by a click on the input itself, which React Aria reads as a virtual press.
 *
 * On Linux, where every sandboxed Claude Code chat runs inside purlis's wrap, on a sandbox-only
 * profile (ADR 0067 as amended 2026-11-10), a chat on a profile a project declares starts this way.
 */
export function tickStartWithoutTheSandbox(text: string): boolean {
  for (const dialog of document.querySelectorAll('[role="dialog"]')) {
    if (!(dialog.textContent ?? "").includes(text)) continue;
    if (dialog.querySelector('[aria-busy="true"]') !== null) return false;
    for (const box of dialog.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')) {
      if (!(box.closest("label")?.textContent ?? "").includes("Start without the sandbox"))
        continue;
      if (!box.checked) box.click();
      return box.checked;
    }
  }
  return false;
}

/**
 * The words to press in a shown dialog saying `text`, of `words` (Approve and start or Start chat,
 * unless it is told {@link STARTS_WITHOUT_THE_SANDBOX}), or `null`
 * while there is none to press: no such dialog, or one still reading its rows' details, which it
 * marks `aria-busy` (#4736). **One look, run in the page**, as {@link aDialogOffers} is: asked in
 * two calls, "is it checking?" and "which button?", the picker could be drawn between them, and
 * its Start chat was pressed before it knew whether the profile asks for its approval (#4979).
 */
export function whatThePickerOffers(
  text: string,
  words: readonly string[] = ["Approve and start", "Start chat"],
): string | null {
  for (const dialog of document.querySelectorAll('[role="dialog"]')) {
    if (!(dialog.textContent ?? "").includes(text)) continue;
    if (dialog.querySelector('[aria-busy="true"]') !== null) return null;
    const seen = dialog as Element & { checkVisibility?: (options?: object) => boolean };
    if (
      typeof seen.checkVisibility === "function" &&
      !seen.checkVisibility({ opacityProperty: true, visibilityProperty: true })
    )
      continue;
    const said = [...dialog.querySelectorAll("button")].map((one) =>
      (one.textContent ?? "").trim(),
    );
    for (const one of words) if (said.includes(one)) return one;
  }
  return null;
}

/**
 * The name of the Set up a harness tab when it is the tab in front, or `null`. **Run in the
 * page.** In a project where no harness can start, New chat opens that tab and never draws the
 * picker (FR-29, `PlaneView`'s `ask`), so a spec waiting on the picker there waits for nothing
 * (#5580). Only while no picker is up: from that tab, its own Start a chat draws one.
 */
export function whatCameInsteadOfThePicker(): string | null {
  const pickerUp = [...document.querySelectorAll('[role="dialog"]')].some((dialog) =>
    (dialog.textContent ?? "").includes("Start a chat"),
  );
  if (pickerUp) return null;
  const front = document
    .querySelector('[data-strip="Tabs"] [role="tab"][aria-selected="true"]')
    ?.querySelector(".tab-name")
    ?.textContent?.trim();
  return front?.startsWith("Set up a harness") ? front : null;
}

/**
 * Whether a dialog saying `text` is still reading its rows' details (#4736): it marks them
 * `aria-busy` until they arrive. **Run in the page**, as {@link aDialogOffers} is.
 */
export function thePickerIsChecking(text: string): boolean {
  return [...document.querySelectorAll('[role="dialog"]')].some(
    (dialog) =>
      (dialog.textContent ?? "").includes(text) &&
      dialog.querySelector('[aria-busy="true"]') !== null,
  );
}

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
async function theDialogHasGone(
  role: string,
  text: string,
  timeoutMsg: string,
  within = 20_000,
): Promise<void> {
  try {
    await browser.waitUntil(
      async () => !(await browser.execute(aDialogOffers, role, text, "Cancel", false)),
      { timeout: within },
    );
  } catch {
    // Said with what the dialog reads, so a red run says why it stayed.
    const says = await browser.execute(
      (wanted: string) =>
        [...document.querySelectorAll(`[role="${wanted}"]`)]
          .map((one) => (one.textContent ?? "").replace(/\s+/g, " ").trim())
          .join(" | ")
          .slice(-1500),
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
    return group ? group.querySelectorAll('input[type="radio"]').length : 0;
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
    // A choice's input is in its own label (#2907); a row's box is named by its `<label for>`.
    const labelled =
      (on.id ? document.querySelector(`label[for="${CSS.escape(on.id)}"]`) : null) ??
      (on instanceof HTMLInputElement ? on.closest("label") : null);
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

/**
 * Presses a button that starts a chat, and answers the picker it opens.
 *
 * **"New tab" is the strip's `+`, which opens a tab of one empty pane** (#4758): no chat, and no
 * picker. A spec that asks for it wants its chat in a tab of its own, so it is what the person
 * does for that: `+`, then the empty pane's own New chat, which opens the picker for that pane.
 */
export async function pressAndStart(name: string, within?: number): Promise<void> {
  await pressOnly(name);
  if (name === NEW_TAB) await pressTheEmptyPanesNewChat();
  await pickAndStart(within);
}

/** **Opens the picker for a chat in a tab of its own**: the strip's `+`, then the new tab's
 *  empty pane's New chat (#4758), and nothing picked. */
export async function openThePicker(): Promise<void> {
  await pressOnly(NEW_TAB);
  await pressTheEmptyPanesNewChat();
}

/** The strip's `+` (`tab.new`). */
const NEW_TAB = "New tab";

/** Presses New chat on the empty pane drawn in front: the one a `+` just opened. */
async function pressTheEmptyPanesNewChat(): Promise<void> {
  await browser.waitUntil(
    async () =>
      browser.execute(() => {
        const pane = [...document.querySelectorAll<HTMLElement>('[data-testid="empty-pane"]')].find(
          (one) => one.getBoundingClientRect().width > 0,
        );
        const button = [...(pane?.querySelectorAll<HTMLElement>("button") ?? [])].find(
          (one) => (one.textContent ?? "").trim() === "New chat",
        );
        button?.click();
        return button !== undefined;
      }),
    { timeout: 20_000, interval: 200, timeoutMsg: "the new tab drew no empty pane to start in" },
  );
}

/**
 * Ending a chat, as the operator does it since the pane controls landed: press, then answer
 * **Close** — the dialog's answer that ends it (ADR 0064 added Smart close beside it), or
 * **End** on a shell, whose question has only Cancel and End (#4761).
 *
 * **Nothing ends a chat without asking** — the operator's *"closing session should ask
 * confirmation"*, and it is asked in one place (`PlaneView`'s `run`), so a tab's `×`, a
 * pane's `×` and the palette's rows all come through here. That is also what keeps the rule
 * from being quietly removed: take the question out and every spec below hangs waiting for a
 * dialog that never comes.
 *
 * It is `role="alertdialog"` rather than `dialog`: the dialog family's `AlertDialog`
 * (`ui/dialog/Dialog.tsx`), which is the one for a question the operator did not go looking for.
 */
export async function answerTheAsk(ending: string): Promise<void> {
  // The question is about the row that was pressed; its title says which. Looked for by that,
  // afresh at every look: `endEveryChat` asks one after another, and a node held from the last
  // question is not this one (#2126).
  // A chat's question ends it on Close; a shell's asks only Cancel or End (#4761).
  let word = "Close";
  await browser.waitUntil(
    async () => {
      for (const one of ["Close", "End"])
        if (await browser.execute(aDialogOffers, "alertdialog", ending, one, false)) {
          word = one;
          return true;
        }
      return false;
    },
    { timeout: 20_000, timeoutMsg: `the question about ${ending} was never asked` },
  );
  // Found and pressed in one look in the page (#6227): a button held from a look before is one
  // the question may have drawn again since.
  await browser.waitUntil(async () => browser.execute(pressTheAnswer, word), {
    timeout: 20_000,
    interval: 200,
    timeoutMsg: `the question about ${ending} never offered ${word}`,
  });
  await theDialogHasGone("alertdialog", ending, `the question about ${ending} never went`);
}

/**
 * **Opens the palette**, and makes sure it is up: F2, and where the driver's F2 reaches nothing
 * that opens it (the keyboard left on something that takes the key, such as a terminal or a
 * dialog that just closed), the same key dispatched on the window, where the palette listens,
 * as `reach.ts`'s `runFromThePalette` does.
 */
async function openThePalette(): Promise<void> {
  const palette = await $(PALETTE);
  if (await palette.isDisplayed().catch(() => false)) return;
  await browser.keys(["F2"]);
  const opened = await palette
    .waitForDisplayed({ timeout: 3_000 })
    .then(() => true)
    .catch(() => false);
  if (!opened) {
    await browser.execute(() => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "F2", bubbles: true, cancelable: true }),
      );
    });
    await palette.waitForDisplayed({ timeout: 20_000 });
  }
  await $("#palette-query").waitForDisplayed({ timeout: 10_000 });
}

/**
 * Ends every chat of the project in front, on a strip or in no pane at all: each one's End…
 * (#4753), the one row that ends a chat, since a close ends nothing (#4756). Run from the
 * palette, which lists an End… for every chat, one at a time, and bounded so that an End which
 * stops taking is a failure and not a hang. Then the panes End… left on the ended band are
 * cleared (#5019).
 */
export async function endEveryChat(most = 200): Promise<void> {
  for (let pressed = 0; pressed < most; pressed++) {
    await openThePalette();
    await $("#palette-query").addValue("End chat");
    const row = await browser.execute(() => {
      const found = document.querySelector<HTMLElement>(
        '[id^="palette-row-chat.end:"]:not([aria-disabled="true"])',
      );
      found?.click();
      return found?.id ?? null;
    });
    if (row === null) {
      await browser.keys(["Escape"]);
      await $(PALETTE).waitForDisplayed({ reverse: true, timeout: 10_000 });
      await clearEndedPanes(undefined, most);
      return;
    }
    await answerTheAsk("");
  }
  throw new Error(`${most} presses did not end every chat`);
}

/** **An ended chat's band** (#4756, #5019), whichever chat's: the pane Notice its cause names. */
export const ENDED_BAND = '[data-cause^="chat-ended:"]';

/**
 * **Clears the panes End… left on the ended band** (#5019): each ended chat's pane stays, its
 * last screen under the band, until Clear pane takes it away, as a person does. Every tab of the
 * strip is looked at in turn, since a band is drawn only in the tab in front, until a whole pass
 * finds none: chat `session`'s alone where one is named, and every one where none is. Bounded,
 * so a Clear pane that stops taking is a failure and not a hang.
 */
export async function clearEndedPanes(session?: number, most = 200): Promise<void> {
  const cause = session === undefined ? ENDED_BAND : `[data-cause="chat-ended:${session}"]`;
  /** Presses Clear pane on a band drawn now; whether there was one. */
  const clearedOne = () => browser.execute(pressClearPane, cause);
  let cleared = 0;
  for (let pass = 0; pass < most; pass++) {
    let found = false;
    const tabs = await browser.execute(
      () => document.querySelectorAll('[data-strip="Tabs"] [role="tab"]').length,
    );
    for (let at = 0; at <= tabs; at++) {
      if (await clearedOne()) {
        found = true;
        if (++cleared >= most) throw new Error(`${most} presses did not clear every ended pane`);
        await browser.pause(100);
        break;
      }
      if (at === tabs) break;
      await browser.execute((index: number) => {
        document.querySelectorAll<HTMLElement>('[data-strip="Tabs"] [role="tab"]')[index]?.click();
      }, at);
      await browser.pause(100);
    }
    if (!found) return;
  }
}

/** Whether `one` takes a press: neither `disabled` nor `aria-disabled`. Run in the page. */
function pressable(one: Element): boolean {
  return !one.hasAttribute("disabled") && one.getAttribute("aria-disabled") !== "true";
}

/**
 * **Presses Clear pane on the first band `cause` names that offers it** (#4756, #6227), and
 * answers whether it did. **Run in the page**, so the band is the one drawn now: clearing one
 * pane of a split draws the tab again, and the other pane's band with it, and a band WebdriverIO
 * had found 100 ms before was a node no longer in the window.
 */
export function pressClearPane(cause: string): boolean {
  for (const band of document.querySelectorAll(cause)) {
    const clear = [...band.querySelectorAll("button")].find(
      (one) =>
        pressable(one) &&
        [one.getAttribute("aria-label"), one.textContent].some(
          (said) => (said ?? "").trim() === "Clear pane",
        ),
    );
    if (clear === undefined) continue;
    clear.click();
    return true;
  }
  return false;
}

/**
 * **Presses the button of a question (an `alertdialog`) whose words are `word`** (#6227), and
 * answers whether one was there to press. **Run in the page**, as {@link aDialogOffers} is: a
 * button held from a look before is one the question may have drawn again since.
 */
export function pressTheAnswer(word: string): boolean {
  for (const dialog of document.querySelectorAll('[role="alertdialog"]')) {
    const answer = [...dialog.querySelectorAll("button")].find(
      (one) => pressable(one) && (one.textContent ?? "").trim() === word,
    );
    if (answer === undefined) continue;
    answer.click();
    return true;
  }
  return false;
}

/**
 * **Presses the close question's main answer, whatever it says by now** (#5609), and answers
 * with the words it pressed, or `""` when no close question offers one yet. **Run in the page**,
 * as {@link aDialogOffers} is, so each look finds the question afresh.
 */
export function pressTheCloseAnswer(): string {
  for (const dialog of document.querySelectorAll('[role="alertdialog"]')) {
    // Its title, either way: "Close project <name>?" or "Close project <name> and end …?".
    if (!(dialog.textContent ?? "").includes("Close project")) continue;
    const answer = [...dialog.querySelectorAll("button")].find((one) => {
      const said = (one.textContent ?? "").trim();
      return said === "Close project" || said.startsWith("Close and end");
    });
    if (answer === undefined || answer.disabled) continue;
    answer.click();
    return (answer.textContent ?? "").trim();
  }
  return "";
}

/**
 * Closes a project by its button on the rail (a selector for it), with Delete, the key that
 * closes a focused project (#2915: the rail has no `×`), and answers the question it asks when it
 * has chats open (`ClosingProject`, charter-app#239). A project with nothing open closes without
 * one, so the question is waited for briefly and not required.
 */
export async function closeProject(button: string): Promise<void> {
  await $(button).waitForExist({ timeout: 20_000 });
  await browser.execute((css: string) => {
    document.querySelector<HTMLElement>(css)?.focus();
  }, button);
  await browser.keys(["Delete"]);
  // Looked for afresh, by its title, at every look (#2126). Its main button's words are not
  // waited for: they count the project's chats as they are now, and a chat that ends while the
  // question is up turns "Close and end 1 chat" into "Close project" under the press (#5609).
  const asked = await browser
    .waitUntil(
      async () => browser.execute(aDialogOffers, "alertdialog", "Close project", "Cancel", false),
      { timeout: 3_000 },
    )
    .then(() => true)
    .catch(() => false);
  if (asked) {
    let pressed = "";
    await browser.waitUntil(
      async () => (pressed = await browser.execute(pressTheCloseAnswer)) !== "",
      {
        timeout: 20_000,
        interval: 250,
        timeoutMsg: "the question about closing the project offered no answer to press",
      },
    );
    await theDialogHasGone(
      "alertdialog",
      "Close project",
      `the question about closing the project never went, after a press of ${pressed}`,
    );
  }
  await everyCloseHasLetGo();
}

/**
 * **Closes the project at `plane` by its tab, for a spec's `after`, and hands back what failed
 * rather than throwing it** (#5580): an `after` that stops at a failed close leaves the project
 * open and in front for every spec after it, so it goes on to close what is left through the
 * core and throws this at its end. A question the press never answered is put away with Escape,
 * so no dialog outlives the spec. Nothing to do, and `undefined`, for `""` or a project with no
 * tab.
 */
export async function closingByTheWindow(plane: string): Promise<unknown> {
  if (plane === "") return undefined;
  // `switching.ts`'s PROJECTS, written out: that module imports this one.
  const closer = `[data-strip="Projects"] [role="tab"][aria-label="${basename(plane)}"]`;
  try {
    if (await $(closer).isExisting()) await closeProject(closer);
    return undefined;
  } catch (error) {
    for (let left = 0; left < 3; left++) {
      const up = await browser.execute(
        () => document.querySelector('[role="alertdialog"]') !== null,
      );
      if (!up) break;
      await browser.keys(["Escape"]);
      await browser.pause(200);
    }
    return error;
  }
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

/** The palette, once it is up. */
const PALETTE = '[role="dialog"][aria-label="Command palette"]';

/**
 * Runs a palette row by typing its words and Enter: how a scenario reaches what the window has
 * no button for, New project… among them since the project strip went (#2915).
 */
export async function fromThePalette(words: string): Promise<void> {
  await browser.keys(["F2"]);
  await $(PALETTE).waitForDisplayed({ timeout: 20_000 });
  const box = await $("#palette-query");
  await browser.waitUntil(async () => box.isFocused(), {
    timeout: 10_000,
    timeoutMsg: "the palette did not take the keyboard when it opened",
  });
  await box.addValue(words);
  await browser.keys(["Enter"]);
}

/** The trust question's title for a project this machine has approved nothing about (ADR 0035). */
export const FIRST_ASK = "Open this project?";

/**
 * Whether the page shows a dialog titled `title`, looked for afresh. **Run in the page**, by
 * `browser.execute`, so it closes over nothing: what it needs is its argument and `document`.
 *
 * A dialog's title is the heading its `aria-labelledby` names, as the dialog family draws it
 * (`ui/dialog/Dialog.tsx`). A dialog mid-fade is shown once its opacity is past zero;
 * `checkVisibility` says so where the engine has it.
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
 * on Chats, the right on Asks (ADR 0119), and the Panel's right half on Memory (#1738), so a spec about
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
    | "Asks"
    | "Updates"
    | "Notices"
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
