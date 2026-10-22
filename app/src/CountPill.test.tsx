// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { CountPill, countsSaid } from "./CountPill";
import { fitPill } from "./wholeWays";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

/**
 * **The one pill a tab's counts are drawn in**: how many chats are open there, and how many of
 * them need you, as two segments of one pill with one accessible name.
 *
 * What is drawn and what it is called is asked here. Where the pill sits, and that it gives way
 * before its tab's title, is asked in the real WebView (`strip-plus.e2e.ts`); the rule it gives
 * way by is asked below, on a tab laid out by hand, as `wholeWays.test.ts` does.
 */

describe("what a tab's counts are called", () => {
  it.each([
    [1, 0, "1 chat"],
    [3, 0, "3 chats"],
    [3, 1, "3 chats, 1 needs you"],
    [3, 2, "3 chats, 2 need you"],
    [1, 1, "1 chat, 1 needs you"],
  ])("%i open and %i asking is %s", (chats, needs, said) => {
    expect(countsSaid(chats, needs)).toBe(said);
  });

  it.each([
    [undefined, 1, "1 chat needs you"],
    [undefined, 4, "4 chats need you"],
    [0, 2, "2 chats need you"],
  ])("with no count of chats (%s) and %i asking is %s", (chats, needs, said) => {
    expect(countsSaid(chats, needs)).toBe(said);
  });
});

describe("the count pill", () => {
  it("draws nothing where no chat is open and none needs you", () => {
    const { container } = render(<CountPill chats={0} needs={0} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("draws the chats alone, in the neutral segment, where none needs you", () => {
    render(<CountPill chats={3} needs={0} />);
    const pill = screen.getByRole("img", { name: "3 chats" });
    expect(pill.querySelector(".count-pill-chats")?.textContent).toBe("3");
    expect(pill.querySelector(".count-pill-needs")).toBeNull();
  });

  it("draws both in one pill with one name, the second segment in the needs-you colour", () => {
    render(<CountPill chats={3} needs={1} />);
    const pill = screen.getByRole("img", { name: "3 chats, 1 needs you" });
    expect(screen.getAllByRole("img")).toHaveLength(1);
    expect(pill.querySelector(".count-pill-chats")?.textContent).toBe("3");
    expect(pill.querySelector(".count-pill-needs")?.textContent).toBe("1");
    // The digits are the pill's drawing: its one name is what a screen reader is told.
    for (const part of pill.querySelectorAll("span"))
      expect(part.getAttribute("aria-hidden")).toBe("true");
    expect(tipOf(pill)).toBe("3 chats, 1 needs you");
  });

  it("never hides a chat that needs you behind there being no tab to count", () => {
    render(<CountPill chats={0} needs={2} />);
    const pill = screen.getByRole("img", { name: "2 chats need you" });
    expect(pill.querySelector(".count-pill-chats")).toBeNull();
    expect(pill.querySelector(".count-pill-needs")?.textContent).toBe("2");
  });

  it("draws the needs-you segment alone on a surface that counts no chats", () => {
    render(<CountPill needs={1} />);
    const pill = screen.getByRole("img", { name: "1 chat needs you" });
    expect(pill.querySelector(".count-pill-chats")).toBeNull();
    expect(pill.dataset.needs).toBe("1");
  });

  it("says its counts as data, for a spec that measures it", () => {
    render(<CountPill chats={5} needs={2} />);
    const pill = screen.getByRole("img");
    expect(pill.dataset.chats).toBe("5");
    expect(pill.dataset.needs).toBe("2");
  });
});

/** A tab `width` wide, laid out by hand as a centred flex row lays it out: its padding, its
 *  title (which gives way first, to nothing), then its pill, which does not shrink. */
const PAD = 8;
const TITLE = 60;
const PILL = 30;
/** The window's text, and so the least of a title that still reads as one (2.5 of them). */
const TEXT = 13;

function tab(width: number, titled = true): { pill: HTMLElement; name: HTMLElement } {
  const button = document.createElement("button");
  button.setAttribute("role", "tab");
  const name = document.createElement("span");
  name.className = "workspace-name";
  name.textContent = "smart-ide";
  const pill = document.createElement("span");
  pill.className = "count-pill";
  if (titled) button.append(name);
  button.append(pill);
  document.body.append(button);

  const rect = (left: number, w: number) => ({ left, right: left + w, width: w }) as DOMRect;
  const pillWidth = () => (pill.dataset.away === undefined ? PILL : 0);
  const nameWidth = () =>
    titled ? Math.min(TITLE, Math.max(0, width - 2 * PAD - pillWidth())) : 0;
  button.getBoundingClientRect = () => rect(0, width);
  name.getBoundingClientRect = () => rect(PAD, nameWidth());
  Object.defineProperty(name, "scrollWidth", { configurable: true, get: () => TITLE });
  pill.getBoundingClientRect = () => rect(PAD + nameWidth(), pillWidth());
  name.style.fontSize = `${TEXT}px`;
  return { pill, name };
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("the pill on a tab, as the tab narrows", () => {
  it("is drawn where the title and the pill both fit", () => {
    const { pill } = tab(2 * PAD + TITLE + PILL);
    fitPill(pill);
    expect(pill.dataset.away).toBeUndefined();
  });

  it("stays while the title, cut, still reads as one", () => {
    const { pill } = tab(2 * PAD + 2.5 * TEXT + PILL + 1);
    fitPill(pill);
    expect(pill.dataset.away).toBeUndefined();
  });

  it("goes before the title is cut to less than that (D-1867-1)", () => {
    const { pill } = tab(2 * PAD + 2.5 * TEXT + PILL - 4);
    fitPill(pill);
    expect(pill.dataset.away).toBe("");
  });

  it("comes back with the room", () => {
    const narrow = tab(2 * PAD + PILL + 10);
    fitPill(narrow.pill);
    expect(narrow.pill.dataset.away).toBe("");

    document.body.replaceChildren();
    const wide = tab(2 * PAD + TITLE + PILL);
    wide.pill.dataset.away = "";
    fitPill(wide.pill);
    expect(wide.pill.dataset.away).toBeUndefined();
  });

  it("goes where it would run past the end of a tab with no title to give way", () => {
    const { pill } = tab(PAD + PILL - 2, false);
    fitPill(pill);
    expect(pill.dataset.away).toBe("");
  });

  it("is left alone on a tab that is not laid out, and outside a tab", () => {
    const { pill } = tab(0);
    fitPill(pill);
    expect(pill.dataset.away).toBeUndefined();

    const loose = document.createElement("span");
    document.body.append(loose);
    fitPill(loose);
    expect(loose.dataset.away).toBeUndefined();
  });
});
