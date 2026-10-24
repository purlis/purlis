// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { FinishedTask } from "./bindings";
import { cascadeOf, drawnWith } from "./cascade.testkit";
import { theCard } from "./chatCard.testkit";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import { DEFAULT_CHATS_LIST, setChatsListPrefs } from "./chatsListPrefs";
import { chatsTree, type ListedChat } from "./chatsTree";
import { declarations } from "./guards.testkit";
import { tasksBelowOf } from "./sessionTasks";
import { TasksBelowLent } from "./TasksBelow";
import { forgetThisLaunch } from "./regions";

/**
 * **Every word of the Chats list wears a text role** (#1920, the first surface to move;
 * `docs/design-system.md` "Text roles"). The list set one size on its root (`0.85rem`) and told
 * its words apart by colour, with four more literal sizes on its notes, two of them under the
 * scale's smallest step. Each word is now one of the roles, and this file holds each to the
 * declarations of its role, resolved by the window's own cascade (`cascade.testkit.ts`), so a
 * rule of `App.css` that re-ranked one would fail here.
 *
 * **A row is the shared row's height**, the Explorer's file rows' (`--ui-row-height`), and
 * nothing in the list writes a height or a block padding over it.
 */

const read = (file: string) => readFileSync(join(process.cwd(), "src", file), "utf8");
const core = new Map(
  declarations(read("theme/geometry.css")).map(({ property: name, value }) => [name, value]),
);
const app = declarations(read("App.css"));
const collections = cascadeOf("primitives/collections.css");

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
  const found = within(section)
    .getAllByRole("treeitem")
    .find((one) => one.querySelector(".session")?.textContent === name);
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

  it("titles the section with the one section label", () => {
    const section = draw();
    const title = section.querySelector("#chats-title");

    wears(title, "label", "the title");
    expect(drawnWith(title as Element, "text-transform")).toBe("uppercase");
    expect(drawnWith(title as Element, "letter-spacing")).toBe("0.06em");
  });

  it("draws a chat's name as a body, at the regular weight on every row", () => {
    const section = draw();
    const names = [...section.querySelectorAll('[role="treeitem"] > .session')];

    expect(names.length).toBeGreaterThanOrEqual(5);
    for (const name of names) wears(name, "body", `${name.textContent}'s name`);
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
    // The switch has a fixed size of its own (`.ui-levels-small`, `--font-size-2xs`, #2319), so
    // this line's size no longer sets it; the line is the notes', the secondary's.
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

  it("draws a folded row's count and a stop in progress as meta, the count strong", () => {
    const section = draw();
    fireEvent.keyDown(rowOf(section, "steward 4"), { key: "ArrowLeft" });

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

  it("heads a workspace's sessions with the section label, where the list is grouped", () => {
    setChatsListPrefs({ grouped: true });
    const section = draw();
    const groups = [...section.querySelectorAll(".chats-group")];

    expect(groups.map((one) => one.textContent)).toEqual(["smart-ide", "volaticloud"]);
    for (const group of groups) {
      wears(group, "label", `${group.textContent}'s heading`);
      expect(drawnWith(group, "text-transform")).toBe("uppercase");
      expect(drawnWith(group, "letter-spacing")).toBe("0.06em");
    }
  });

  it("draws a finished task quieter than a chat: its name, its fold and its note secondary", async () => {
    const section = draw();

    wears(section.querySelector(".finished-name .session"), "secondary", "a finished name");
    wears(section.querySelector(".finished-count"), "secondary", "Finished (n)");
    wears(section.querySelector(".finished-note"), "secondary", "a finished task's note");
    // No colour on the group itself: each word in it has its own role's.
    expect(drawnWith(section.querySelector(".finished-tasks") as Element, "color")).toBeUndefined();

    await userEvent.click(section.querySelector(".finished-name") as Element);
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
      ".chats-section .finished-task > .trouble font-size: var(--font-size-sm)",
    ]);
  });
});

describe("a Chats row is the shared row's height (#1920)", () => {
  it("is as high as a file's row: the row token, and nothing of the list's over it", () => {
    const section = draw();
    const row = rowOf(section, "steward 1");
    document.body.insertAdjacentHTML(
      "beforeend",
      '<div class="explorer"><div class="ui-tree"><div class="ui-row ui-tree-row file-node" aria-level="1">a</div></div></div>',
    );
    const file = document.body.querySelector(".file-node") as Element;
    const drawn = (one: Element, property: string) =>
      drawnWith(one, property) ?? collections(one, property);

    try {
      for (const one of [row, file]) {
        expect(drawn(one, "min-block-size")).toBe("var(--ui-row-height)");
        expect(collections(one, "--ui-row-height")).toBe("var(--size-chrome)");
        expect(drawn(one, "padding-block")).toBe("0");
        for (const property of ["height", "block-size", "min-height", "padding", "line-height"])
          expect(drawnWith(one, property), property).toBeUndefined();
      }
      // The tree's step is the shared one for both: no list sets its own.
      for (const tree of [section.querySelector(".ui-tree"), file.parentElement])
        expect(drawnWith(tree as Element, "--ui-tree-step")).toBe("var(--space-5)");
      // 1.5rem: 21px at the window's default text.
      expect(px("--size-chrome")).toBe(21);
    } finally {
      file.closest(".explorer")?.remove();
    }
  });

  it("spaces what is under and beside a row from the geometry tokens", () => {
    const section = draw();
    fireEvent.keyDown(rowOf(section, "steward 4"), { key: "ArrowLeft" });
    const under = "0 var(--space-4) var(--space-2) var(--chats-name-at)";

    expect(drawnWith(section.querySelector(".at-limit") as Element, "padding")).toBe(under);
    expect(drawnWith(section.querySelector(".at-limit") as Element, "gap")).toBe("var(--space-2)");
    expect(drawnWith(section.querySelector(".restart-said") as Element, "padding")).toBe(under);
    const count = section.querySelector(".task-count") as Element;
    expect(drawnWith(count, "padding-inline")).toBe("var(--space-2)");
    expect(drawnWith(count, "border-radius")).toBe("var(--radius-full)");
  });
});
