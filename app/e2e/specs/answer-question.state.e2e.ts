// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { $, browser, expect } from "@wdio/globals";
import { pressAndStart } from "../opening.js";
import { ask } from "../switching.js";
import { endTheChatsOpenedSince, projectInFront, sessionInFront, tabNames } from "../reach.js";

/**
 * **The person answers a task's question in the asking chat's Activity tab** (#1496, V100-46),
 * the form (`AnswerQuestion`) driven in the built app against the real core.
 *
 * A question a task is paused on needs a persona chat and a core that dispatched it, which the
 * scenario harness cannot make (`activity.e2e.ts` says the same of a timeline). So the timeline
 * is pretended (`src/e2eActivity.ts`), and **the question is put in the mouth of the real chat
 * this spec starts**: its line names that chat by the key the core knows it by, so Answer asks
 * the real core which session it has now, the form opens on it, and Send reaches the real
 * `answer_task_question`. That chat is no task, so the core refuses the answer in its own
 * sentence and delivers nothing: what is proved here is the whole path from the line to the
 * core and back, the focus, the empty refusal, the text kept through a refusal, and Cancel.
 * That an answer reaches a task that asked is the core's tests' (`dispatched.rs`).
 *
 * **It leaves the window as it found it**: the Activity tab is closed and the chat is ended.
 */

const STRIP = '[data-strip="Tabs"]';
const FORM = '[data-testid="answer-question"]';
const TASK = "probe";
const QUESTION = "Which queue should the probe read: the live one or the replay?";
const TYPED = "The replay, please.";

type Line = Record<string, unknown>;

/** One line as the core hands it over. */
function line(over: Line): Line {
  return {
    dispatch: "01K7E2E0000000000000ANSWER",
    n: 0,
    at: "2026-10-12T09:00:00+00:00",
    kind: "note",
    from: TASK,
    from_key: "",
    from_session: null,
    to: "its session",
    to_key: "01K7E2E0000000000000SESSN0",
    text: "",
    by_person: false,
    asks: null,
    answers: null,
    unread: false,
    by_purlis: false,
    expired: false,
    left_out: false,
    unkept: null,
    unkept_why: null,
    outcome: null,
    files: [],
    task: TASK,
    place: "project root\u0000.",
    depth: 1,
    ...over,
  };
}

/** Opens the Activity of the chat in the tab called `name`, from that tab's own menu. */
async function openActivityOf(name: string): Promise<void> {
  const sent = await browser.execute(
    (strip: string, called: string) => {
      const tab = [...document.querySelectorAll<HTMLElement>(`${strip} [role="tab"]`)].find(
        (one) => one.querySelector(".tab-name")?.textContent === called,
      );
      if (!tab) return false;
      const box = tab.getBoundingClientRect();
      tab.dispatchEvent(
        new MouseEvent("contextmenu", {
          bubbles: true,
          cancelable: true,
          clientX: Math.round(box.left + box.width / 2),
          clientY: Math.round(box.top + box.height / 2),
        }),
      );
      return true;
    },
    STRIP,
    name,
  );
  expect(sent).toBe(true);
  await browser.waitUntil(
    async () =>
      browser.execute(() => {
        const item = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
          (one) => one.getAttribute("aria-label") === "Activity",
        );
        item?.click();
        return item !== undefined;
      }),
    { timeout: 10_000, interval: 100, timeoutMsg: `${name}'s menu never offered Activity` },
  );
  await browser.waitUntil(
    async () =>
      browser.execute(
        () =>
          document.querySelector(
            '[data-testid="activity-empty"], [data-testid="activity-closed"], ol.activity',
          ) !== null,
      ),
    { timeout: 20_000, interval: 100, timeoutMsg: "the Activity tab never answered its read" },
  );
}

/** Hands the open Activity tab `lines`, as the core would answer its read. */
async function pretend(lines: Line[]): Promise<void> {
  await browser.execute((said: Line[]) => {
    window.dispatchEvent(
      new CustomEvent("purlis-e2e-activity", {
        detail: {
          name: "its session",
          key: "01K7E2E0000000000000SESSN0",
          lines: said,
          undrawn: 0,
          unlisted: 0,
          unread: 0,
          most_listed: 200,
          most_read: 2000,
        },
      }),
    );
  }, lines);
  await browser.waitUntil(
    async () =>
      browser.execute(
        (n: number) => document.querySelectorAll("ol.activity > li").length === n,
        lines.length,
      ),
    { timeout: 10_000, interval: 100, timeoutMsg: "the pretended timeline was never drawn" },
  );
}

/** Presses the element `selector` names, by its own click from the page. */
async function press(selector: string): Promise<void> {
  const found = await browser.execute((one: string) => {
    const element = document.querySelector<HTMLElement>(one);
    element?.click();
    return element !== null;
  }, selector);
  if (!found) throw new Error(`nothing to press at ${selector}`);
}

/** The form's own button that says `says`. */
const formButton = (says: string) =>
  browser.execute(
    (form: string, words: string) => {
      const button = [...document.querySelectorAll<HTMLElement>(`${form} button`)].find(
        (one) => one.textContent?.trim() === words,
      );
      button?.click();
      return button !== undefined;
    },
    FORM,
    says,
  );

/** What the form says is wrong, or nothing. */
const refusal = () =>
  browser.execute(
    (form: string) => document.querySelector(`${form} .answer-refused`)?.textContent ?? "",
    FORM,
  );

describe("answering a task's question from the Activity tab", function () {
  this.timeout(180_000);

  let wereAlreadyOpen: string[] = [];
  const answerOf = `button[aria-label="Answer ${TASK}'s question"]`;

  before(async () => {
    wereAlreadyOpen = await tabNames();
    await pressAndStart("New tab");
    const session = await sessionInFront();
    const [mine] = (await tabNames()).filter((tab) => !wereAlreadyOpen.includes(tab));

    // The key the core knows this chat by: its id, else its number (`activity::chat_now`).
    const plane = await projectInFront();
    const chats = await ask<{ session: number; id?: string | null }[]>("opened_chats", { plane });
    const id = chats.find((one) => one.session === session)?.id ?? null;
    const key = id ?? `#${session}`;

    await openActivityOf(mine);
    await pretend([
      line({
        n: 0,
        kind: "dispatched",
        text: "Read the probe's queue.",
        from: "its session",
        from_key: "01K7E2E0000000000000SESSN0",
        to: TASK,
        to_key: key,
      }),
      line({
        n: 1,
        kind: "question",
        at: "2026-10-12T09:01:00+00:00",
        text: QUESTION,
        from_key: key,
        from_session: session,
        asks: 1,
      }),
    ]);
  });

  after(async () => {
    await browser.execute((strip: string) => {
      for (const closer of document.querySelectorAll<HTMLElement>(
        `${strip} button[aria-label^="Close Activity"]`,
      ))
        closer.click();
    }, STRIP);
    await endTheChatsOpenedSince(wereAlreadyOpen);
  });

  it("offers Answer on a question its task is paused on, and opens the form with the keyboard in its box", async () => {
    await $(answerOf).waitForExist({ timeout: 10_000 });
    await press(answerOf);
    await $(FORM).waitForExist({ timeout: 10_000 });

    const form = await browser.execute((selector: string) => {
      const found = document.querySelector<HTMLElement>(selector);
      return {
        label: found?.getAttribute("aria-label") ?? "",
        asked: found?.querySelector(".answer-asked")?.textContent ?? "",
        typing: document.activeElement?.getAttribute("aria-label") ?? "",
      };
    }, FORM);
    expect(form.label).toBe(`Answer ${TASK}'s question`);
    // The question as text, whole.
    expect(form.asked).toBe(QUESTION);
    expect(form.typing).toBe("Your answer");
  });

  it("refuses an empty answer in a sentence and asks the core nothing", async () => {
    expect(await formButton("Send")).toBe(true);
    await browser.waitUntil(async () => (await refusal()) !== "", {
      timeout: 10_000,
      interval: 100,
      timeoutMsg: "an empty Send said nothing",
    });
    expect(await refusal()).toBe("The answer is empty, so nothing was sent.");
  });

  it("sends what was typed to the core, says its refusal as it says it, and keeps the text", async () => {
    const box = await $(`${FORM} textarea`);
    await box.addValue(TYPED);
    // Typing takes the last refusal down.
    await browser.waitUntil(async () => (await refusal()) === "", {
      timeout: 10_000,
      interval: 100,
      timeoutMsg: "typing never took the empty refusal down",
    });
    expect(await formButton("Send")).toBe(true);

    // This chat is no task: the core delivers nothing and says why, in its own words.
    await browser.waitUntil(async () => (await refusal()) !== "", {
      timeout: 20_000,
      interval: 100,
      timeoutMsg: "the core's answer to the Send was never said",
    });
    expect(await refusal()).toContain("so your answer was not sent");
    expect(await box.getValue()).toBe(TYPED);
    expect(await box.getAttribute("aria-invalid")).toBe("true");
  });

  it("puts the form away on Cancel, with Answer back and the keyboard on it", async () => {
    expect(await formButton("Cancel")).toBe(true);
    await browser.waitUntil(async () => !(await $(FORM).isExisting()), {
      timeout: 10_000,
      interval: 100,
      timeoutMsg: "Cancel left the form open",
    });
    await $(answerOf).waitForExist({ timeout: 10_000 });
    await browser.waitUntil(
      async () =>
        browser
          .execute(() => document.activeElement?.getAttribute("aria-label") ?? "")
          .then((on) => on === `Answer ${TASK}'s question`),
      { timeout: 10_000, interval: 100, timeoutMsg: "the keyboard never went back to Answer" },
    );
  });
});
