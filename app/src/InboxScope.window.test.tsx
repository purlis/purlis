// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cascadeOf, drawnWith } from "./cascade.testkit";
import { Inbox, NO_CHAT_FOCUSED, NOTHING_WAITS, landOnGroupIn } from "./Inbox";
import { OTHER_PROJECTS, type Elsewhere } from "./InboxElsewhere";
import type { Needing } from "./NeedsYou";
import { forgetInbox } from "./inboxRules";
import { forgetAsksScope, keptAsksScope, pickAsksScope } from "./asksScope";
import { forgetProjectViews, keptFacet } from "./projectViews";
import type { Shown } from "./bindings";
import { hoverTipOf, tipOf } from "./ui/tooltip/tooltip.testkit";
import { chatGroupOrNone } from "./test-inbox";

/**
 * **The Inbox's scope, and what one of its rows is made of** (qw359): every project's, this
 * project's or this workspace's asks, the pick kept for the project; and a row's one strong
 * line, its quieter line of where, and its small trailing kind and age.
 */

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date("2026-10-13T12:00:00Z"));
  mockIPC(() => null);
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  forgetAsksScope(PLANE);
  vi.useRealTimers();
});

const PLANE = "/home/dev/plane";
/** The key the strip gives the plane root, as the window hands it. */
const ROOT = "\u0000root";

const permission = (session: number, ask: string, says = "Run cargo test"): Shown => ({
  session,
  ask,
  says,
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: [`steward ${session}`],
  answer: { via: "hook" },
});

const question = (session: number, says = "Waiting on your reply"): Shown => ({
  session,
  ask: `question:${session}`,
  says,
  options: [],
  source: "question",
  chain: [`steward ${session}`],
  answer: { via: "in-its-pane" },
});

/** Chats 3, 4, 6 and 8 work in alpha, chat 5 in beta, and chat 9 at the plane root. */
const FILED: Record<number, string> = {
  3: "alpha",
  4: "alpha",
  6: "alpha",
  8: "alpha",
  5: "beta",
  9: ROOT,
};
const strip = (here: string) => ({
  here,
  workspaceOf: (session: number) => FILED[session],
  workspaceNamed: (key: string) => (key === ROOT ? "Plane root" : key),
});

const away: Needing = {
  plane: "/home/dev/ops",
  project: "ops",
  session: 7,
  name: "steward 7",
  workspace: "infra",
};

const elsewhere = (over: Partial<Elsewhere> = {}): Elsewhere => ({
  asking: [],
  quiet: [],
  onPress: vi.fn(),
  onOpen: vi.fn(),
  waitingHere: [{ plane: PLANE, project: "plane", count: 0 }],
  ...over,
});

const ASKS = [permission(3, "a"), permission(5, "b", "Run ls"), question(9)];

function draw(asks: readonly Shown[] | undefined, more: Partial<Parameters<typeof Inbox>[0]> = {}) {
  const props = { plane: PLANE, asks, onGo: vi.fn(), onLeave: vi.fn(), ...more };
  const drawn = render(<Inbox {...props} />);
  return {
    ...drawn,
    again: (next: Partial<Parameters<typeof Inbox>[0]>) =>
      drawn.rerender(<Inbox {...props} {...next} />),
  };
}

const scopes = () => within(screen.getByRole("radiogroup", { name: "Show what needs you in" }));
const picked = () => scopes().getByRole("radio", { checked: true }).getAttribute("aria-label");
/** The chats the Asks list draws, by the chain each group is named by (#3123: its row header). */
const chats = () =>
  [...document.querySelectorAll('.inbox-chat [role="rowheader"]')].map((h) => h.textContent);

/** Chat 3 dispatched 4, which dispatched 8; chat 6 is 3's sibling, a task of 1. */
const PARENT: Record<number, number> = { 4: 3, 8: 4, 6: 1, 3: 1 };
const askedBy = (session: number) => PARENT[session];
/** Chat 3 has focus, and holds two tasks. */
const focus = (session = 3, tasks = 2) => ({
  chat: { session, name: `steward ${session}`, tasks },
  askedBy,
});

describe("the scope chips (ADR 0121, amended 2026-11-12)", () => {
  const FAMILY = [
    permission(3, "a"),
    permission(4, "c", "Run npm ci"),
    permission(8, "d", "Run git push"),
    permission(6, "e", "Run gh pr view"),
  ];
  const ALL = [...FAMILY, ...ASKS.slice(1)];

  it("offers Chat, Workspace and Project, narrowest first, each with its count, on Chat", () => {
    draw(ALL, { ...strip("alpha"), ...focus() });
    expect(
      scopes()
        .getAllByRole("radio")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual([
      "Chat, 3 waiting on you",
      "Workspace, 4 waiting on you",
      "Project, 6 waiting on you",
    ]);
    expect(picked()).toBe("Chat, 3 waiting on you");
  });

  it("lists under Chat the focused chat's asks and every chat it dispatched, never a sibling's", () => {
    draw(ALL, { ...strip("alpha"), ...focus() });
    expect(chats()).toEqual(["steward 3", "steward 4", "steward 8"]);
  });

  it("draws no Workspace where the project has one workspace", () => {
    draw(ALL, { ...strip("alpha"), ...focus(), oneWorkspace: true });
    expect(
      scopes()
        .getAllByRole("radio")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Chat, 3 waiting on you", "Project, 6 waiting on you"]);
  });

  it("names the chat and its tasks in Chat's tooltip", async () => {
    draw(ALL, { ...strip("alpha"), ...focus() });
    const chat = scopes().getByRole("radio", { name: /^Chat/ });
    expect(await hoverTipOf(chat.closest("label"))).toBe("steward 3 and its 2 tasks");
  });

  it("greys Chat with no chat focused, says why, and lists Workspace", async () => {
    draw(ALL, { ...strip("alpha"), askedBy });
    const chat = scopes().getByRole("radio", { name: "Chat" });
    expect(chat).toBeDisabled();
    expect(await hoverTipOf(chat.closest("label"))).toBe(NO_CHAT_FOCUSED);
    expect(picked()).toBe("Workspace, 4 waiting on you");
    expect(chats()).toEqual(["steward 3", "steward 4", "steward 8", "steward 6"]);
  });

  it("says why Chat is greyed to the keyboard and a screen reader, with no pointer over it", () => {
    draw(ALL, { ...strip("alpha"), askedBy });
    const chat = scopes().getByRole("radio", { name: "Chat" });
    expect(chat).toBeDisabled();
    // Its description, read where the radio is (ADR 0104, `ui/unavailable.ts`): no hover needed.
    expect(chat).toHaveAccessibleDescription(NO_CHAT_FOCUSED);
  });

  it("lists Project for Chat with no chat focused and one workspace", () => {
    draw(ALL, { ...strip("alpha"), askedBy, oneWorkspace: true });
    expect(picked()).toBe("Project, 6 waiting on you");
  });

  it("shows a chip's count only above 0", () => {
    draw(ASKS, { ...strip("beta"), ...focus(6, 0) });
    const counts = [...document.querySelectorAll('.inbox-scope [data-part="count"]')].map(
      (one) => one.textContent,
    );
    expect(counts).toEqual(["1", "3"]);
    expect(scopes().getByRole("radio", { name: "Chat" })).toBeTruthy();
  });

  it("lists Workspace's and Project's asks, and follows the strip", async () => {
    const { again } = draw(ASKS, { ...strip("alpha"), ...focus() });
    await userEvent.click(scopes().getByRole("radio", { name: /^Workspace/ }));
    expect(chats()).toEqual(["steward 3"]);
    again(strip("beta"));
    expect(chats()).toEqual(["steward 3", "steward 5"]);
    await userEvent.click(scopes().getByRole("radio", { name: /^Project/ }));
    expect(chats()).toEqual(["steward 3", "steward 5", "steward 9"]);
  });

  it("lists the asks of no workspace under Workspace at the plane root", async () => {
    draw(ASKS, strip(ROOT));
    expect(picked()).toBe("Workspace, 1 waiting on you");
    expect(chats()).toEqual(["steward 9"]);
  });

  it("holds a pick until Asks closes, opens on Chat again, and keeps nothing for the project", async () => {
    const { again } = draw(ALL, { ...strip("alpha"), ...focus(), shown: true });
    await userEvent.click(scopes().getByRole("radio", { name: /^Project/ }));
    expect(picked()).toBe("Project, 6 waiting on you");
    again({ ...focus(6) });
    expect(picked()).toBe("Project, 6 waiting on you");
    again({ ...focus(6), shown: false });
    again({ ...focus(6), shown: true });
    expect(picked()).toBe("Chat, 1 waiting on you");
    expect(keptFacet(PLANE, "inbox")).toBeUndefined();
  });

  it("lists no other project's asks, though the window knows what waits there", () => {
    const npm = { ...permission(2, "x", "Run npm test"), chain: ["dev 2"] };
    const there = elsewhere({
      asking: [away],
      waitingHere: [
        { plane: PLANE, project: "plane", count: 3 },
        { plane: "/home/dev/web", project: "web", count: 1, asks: [npm] },
        { plane: "/home/dev/api", project: "api", count: 2 },
      ],
    });
    draw(ASKS, { ...strip("alpha"), ...focus(), elsewhere: there });
    expect(
      scopes()
        .getAllByRole("radio")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual([
      "Chat, 1 waiting on you",
      "Workspace, 1 waiting on you",
      "Project, 3 waiting on you",
    ]);
    expect(screen.queryByRole("radio", { name: /All projects/ })).toBeNull();
    for (const scope of [/^Chat/, /^Workspace/, /^Project/]) {
      act(() => scopes().getByRole("radio", { name: scope }).click());
      expect(chatGroupOrNone("dev 2")).toBeNull();
      expect(chatGroupOrNone("steward 7")).toBeNull();
      expect(chatGroupOrNone(OTHER_PROJECTS)).toBeNull();
    }
  });
});

describe("a New ask a chip shows (#6002)", () => {
  it("is watched to be seen once its row is drawn, though nothing else draws Asks again", async () => {
    const watch = vi.fn<(element: Element, key: string) => () => void>(() => () => undefined);
    draw(ASKS, { ...strip("beta"), ...focus(6, 0), seen: { isNew: () => true, watch } });
    expect(watch).not.toHaveBeenCalled();
    await userEvent.click(scopes().getByRole("radio", { name: /^Workspace/ }));
    await waitFor(() =>
      expect(watch.mock.calls.map(([element, key]) => [element.isConnected, key])).toContainEqual([
        true,
        "ask:5:b",
      ]),
    );
  });
});

describe("an empty scope", () => {
  it("offers the nearest wider scope with something waiting, one press to widen, and never widens itself", async () => {
    draw([permission(5, "b"), question(9)], { ...strip("beta"), ...focus() });
    expect(picked()).toBe("Chat");
    expect(chats()).toEqual([]);
    expect(screen.getByText("Nothing is waiting on you in this chat.")).toBeTruthy();
    expect(screen.queryByText(NOTHING_WAITS)).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "1 waiting in this workspace" }));
    expect(chats()).toEqual(["steward 5"]);
    expect(picked()).toBe("Workspace, 1 waiting on you");
  });

  it("takes the keyboard from its way out to the first ask it lists", async () => {
    draw([permission(5, "b"), question(9)], { ...strip("beta"), ...focus() });
    const out = screen.getByRole("button", { name: "1 waiting in this workspace" });
    act(() => out.focus());
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(document.activeElement?.closest(".inbox-chat")?.getAttribute("data-session")).toBe(
        "5",
      ),
    );
  });

  it("goes past an empty workspace to the project", async () => {
    draw([question(9)], { ...strip("alpha"), ...focus() });
    await userEvent.click(screen.getByRole("button", { name: "1 waiting in this project" }));
    expect(chats()).toEqual(["steward 9"]);
  });

  it("is the Inbox's own empty sentence where no scope lists anything", () => {
    draw([], { ...strip("alpha"), ...focus(), elsewhere: elsewhere({ asking: [away] }) });
    expect(screen.getByText(NOTHING_WAITS)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /waiting in/ })).toBeNull();
    // Nothing to narrow, so no chips: as the Chats view draws none over an empty list.
    expect(screen.queryByRole("radiogroup")).toBeNull();
  });
});

describe("what a row is made of", () => {
  /** The one element under `within` that matches, which the test fails without. */
  const part = (within: ParentNode, selector: string): HTMLElement => {
    const found = within.querySelector<HTMLElement>(selector);
    if (found === null) throw new Error(`nothing drawn matches ${selector}`);
    return found;
  };
  const row = (session: number) =>
    part(document, `.inbox-chat[data-session="${session}"] [role="row"].inbox-ask`);

  it("draws a change purlis holds as purlis's, with the chat behind it, filed nowhere (#5240)", () => {
    const held: Shown = {
      session: 0,
      ask: "proposal:3",
      says: "Proposes a change to personas/devops/persona.md, which only you apply",
      options: [{ id: "dismiss", label: "Dismiss", allows: false, preselected: null }],
      source: "proposed-change",
      chain: ["purlis", "billing work"],
      answer: { via: "proposed-change", id: 3, path: "personas/devops/persona.md" },
    };
    draw([held], { ...strip(ROOT), workspaceOf: () => ROOT });
    expect(chats()).toEqual(["purlis › billing work"]);
    expect(document.body.textContent).not.toContain("chat 0");
    expect(document.body.textContent).not.toContain("Plane root");
  });

  it("has its question for its title, its age at the right, then its chat, kind and where, then its words (#5649, #5998)", () => {
    pickAsksScope(PLANE, "project");
    draw([{ ...permission(3, "a"), since: Date.now() / 1000 - 5 * 60 }, question(9)], {
      ...strip("alpha"),
      personaOf: (session) => (session === 3 ? "steward" : null),
    });
    const ask = row(3);
    expect(ask).toHaveAttribute("data-scope", "message");
    expect(part(ask, '[data-part="title"]').textContent).toBe("Allow this action?");
    expect(part(ask, '[data-part="time"]').textContent).toBe("5m");
    expect(within(ask).getByText("5m").tagName).toBe("TIME");
    // Its chat, what it waits on, then where: workspace, then persona. The project only among
    // others'.
    expect(part(ask, '[data-part="meta"]').textContent).toBe(
      "steward 3 · Asks your permission · alpha · steward",
    );
    expect(part(row(9), '[data-part="meta"]').textContent).toBe(
      "Waiting on your reply · Plane root",
    );
    const primary = part(ask, ".inbox-primary");
    expect(primary.textContent).toBe("Run cargo test");
    expect(part(ask, '[data-part="body"]').contains(primary)).toBe(true);
    expect(ask.getAttribute("aria-labelledby")).toBe(primary.id);
  });

  it("marks its state with a tinted glyph beside its words, never by colour alone (#5649)", () => {
    pickAsksScope(PLANE, "project");
    draw([permission(3, "a"), question(9)], strip("alpha"));
    const ask = row(3);
    expect(ask).toHaveAttribute("data-variant", "waiting");
    const mark = part(ask, '[data-part="mark"]');
    expect(mark.querySelector("svg")).not.toBeNull();
    expect(mark.getAttribute("aria-hidden")).toBe("true");
    expect(part(ask, '[data-part="meta"]').textContent).toContain("Asks your permission");
    expect(part(row(9), '[data-part="mark"]').querySelector("svg")).not.toBeNull();
  });

  it("cuts a long line to two with the whole of it in the tooltip", async () => {
    const long = `Should I ${"keep going with the migration ".repeat(20)}or stop?`;
    draw([{ ...question(9, long), source: "terminal", ask: "terminal:9" }], strip(ROOT));
    const primary = part(row(9), ".inbox-primary");
    expect(primary.classList.contains("inbox-clamped")).toBe(true);
    // The whole of it for a pointer: it only repeats the words, which the keyboard and a screen
    // reader have already, so no focus opens it.
    expect(await hoverTipOf(primary)).toBe(long);
    expect(tipOf(primary)).toBeNull();
    // The words are all there for a screen reader: only the drawing is cut.
    expect(primary.textContent).toBe(long);
  });

  it("never cuts the words of an ask it offers to allow", async () => {
    const long = `Run ${"cargo test --workspace ".repeat(20)}`.trim();
    draw([permission(3, "a", long)], strip("alpha"));
    const primary = part(row(3), ".inbox-primary");
    expect(primary.classList.contains("inbox-clamped")).toBe(false);
    expect(await hoverTipOf(primary)).toBe(long);
  });

  it("says a group's chat in the label style, and a section's name as a section title", () => {
    draw([permission(3, "a")], {
      ...strip("alpha"),
      updates: {
        rows: [
          {
            update: {
              key: "u1",
              kind: "task-done",
              says: "finished",
              chain: ["steward 3"],
              at: Date.now() / 1000,
              read: false,
              session: 3,
            },
            dismiss: () => undefined,
          },
        ],
        onMarkAllRead: () => undefined,
        onDismissAll: () => undefined,
      },
    });
    expect(screen.getByRole("heading", { name: "Updates" })).toHaveAttribute("data-part", "title");
    const update = part(document, '[role="row"].inbox-update');
    expect(update).toHaveAttribute("data-scope", "message");
    expect(update.querySelector('[data-part="time"] time')).not.toBeNull();
  });
});

describe("a row's ranks are the text roles (#1920)", () => {
  const part = (within: ParentNode, selector: string): HTMLElement => {
    const found = within.querySelector<HTMLElement>(selector);
    if (found === null) throw new Error(`nothing drawn matches ${selector}`);
    return found;
  };
  /** A core token as the value it stood for before it had a name, so what is asserted below
   *  is the declarations #1919 wrote, to the letter. */
  const AS_WRITTEN: Record<string, string> = {
    "var(--font-weight-strong)": "600",
    "var(--font-weight-regular)": "400",
    "var(--font-weight-medium)": "500",
  };
  /** What the window's stylesheets draw a word with: size, weight, colour. */
  const drawn = (word: Element) =>
    ["font-size", "font-weight", "color"].map((name) => {
      const value = drawnWith(word, name);
      return value === undefined ? undefined : (AS_WRITTEN[value] ?? value);
    });
  const update = (key: string, read: boolean) => ({
    update: {
      key,
      kind: "task-done" as const,
      says: `finished ${key}`,
      chain: ["steward 3"],
      at: Date.now() / 1000,
      read,
      session: 3,
    },
    dismiss: () => undefined,
  });

  it("draws each rank as the message row does: title, quiet meta, the words (#5649)", () => {
    draw([{ ...permission(3, "a"), since: Date.now() / 1000 - 5 * 60 }], {
      ...strip("alpha"),
      personaOf: () => "steward",
      updates: {
        rows: [update("u1", false), update("u2", true)],
        onMarkAllRead: () => undefined,
        onDismissAll: () => undefined,
      },
    });
    const ask = part(document, '.inbox-chat[data-session="3"] [role="row"].inbox-ask');
    const title = part(ask, '[data-part="title"]');
    const meta = part(ask, '[data-part="meta"]');
    const primary = part(ask, ".inbox-primary");
    // The chat's name strongest; what it waits on and where quiet; its words as they were.
    expect(drawn(title)).toEqual(["var(--font-size-base)", "600", "var(--text-primary)"]);
    expect(drawn(meta)).toEqual(["var(--font-size-sm)", undefined, "var(--text-quiet)"]);
    expect([...primary.classList]).toEqual(
      expect.arrayContaining(["ui-text-body", "ui-text-strong"]),
    );
    expect(drawn(primary)).toEqual(["var(--font-size-md)", "600", "var(--text-primary)"]);
    expect(drawn(part(ask, '[data-part="time"]'))).toEqual([
      "var(--font-size-sm)",
      undefined,
      "var(--text-quiet)",
    ]);
    // An ask's kind is its status, in the waiting amber (R132, #2917): the message family's own
    // `state` part, which its waiting variant colours.
    expect(drawnWith(part(meta, '[data-part="state"]'), "color")).toBe("var(--state-waiting)");

    const [fresh, read] = [...document.querySelectorAll('[role="row"].inbox-update')];
    expect(fresh.hasAttribute("data-read")).toBe(false);
    expect(read.hasAttribute("data-read")).toBe(true);
    expect(drawn(part(fresh, '[data-part="title"]'))).toEqual([
      "var(--font-size-base)",
      "600",
      "var(--text-primary)",
    ]);
    expect(drawn(part(fresh, ".inbox-unread")).slice(1)).toEqual(["600", "var(--text-primary)"]);
    // One read steps back: its title to the medium weight and the secondary text, its words to
    // the secondary text.
    expect(drawn(part(read, '[data-part="title"]'))).toEqual([
      "var(--font-size-base)",
      "500",
      "var(--text-secondary)",
    ]);
    expect(drawnWith(part(read, ".ask-says"), "color")).toBe("var(--text-secondary)");
  });

  it("leaves the placing to the row's own classes, which set no size, weight or colour", () => {
    const own = cascadeOf("App.css");
    document.body.innerHTML =
      '<span class="inbox-primary"></span><span class="inbox-secondary"></span><span class="inbox-meta"></span>';
    for (const word of document.body.children) {
      for (const name of ["font-size", "font-weight", "color"]) {
        expect(own(word, name), `${word.className} ${name}`).toBeUndefined();
      }
    }
    document.body.innerHTML = "";
  });
});

describe("a notification's landing", () => {
  it("widens a scope that leaves its chat out, and lands on it", async () => {
    draw(ASKS, { ...strip("alpha"), ...focus() });
    expect(chats()).toEqual(["steward 3"]);
    act(() => landOnGroupIn(PLANE, 5));
    await waitFor(() => expect(chats()).toEqual(["steward 3", "steward 5", "steward 9"]));
    await waitFor(() =>
      expect(document.activeElement?.closest(".inbox-chat")?.getAttribute("data-session")).toBe(
        "5",
      ),
    );
  });

  it("leaves the scope alone where it lists the chat", () => {
    draw(ASKS, { ...strip("alpha"), ...focus() });
    act(() => landOnGroupIn(PLANE, 3));
    expect(keptAsksScope(PLANE)).toBe("chat");
    expect(document.activeElement?.closest(".inbox-chat")?.getAttribute("data-session")).toBe("3");
  });
});
