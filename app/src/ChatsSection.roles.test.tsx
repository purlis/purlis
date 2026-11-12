// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { FinishedTask } from "./bindings";
import { drawnWith } from "./cascade.testkit";
import { theCard } from "./chatCard.testkit";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import { DEFAULT_CHATS_LIST, setChatsListPrefs } from "./chatsListPrefs";
import { chatsTree, type ListedChat } from "./chatsTree";
import { declarations } from "./guards.testkit";
import { tasksBelowOf } from "./sessionTasks";
import { TasksBelowLent } from "./TasksBelow";
import { forgetThisLaunch } from "./regions";
import { keysOn, rowAround, rowsOf } from "./ui/tree/tree.testkit";

/**
 * **Every word of the Chats list wears a text role** (#1920, the first surface to move;
 * `docs/design-system.md` "Text roles"). The list set one size on its root (`0.85rem`) and told
 * its words apart by colour, with four more literal sizes on its notes, two of them under the
 * scale's smallest step. Each word is now one of the roles, and this file holds each to the
 * declarations of its role, resolved by the window's own cascade (`cascade.testkit.ts`), so a
 * rule of `App.css` that re-ranked one would fail here.
 *
 * **A row is a tree row's height**, the tree family's in a view, as the Explorer's file rows are
 * (#3198), and nothing in the list writes a height or a block padding over it.
 */

const read = (file: string) => readFileSync(join(process.cwd(), "src", file), "utf8");
const core = new Map(
  declarations(read("theme/geometry.css")).map(({ property: name, value }) => [name, value]),
);
const app = declarations(read("App.css"));

/** What a `var(--x)` names, or the value itself when it is not one. */
const named = (value: string | undefined) => /^var\((--[\w-]+)\)$/.exec(value ?? "")?.[1] ?? value;

/** A size token in px at the window's default text size (14px). */
function px(token: string | undefined): number {
  const value = core.get(token ?? "") ?? "";
  const share = /^calc\(([\d.]+)rem \/ 14\)$/.exec(value);
  if (share) return Number(share[1]);
  const rem = /^([\d.]+)rem$/.exec(value);
  if (rem) return Number(rem[1]) * 14;
  throw new Error(`${token} is ${value}, not a core size`);
}

/** The role's table (`primitives/text.test.ts` holds the classes to it). */
const ROLES = {
  body: { class: "ui-text-body", size: 13, weight: "400", colour: "--text-primary" },
  secondary: { class: "ui-text-secondary", size: 12, weight: "400", colour: "--text-secondary" },
  meta: { class: "ui-text-meta", size: 11, weight: "400", colour: "--text-quiet" },
  label: { class: "ui-text-label", size: 11, weight: "600", colour: "--text-secondary" },
} as const;

/** `word` wears `role`, and the window draws it with that role's size, weight and colour. */
function wears(word: Element | null | undefined, role: keyof typeof ROLES, what: string) {
  expect(word, `${what} was not drawn`).toBeTruthy();
  const one = word as Element;
  const { class: name, size, weight, colour } = ROLES[role];
  expect(one.classList.contains(name), `${what} does not wear ${name}`).toBe(true);
  expect(px(named(drawnWith(one, "font-size"))), `${what}: size`).toBe(size);
  expect(core.get(named(drawnWith(one, "font-weight")) ?? ""), `${what}: weight`).toBe(weight);
  expect(named(drawnWith(one, "color")), `${what}: colour`).toBe(colour);
}

function listed(session: number, more: Partial<ListedChat> = {}): ListedChat {
  return {
    session,
    name: `steward ${session}`,
    persona: "steward",
    workspace: "smart-ide",
    shell: false,
    parent: null,
    mode: null,
    from: null,
    tab: true,
    branch: null,
    report: null,
    outcome: null,
    asking: null,
    harness: "Claude Code",
    ...more,
  };
}

const task = (session: number, parent: number, more: Partial<ListedChat> = {}) =>
  listed(session, {
    persona: "devops",
    parent,
    mode: "task",
    from: `chat ${parent}`,
    tab: false,
    report: "owed",
    ...more,
  });

const CHATS: ListedChat[] = [
  // At its task limit, with a dispatch waiting on memory: the two lines under a row.
  listed(1, {
    atLimit: { limit: 2, row: "at its task limit (2)", said: "It runs 2 tasks at once." },
    waitingOnMemory: 1,
  } as Partial<ListedChat>),
  // A task with no tab of its own, whose restart was refused: said under its row.
  task(2, 1, { name: "devops 2" }),
  // Being stopped.
  listed(3),
  // Folded over a task: the count at its row's end.
  listed(4, { workspace: "volaticloud" }),
  task(5, 4, { name: "devops 5", workspace: "volaticloud" }),
  // Over finished tasks: one alone with a note, and enough to fold.
  listed(6, { workspace: "volaticloud" }),
];

function finished(id: string, asker: number, more: Partial<FinishedTask> = {}): FinishedTask {
  return {
    how: "done",
    chat: null,
    not_reopened: null,
    did_not_start: false,
    attempts: 0,
    waits: null,
    id,
    asker,
    name: `check ${id}`,
    persona: "devops",
    outcome: "done",
    folds: true,
    report: "All good.",
    changed: null,
    ended: "2026-10-08T12:04:30+00:00",
    place: "smart-ide",
    branch: "purlis/dispatch/check",
    reopens: true,
    ...more,
  };
}

const FINISHED = new Map<number, FinishedTask[]>([
  [
    6,
    [
      finished("f", 6, {
        how: "unreported",
        outcome: "ended without a report",
        folds: false,
        not_reopened: "The chat it reopened in ended at once.",
      }),
      ...["a", "b", "c"].map((id) => finished(id, 6)),
    ],
  ],
]);

function draw(chats: ListedChat[] = CHATS) {
  const { container } = render(
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <TasksBelowLent below={tasksBelowOf(chats, FINISHED)}>
        <ChatsSection
          rows={chatsTree(chats)}
          front={1}
          onOpen={() => {}}
          stopping={new Set([3])}
          restarts={{ 2: "It is restarted when its turn ends." }}
          finished={FINISHED}
          onReopen={() => Promise.resolve("It could not be reopened: its branch is gone.")}
        />
      </TasksBelowLent>
    </ChatsHere.Provider>,
  );
  const section = container.querySelector<HTMLElement>(".chats-section");
  if (section === null) throw new Error("no Chats section was drawn");
  return section;
}

const rowOf = (section: HTMLElement, name: string) => {
  const found = rowsOf(section).find((one) => one.querySelector(".session")?.textContent === name);
  if (found === undefined) throw new Error(`no row is named ${name}`);
  return found;
};

beforeEach(() => {
  forgetThisLaunch();
  setChatsListPrefs(DEFAULT_CHATS_LIST);
});
afterEach(cleanup);

describe("the Chats list's words wear text roles (#1920)", () => {
  it("is a body's surface: what no rule ranks is drawn at the body's size", () => {
    const section = draw();

    wears(section, "body", "the section");
    // The root's own size is gone with its rule: the role is the one thing that sets it.
    expect(
      app.filter(
        ({ context, property: name }) =>
          context.join(" ") === ".chats-section" && ["font-size", "color"].includes(name),
      ),
    ).toEqual([]);
  });

  it("titles the view as a view's title, a strong body in its own case (#3342)", () => {
    const section = draw();
    const title = section.querySelector("#chats-title") as Element;

    expect(title).toHaveClass("side-title", "ui-text-body", "ui-text-strong");
    expect(title.tagName).toBe("H2");
  });

  it("draws a chat's name as a body, at the regular weight on every row", () => {
    const section = draw();
    const names = rowsOf(section)
      .filter((row) => row.classList.contains("chat"))
      .map((row) => row.querySelector('[data-part="label"]'));

    expect(names.length).toBeGreaterThanOrEqual(5);
    for (const name of names) wears(name, "body", `${name?.textContent}'s name`);
  });

  it("draws what is said under a row as secondary lines", () => {
    const section = draw();

    wears(section.querySelector('[data-testid="at-limit-1"]'), "secondary", "the task limit");
    wears(section.querySelector('[data-testid="on-memory-1"]'), "secondary", "waiting on memory");
    wears(section.querySelector(".restart-said"), "secondary", "what a restart says");
  });

  it("draws the notes on the title's line as a secondary line", () => {
    const section = draw();

    wears(section.querySelector(".chats-notes"), "secondary", "the notes");
    // Still one line high, in its own text's line, so its words coming and going move no row.
    expect(drawnWith(section.querySelector(".chats-notes") as Element, "height")).toBe("1lh");
  });

  it("draws the switch's line in the secondary's size", () => {
    // The switch has a fixed size of its own (the segmented control's `sm`, #2907), so this
    // line's size no longer sets it; the line is the notes', the secondary's.
    const section = draw();

    expect(named(drawnWith(section.querySelector(".chats-top") as Element, "font-size"))).toBe(
      "--font-size-sm",
    );
  });

  it("draws the line that says there are no chats as a secondary line, undimmed", () => {
    const section = draw([]);
    const none = section.querySelector(".empty");

    wears(none, "secondary", "the empty line");
    // The role's colour is what makes it quiet, not a second dimming on top of it.
    expect(drawnWith(none as Element, "opacity")).toBe("1");
  });

  it("draws a folded row's count and a stop in progress as meta, the count strong", async () => {
    const section = draw();
    await keysOn(rowOf(section, "steward 4"), "{ArrowLeft}");

    wears(section.querySelector(".stopping"), "meta", "Stopping…");
    // A count is the count family's pill (#2912): the counts' 11px at the strong weight, as every
    // count in the window is (the ruling's T5), on the count's ground.
    const count = section.querySelector(".task-count") as Element;
    expect(count).toHaveAttribute("data-scope", "count");
    expect(px(named(drawnWith(count, "font-size")))).toBe(11);
    expect(core.get(named(drawnWith(count, "font-weight")) ?? "")).toBe("600");
    expect(named(drawnWith(count, "color"))).toBe("--text-secondary");
    expect(named(drawnWith(count, "background-color"))).toBe("--control-count");
  });

  it("heads a workspace's sessions with the frame's group head, where the list is grouped (#3484)", () => {
    setChatsListPrefs({ grouped: true });
    const section = draw();
    // Each group's head is its `TreeSection`'s `header` part (#3198).
    const groups = [...section.querySelectorAll('.chats-group [data-part="header"]')];

    expect(groups.map((one) => one.querySelector(".chats-group-name")?.textContent)).toEqual([
      "smart-ide",
      "volaticloud",
    ]);
    for (const group of groups) {
      // The frame's group head (`ui/panel/panel.css`): 11px spaced capitals, medium, quiet.
      expect(named(drawnWith(group, "font-size"))).toBe("--font-size-xs");
      expect(named(drawnWith(group, "font-weight"))).toBe("--font-weight-medium");
      expect(named(drawnWith(group, "color"))).toBe("--text-quiet");
      expect(drawnWith(group, "text-transform")).toBe("uppercase");
      expect(drawnWith(group, "letter-spacing")).toBe("0.06em");
      expect(group.querySelector("[data-count]")?.textContent).toMatch(/^\d+$/);
    }
  });

  it("lets a chat row's trail give way where its words may, so a long asker's name never widens the list", () => {
    // A trail that says who a question waits on gives way with the label (#4564), on the tree
    // family's trail as it did on the shared row's (#3198). Drawn as `Row` draws it, with the
    // `data-gives-way` a task's question sets.
    document.body.insertAdjacentHTML(
      "beforeend",
      '<section class="chats-section"><div data-scope="tree" data-part="root" class="chat"><span data-scope="tree" data-part="trail" data-gives-way="" class="giving">x</span></div></section>',
    );
    const trail = document.body.querySelector(".giving") as Element;
    try {
      expect(drawnWith(trail, "flex")).toBe("0 1 auto");
      expect(drawnWith(trail, "min-inline-size")).toBe("0");
    } finally {
      trail.closest("section")?.remove();
    }
  });

  it("lets a stopping row's word give way before its time", async () => {
    // A stopping chat's trail gives way (#3198), and in it the word shrinks and the time does not.
    document.body.insertAdjacentHTML(
      "beforeend",
      '<section class="chats-section"><div class="chat"><span class="stopping">Stopping…</span><span class="chat-time">10:42</span></div></section>',
    );
    const word = document.body.querySelector(".stopping") as Element;
    const time = document.body.querySelector(".chat-time") as Element;
    try {
      expect(drawnWith(word, "flex")).toBe("0 1 auto");
      expect(drawnWith(word, "text-overflow")).toBe("ellipsis");
      expect(drawnWith(time, "flex")).toBe("none");
    } finally {
      word.closest("section")?.remove();
    }
  });

  it("holds who a question waits on to a share of the list, so the chat's name and the time keep theirs", () => {
    document.body.insertAdjacentHTML(
      "beforeend",
      '<section class="chats-section"><span class="question-waits"><span class="question-who">asking x</span></span></section>',
    );
    const who = document.body.querySelector(".question-who") as Element;
    try {
      expect(drawnWith(who, "max-inline-size")).toBe("30cqi");
      expect(drawnWith(who, "text-overflow")).toBe("ellipsis");
    } finally {
      who.closest("section")?.remove();
    }
  });

  it("puts a finished task's buttons under its name where they do not fit, so the list never scrolls sideways", () => {
    const section = draw();
    const row = rowAround(section.querySelector('.finished-task [data-part="label"]'));
    expect(row).not.toBeNull();
    const trail = (row as Element).querySelector('[data-part="trail"]') as Element;
    // The row wraps onto a line of its own for them, and they wrap within its width (#1499):
    // `chats-list.e2e.ts` measures the list at the region's least width in the real engine.
    expect(drawnWith(row as Element, "flex-wrap")).toBe("wrap");
    expect(drawnWith(trail, "flex-wrap")).toBe("wrap");
    expect(drawnWith(trail, "max-inline-size")).toBe("100%");
  });

  it("draws a finished task quieter than a chat: its name, its fold and its note secondary", async () => {
    const section = draw();

    const name = section.querySelector('.finished-task [data-part="label"]');
    wears(name, "secondary", "a finished name");
    wears(
      section.querySelector('.finished-count [data-part="label"]'),
      "secondary",
      "Finished (n)",
    );
    wears(section.querySelector(".finished-note"), "secondary", "a finished task's note");
    // No colour of the list's own on a finished row: the tree family's, which its words' roles
    // draw over (#3198).
    expect(named(drawnWith(rowAround(name) as Element, "color"))).toBe("--text-secondary");

    await userEvent.click(name as Element);
    wears(section.querySelector(".finished-report"), "body", "a report");
    wears(section.querySelector(".report-where"), "secondary", "where a report was written");
  });

  it("draws why a finished task was not reopened at its name's size, in the danger colour", async () => {
    const section = draw();
    await userEvent.click(section.querySelector(".finished-task .finished-reopen") as Element);
    const refusal = await within(section).findByRole("alert");

    // No role has the danger colour, so it wears none: one rule built from a token sizes it.
    expect([...refusal.classList]).toEqual(["trouble"]);
    expect(px(named(drawnWith(refusal, "font-size")))).toBe(12);
    expect(named(drawnWith(refusal, "color"))).toBe("--danger-text");
    expect(drawnWith(refusal, "font-weight")).toBeUndefined();
  });

  it("draws a row's card as secondary lines under a strong body name, its terms meta", async () => {
    const section = draw();
    await userEvent.hover(rowOf(section, "steward 3"));
    const card = await theCard();

    wears(card, "secondary", "the card");
    // Its first line is a body's: the state's word beside the name is sized by it, as it was.
    wears(card.querySelector(".chat-card-head"), "body", "the card's first line");
    const name = card.querySelector(".chat-card-name");
    expect(name?.classList.contains("ui-text-strong")).toBe(true);
    expect(px(named(drawnWith(name as Element, "font-size")))).toBe(13);
    expect(core.get(named(drawnWith(name as Element, "font-weight")) ?? "")).toBe("600");
    expect(named(drawnWith(name as Element, "color"))).toBe("--text-primary");
    const terms = [...card.querySelectorAll("dt")];
    expect(terms.length).toBeGreaterThanOrEqual(2);
    for (const term of terms) wears(term, "meta", `the card's ${term.textContent}`);
    // A term and its value are two sizes: they share a baseline.
    expect(drawnWith(card.querySelector(".chat-card-facts") as Element, "align-items")).toBe(
      "baseline",
    );
  });

  it("writes no size, weight or letter spacing of its own, but the scope switch's line's", () => {
    const own = app.filter(
      ({ context, property: name, value }) =>
        /\.chats-|\.chat-card/.test(context.join(" ")) &&
        ["font-size", "font-weight", "letter-spacing", "text-transform"].includes(name) &&
        value !== "inherit",
    );

    // A token, for a control sized in `em`; every word on that line wears its own role.
    expect(
      own.map(({ context, property: name, value }) => `${context.join(" ")} ${name}: ${value}`),
    ).toEqual([
      ".chats-top font-size: var(--font-size-sm)",
      ".chats-section .finished-below .trouble font-size: var(--font-size-sm)",
    ]);
  });
});

describe("a Chats row is a tree row's height (#1920, #3198)", () => {
  it("is a view's row, 30px from the panel family's rule for the Chats view, and nothing of its own", () => {
    const section = draw();
    const row = rowOf(section, "steward 1");
    // The view's row height (the prototype's 30px, #3342), from `ui/panel/panel.css`'s rule for
    // the Chats view's tree (#3198): the Explorer's trees keep the family's own sizes.
    expect(drawnWith(row, "min-block-size")).toBe("calc(var(--size-control) + var(--space-1))");
    // Nothing of the Chats list's own sizes a row.
    expect(
      app.filter(
        ({ context, property: name }) =>
          /\.chat(\[[^\]]*\]|:[^ ]+)*$/.test(context.join(" ")) &&
          [
            "height",
            "block-size",
            "min-height",
            "min-block-size",
            "padding",
            "padding-block",
            "line-height",
          ].includes(name),
      ),
    ).toEqual([]);
  });

  it("spaces what is under and beside a row from the geometry tokens", async () => {
    const section = draw();
    await keysOn(rowOf(section, "steward 4"), "{ArrowLeft}");
    // What is said under a row is the row's own `below` (#3198): one column of lines, spaced
    // by a token, the task limit's words and its link a token apart.
    const below = section.querySelector(".chat-below") as Element;
    expect(below).toHaveAttribute("data-part", "below");
    expect(drawnWith(below, "gap")).toBe("var(--space-1)");
    expect(below.contains(section.querySelector(".at-limit"))).toBe(true);
    expect(drawnWith(section.querySelector(".at-limit") as Element, "gap")).toBe("var(--space-2)");
    expect(section.querySelector(".restart-said")?.closest(".chat-below")).not.toBeNull();
    const count = section.querySelector(".task-count") as Element;
    expect(drawnWith(count, "padding-inline")).toBe("var(--space-2)");
    expect(drawnWith(count, "border-radius")).toBe("var(--radius-full)");
  });
});
