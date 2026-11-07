// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import {
  MISSED_PRESS_CHARS,
  aMissedPress,
  coveredIn,
  forgetTheMiss,
  rememberTheMiss,
  theMissBehind,
} from "./covers";

/**
 * **A test that could not press or see an element says what covered it** (`covers.ts`): the
 * element's selector is read out of WebdriverIO's message, and only for those two failures.
 */
describe("a failure about an element it could not reach", () => {
  it("names the element's selector, for a press or a sight that never came", () => {
    expect(coveredIn('element ("button=New tab") still not clickable after 20000ms')).toBe(
      "button=New tab",
    );
    expect(coveredIn('element ("[role="dialog"]") still not displayed after 30000ms')).toBe(
      '[role="dialog"]',
    );
  });

  it("names none for any other failure", () => {
    expect(coveredIn("expect(received).toBe(expected)")).toBeUndefined();
    expect(coveredIn('element ("#x") still not existing after 20000ms')).toBeUndefined();
  });
});

/**
 * **A press that missed says, as it missed, what it was pressing** (#5609): the target's box,
 * what sits at its centre, whether it is disabled, out of view or still animating, and an
 * outline of what is drawn around it. Asked of the page by `aMissedPress`, which
 * `describeEveryMissedPress` runs the moment the press fails, before any `after` hook puts the
 * page away. jsdom draws nothing, so each test draws the boxes and what is at a point itself.
 */
describe("a press that missed", () => {
  afterEach(() => {
    document.body.replaceChildren();
    delete (document as { elementFromPoint?: unknown }).elementFromPoint;
    delete (document as { getAnimations?: unknown }).getAnimations;
  });

  /** The close question, its answer drawn at 100,50, 120 wide and 32 high. */
  function theCloseQuestion(): { dialog: HTMLElement; answer: HTMLButtonElement } {
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "alertdialog");
    dialog.setAttribute("aria-label", "Close this project?");
    const heading = document.createElement("h2");
    heading.textContent = "Close this project?";
    const keep = document.createElement("button");
    keep.textContent = "Keep it open";
    const answer = document.createElement("button");
    answer.textContent = "Close and end 2 chats";
    answer.getBoundingClientRect = () => new DOMRect(100, 50, 120, 32);
    dialog.append(heading, keep, answer);
    document.body.append(dialog);
    return { dialog, answer };
  }

  /** Says `at` is what is drawn at every point. */
  function drawnAtEveryPoint(at: Element | null): { x: number; y: number }[] {
    const asked: { x: number; y: number }[] = [];
    (document as { elementFromPoint?: unknown }).elementFromPoint = (x: number, y: number) => {
      asked.push({ x, y });
      return at;
    };
    return asked;
  }

  it("names its box and what lay over its centre", () => {
    const { answer } = theCloseQuestion();
    const veil = document.createElement("div");
    veil.className = "fixed inset-0 bg-overlay";
    veil.setAttribute("data-state", "closed");
    document.body.append(veil);
    const asked = drawnAtEveryPoint(veil);

    const said = aMissedPress(answer);

    expect(asked).toEqual([{ x: 160, y: 66 }]);
    expect(said).toContain("the element: button “Close and end 2 chats”, 120×32 at 100,50");
    expect(said).toContain("at its centre: div.fixed.inset-0.bg-overlay [data-state=closed]");
  });

  it("says the press would have landed on itself when nothing covered it", () => {
    const { answer } = theCloseQuestion();
    drawnAtEveryPoint(answer);

    expect(aMissedPress(answer)).toContain("at its centre: itself");
  });

  it("finds its target by WebdriverIO's selector, and says when it is not in the page", () => {
    theCloseQuestion();
    drawnAtEveryPoint(null);

    expect(aMissedPress("button*=Close and end")).toContain(
      "the element: button “Close and end 2 chats”, 120×32 at 100,50",
    );
    const missing = aMissedPress("button*=Keep it closed");
    expect(missing).toContain("the element: not in the page");
    expect(missing.join("\n")).not.toContain("at its centre");
  });

  it("finds a chained selector's target inside what the chain found first", () => {
    const elsewhere = document.createElement("button");
    elsewhere.textContent = "Close and end the other";
    document.body.append(elsewhere);
    theCloseQuestion();
    drawnAtEveryPoint(null);

    expect(aMissedPress(['[role="alertdialog"]', "button*=Close and end"])).toContain(
      "the element: button “Close and end 2 chats”, 120×32 at 100,50",
    );
  });

  it("keeps within its budget, cutting the outline first", () => {
    const { dialog, answer } = theCloseQuestion();
    for (let n = 0; n < 40; n++) {
      const row = document.createElement("div");
      row.id = `row-${n}-${"x".repeat(80)}`;
      row.className = `${"a".repeat(60)} ${"b".repeat(60)} ${"c".repeat(60)}`;
      dialog.insertBefore(row, answer);
    }
    drawnAtEveryPoint(answer);

    const said = aMissedPress(answer);

    expect(said.join("\n").length).toBeLessThanOrEqual(MISSED_PRESS_CHARS);
    expect(said[0]).toContain("the element: button “Close and end 2 chats”");
    expect(said).toContain("at its centre: itself");
    expect(said.at(-1)).toMatch(/^ {2}… and \d+ more$/);
  });

  it("says what disables it: its own attribute, or an inert ancestor", () => {
    const { dialog, answer } = theCloseQuestion();
    drawnAtEveryPoint(answer);
    expect(aMissedPress(answer)).toContain("disabled: no");

    answer.setAttribute("aria-disabled", "true");
    expect(aMissedPress(answer)).toContain("disabled: yes, aria-disabled");

    answer.removeAttribute("aria-disabled");
    dialog.setAttribute("inert", "");
    expect(aMissedPress(answer)).toContain(
      'disabled: yes, inert from div [role=alertdialog] "Close this project?"',
    );
  });

  it("says what keeps the pointer off it: pointer-events, visibility or opacity", () => {
    const { dialog, answer } = theCloseQuestion();
    drawnAtEveryPoint(answer);
    expect(aMissedPress(answer)).toContain("reachable: yes");

    dialog.style.pointerEvents = "none";
    answer.style.opacity = "0";
    expect(aMissedPress(answer)).toContain(
      'reachable: no, opacity 0 on button “Close and end 2 chats”; pointer-events none on div [role=alertdialog] "Close this project?"',
    );
  });

  it("names an animation still running on it or on what holds it", () => {
    const { dialog, answer } = theCloseQuestion();
    drawnAtEveryPoint(answer);
    const elsewhere = document.createElement("div");
    document.body.append(elsewhere);
    (document as { getAnimations?: unknown }).getAnimations = () => [
      {
        animationName: "dialog-in",
        playState: "running",
        currentTime: 80,
        effect: { target: dialog, getTiming: () => ({ duration: 150 }) },
      },
      {
        animationName: "spin",
        playState: "running",
        currentTime: 5,
        effect: { target: elsewhere, getTiming: () => ({ duration: 1000 }) },
      },
    ];

    expect(aMissedPress(answer)).toContain(
      'animating: dialog-in on div [role=alertdialog] "Close this project?", running, 80 of 150 ms',
    );
  });

  it("says when nothing animates", () => {
    const { answer } = theCloseQuestion();
    drawnAtEveryPoint(answer);
    (document as { getAnimations?: unknown }).getAnimations = () => [];

    expect(aMissedPress(answer)).toContain("animating: nothing");
  });

  it("says no words of a control that holds a field", () => {
    const { dialog, answer } = theCloseQuestion();
    const label = document.createElement("label");
    label.append("Note ");
    const note = document.createElement("textarea");
    note.textContent = "the password is hunter2";
    label.append(note);
    dialog.insertBefore(label, answer);
    drawnAtEveryPoint(label);

    const said = aMissedPress(answer).join("\n");

    expect(said).toContain("at its centre: label\n");
    expect(said).not.toContain("hunter2");
  });

  it("outlines what is drawn around it, marking it, with no field's value or text", () => {
    const { dialog, answer } = theCloseQuestion();
    const name = document.createElement("input");
    name.setAttribute("aria-label", "Token");
    name.value = "a-private-value-0451";
    const note = document.createElement("textarea");
    note.textContent = "the password is hunter2";
    dialog.insertBefore(name, answer);
    dialog.insertBefore(note, answer);
    drawnAtEveryPoint(answer);

    const said = aMissedPress(answer).join("\n");

    expect(said).toContain(
      [
        "around it:",
        '  div [role=alertdialog] "Close this project?"',
        "    h2 “Close this project?”",
        "    button “Keep it open”",
        '    input "Token"',
        "    textarea",
        "    button “Close and end 2 chats” ← the element",
      ].join("\n"),
    );
    expect(said).not.toContain("private-value");
    expect(said).not.toContain("hunter2");
  });
});

/**
 * **The evidence taken as a press missed is the evidence its failure carries** (#5609): an
 * `after` hook that puts a question away before it throws would otherwise leave the record to
 * describe a page with no question in it.
 */
describe("the miss a failure carries", () => {
  it("is the one taken for its own message, handed over once", () => {
    rememberTheMiss('element ("button*=Close and end") still not clickable after 20000ms', [
      "at its centre: div.veil",
    ]);

    expect(theMissBehind("expect(received).toBe(expected)")).toBeUndefined();
    expect(
      theMissBehind('element ("button*=Close and end") still not clickable after 20000ms'),
    ).toEqual(["at its centre: div.veil"]);
    expect(
      theMissBehind('element ("button*=Close and end") still not clickable after 20000ms'),
    ).toBeUndefined();
  });

  it("is forgotten as a test begins, so a miss a spec caught is not another's", () => {
    rememberTheMiss('element ("#a") still not displayed after 3000ms', ["caught"]);
    forgetTheMiss();

    expect(theMissBehind('element ("#a") still not displayed after 3000ms')).toBeUndefined();
  });

  it("is only the latest, so an earlier miss a spec caught is not mistaken for it", () => {
    rememberTheMiss('element ("#a") still not displayed after 3000ms', ["first"]);
    rememberTheMiss('element ("#b") still not clickable after 20000ms', ["second"]);

    expect(theMissBehind('element ("#a") still not displayed after 3000ms')).toBeUndefined();
  });
});
