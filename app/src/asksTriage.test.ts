// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { OTHER_SCOPES } from "./AskDecision";
import {
  beginTriage,
  endTriage,
  firstLead,
  HIGH_RISK,
  leadOf,
  nextRow,
  returnFromTriage,
  triageKeyOf,
  triaging,
} from "./asksTriage";

/**
 * **Where keyboard triage lands** (ADR 0121 decision 9, #6016), on rows drawn by hand as the list
 * family draws Asks' rows: a high-risk ask is passed by (decision 8, #6011's seam), a grant never
 * takes the keyboard, and ⌘⇧I pressed again in Asks keeps where triage began.
 */

afterEach(() => {
  endTriage();
  document.body.replaceChildren();
});

/** An Asks list of rows, each with its buttons, as `[label, attributes]`. */
function asks(...rows: { risk?: boolean; buttons: [string, Record<string, string>][] }[]) {
  const section = document.createElement("section");
  section.dataset.view = "asks";
  const list = document.createElement("div");
  list.className = "inbox-list";
  rows.forEach((one, at) => {
    const row = document.createElement("div");
    row.setAttribute("role", "row");
    row.dataset.key = `ask-${at}`;
    row.tabIndex = -1;
    if (one.risk === true) row.setAttribute(HIGH_RISK, "");
    for (const [label, attributes] of one.buttons) {
      const button = document.createElement("button");
      button.textContent = label;
      for (const [name, value] of Object.entries(attributes)) button.setAttribute(name, value);
      row.append(button);
    }
    list.append(row);
  });
  section.append(list);
  document.body.append(section);
  return list;
}

const ALLOW: [string, Record<string, string>] = [
  "Allow",
  { "data-way": "fix", "data-variant": "grants" },
];
const DENY: [string, Record<string, string>] = [
  "Deny",
  { "data-way": "fix", "data-variant": "secondary" },
];

describe("where triage lands in a row", () => {
  it("lands on Answer before anything else", () => {
    const list = asks({ buttons: [["Answer", { "data-lead": "" }], DENY] });
    expect(firstLead(list)).toHaveTextContent("Answer");
  });

  it("lands on the answer its source suggests, else the first that grants nothing", () => {
    const suggested = asks({
      buttons: [
        ["Yes", { "data-way": "fix" }],
        ["No", { "data-way": "fix", "data-variant": "primary" }],
      ],
    });
    expect(firstLead(suggested)).toHaveTextContent("No");
    document.body.replaceChildren();
    expect(firstLead(asks({ buttons: [ALLOW, DENY] }))).toHaveTextContent("Deny");
  });

  it("passes Other scopes by, which answers nothing, for the answer after it (#6009)", () => {
    const list = asks({
      buttons: [ALLOW, [OTHER_SCOPES, { "data-way": "fix", "data-variant": "secondary" }], DENY],
    });
    expect(firstLead(list)).toHaveTextContent("Deny");
  });

  it("lands on a group's Answer each, its answer that grants nothing (#6009)", () => {
    const list = asks({ buttons: [ALLOW, ["Answer each", { "data-triage-lead": "" }]] });
    expect(firstLead(list)).toHaveTextContent("Answer each");
  });

  it("never lands on a grant, even one marked as the row's lead", () => {
    const list = asks({
      buttons: [["Allow", { "data-lead": "", "data-variant": "grants" }], DENY],
    });
    expect(firstLead(list)).toHaveTextContent("Deny");
  });

  it("lands on the row itself where it has nothing else", () => {
    const list = asks({ buttons: [] });
    const row = list.querySelector<HTMLElement>('[role="row"]') as HTMLElement;
    expect(leadOf(row)).toBe(row);
  });
});

describe("a high-risk ask (ADR 0121 decision 8)", () => {
  it("is never landed on, first or next", () => {
    const list = asks(
      { risk: true, buttons: [["Push", { "data-way": "fix" }]] },
      { buttons: [DENY] },
      { risk: true, buttons: [["Deploy", { "data-way": "fix" }]] },
    );
    expect(firstLead(list)).toHaveTextContent("Deny");
    const only = list.querySelectorAll<HTMLElement>('[role="row"]')[1];
    expect(nextRow(list, only)).toBeUndefined();
  });

  it("leaves no first lead where every ask is one", () => {
    expect(firstLead(asks({ risk: true, buttons: [DENY] }))).toBeNull();
  });
});

describe("where triage began", () => {
  it("keeps it when ⌘⇧I is pressed again in Asks, and goes back there", () => {
    const list = asks({ buttons: [DENY] });
    const chat = document.createElement("textarea");
    document.body.append(chat);
    chat.focus();
    beginTriage(document.activeElement);
    firstLead(list)?.focus();
    beginTriage(document.activeElement);
    expect(returnFromTriage()).toBe(true);
    expect(chat).toHaveFocus();
    expect(triaging()).toBe(false);
  });

  it("cannot go back to an element taken away since", () => {
    const chat = document.createElement("textarea");
    document.body.append(chat);
    beginTriage(chat);
    chat.remove();
    expect(returnFromTriage()).toBe(false);
  });
});

describe("triage's keys", () => {
  const keystroke = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);

  it("are ⌘⌥ and a letter on a Mac, Alt+Shift and the letter elsewhere, by its place", () => {
    expect(
      triageKeyOf(keystroke({ code: "KeyL", key: "¬", metaKey: true, altKey: true }), true),
    ).toBe("later");
    expect(
      triageKeyOf(keystroke({ code: "KeyX", key: "X", altKey: true, shiftKey: true }), false),
    ).toBe("stop");
  });

  it("are never a letter alone, or a letter with Alt alone (I-11)", () => {
    expect(triageKeyOf(keystroke({ code: "KeyS", key: "s" }), true)).toBeUndefined();
    expect(triageKeyOf(keystroke({ code: "KeyS", key: "s" }), false)).toBeUndefined();
    expect(triageKeyOf(keystroke({ code: "KeyG", key: "©", altKey: true }), true)).toBeUndefined();
  });
});
