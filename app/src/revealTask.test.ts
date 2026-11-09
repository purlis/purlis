// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { chatsTree, type ListedChat } from "./chatsTree";
import { chatRowOf, finishedRowById, finishedRowOf, rowsAbove } from "./revealTask";

/** Bringing a failed task's finished row into view (#1491). */

function listed(session: number, parent: number | null = null): ListedChat {
  return {
    session,
    name: `chat ${session}`,
    persona: null,
    workspace: "alpha",
    shell: false,
    parent,
    mode: parent === null ? null : "task",
    from: parent === null ? null : `chat ${parent}`,
    tab: parent === null,
    branch: null,
    report: null,
    outcome: null,
    asking: null,
    harness: null,
  };
}

describe("the rows a finished row is drawn under", () => {
  const rows = chatsTree([listed(1), listed(2, 1), listed(3, 2), listed(4, 1), listed(5)]);

  it("are its session's and every chat's above that one", () => {
    expect(rowsAbove(rows, 3)).toEqual([3, 2, 1]);
    expect(rowsAbove(rows, 4)).toEqual([4, 1]);
    expect(rowsAbove(rows, 5)).toEqual([5]);
  });

  it("are none for a chat that is not listed", () => {
    expect(rowsAbove(rows, 99)).toEqual([]);
  });
});

describe("a finished row, found by what the list says of itself", () => {
  /** The Chats tree as the tree family draws it (#3198): one flat run of rows, each saying its
   *  level; a task's finished row under its chat's, or under its "Finished (n)" row. */
  const list = () => {
    const section = document.createElement("section");
    section.innerHTML = `
      <div role="treegrid">
        <div role="row" class="chat" aria-label="steward 1" aria-level="1"></div>
        <div role="row" class="finished-task" aria-label="check prod" aria-level="2"></div>
        <div role="row" class="finished-count" aria-label="Finished (1)" aria-level="2"></div>
        <div role="row" class="finished-task" aria-label="check staging" aria-level="3"></div>
        <div role="row" class="chat" aria-label="steward 2" aria-level="1"></div>
        <div role="row" class="chat" aria-label="devops 3" aria-level="2"></div>
        <div role="row" class="finished-task" aria-label="check prod" aria-level="2"></div>
      </div>`;
    return section;
  };

  it("is the row of that name under that session, and no other session's", () => {
    const section = list();
    const rows = [...section.querySelectorAll('[role="row"]')];
    expect(rows.indexOf(finishedRowOf(section, "steward 2", "check prod") as Element)).toBe(6);
    expect(rows.indexOf(finishedRowOf(section, "steward 1", "check prod") as Element)).toBe(1);
    // Folded into its chat's "Finished (n)" row, it is still that chat's.
    expect(finishedRowOf(section, "steward 1", "check staging")?.getAttribute("aria-label")).toBe(
      "check staging",
    );
  });

  it("is nothing while the row is not drawn", () => {
    expect(finishedRowOf(list(), "steward 1", "check the queue")).toBeNull();
    expect(finishedRowOf(list(), "steward 3", "check prod")).toBeNull();
    // A chat drawn between them is not the one that asked.
    expect(finishedRowOf(list(), "devops 3", "check prod")).toBeNull();
    expect(finishedRowOf(null, "steward 1", "check prod")).toBeNull();
  });
});

describe("a chat's own row, found by its number (#1490)", () => {
  it("is the row of the tree that carries it, and no other element that does", () => {
    const section = document.createElement("section");
    section.innerHTML = `
      <div data-session="7">a pane</div>
      <div role="row" data-session="7">steward 7</div>
      <div role="row" data-session="70">steward 70</div>`;

    expect(chatRowOf(section, 7)?.textContent).toBe("steward 7");
    expect(chatRowOf(section, 8)).toBeNull();
    expect(chatRowOf(null, 7)).toBeNull();
  });
});

describe("a finished row, found by its dispatch record's id (#1491)", () => {
  const list = () => {
    const section = document.createElement("section");
    section.innerHTML = `
      <div role="row" class="finished-task" aria-label="check prod" data-task-id="01K6A">
        <button class="finished-clear">Clear</button>
      </div>
      <div role="row" class="finished-task" aria-label="check prod" data-task-id="01K6B"></div>`;
    return section;
  };

  it("tells two tasks of one name apart, and is the row itself (#3198)", () => {
    const row = finishedRowById(list(), "01K6B");
    expect(row?.getAttribute("role")).toBe("row");
    expect(row?.getAttribute("data-task-id")).toBe("01K6B");
  });

  it("is nothing for a row that is not drawn", () => {
    expect(finishedRowById(list(), "01K6C")).toBeNull();
    expect(finishedRowById(null, "01K6A")).toBeNull();
  });
});
