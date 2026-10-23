// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cascadeOf, drawnWith } from "./cascade.testkit";
import { Inbox, NOT_READ_YET, NOTHING_WAITS, landOnGroupIn } from "./Inbox";
import { OTHER_PROJECTS, type Elsewhere } from "./InboxElsewhere";
import type { Needing } from "./NeedsYou";
import { forgetInbox } from "./inboxRules";
import { keepInboxScope, keptInboxScope } from "./inboxScope";
import { forgetProjectViews } from "./projectViews";
import type { Shown } from "./bindings";
import { hoverTipOf, tipOf } from "./ui/tooltip/tooltip.testkit";

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

/** Chat 3 works in alpha, chat 5 in beta, and chat 9 at the plane root. */
const FILED: Record<number, string> = { 3: "alpha", 5: "beta", 9: ROOT };
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
const chats = () => [...document.querySelectorAll(".inbox-chat > h3")].map((h) => h.textContent);

describe("the scope switch", () => {
  it("offers all projects, this project and this workspace, each with its count, on this project", () => {
    draw(ASKS, { ...strip("alpha"), elsewhere: elsewhere({ asking: [away] }) });
    expect(
      scopes()
        .getAllByRole("radio")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual([
      "All projects, 4 waiting on you",
      "This project, 3 waiting on you",
      "This workspace, 1 waiting on you",
    ]);
    expect(picked()).toBe("This project, 3 waiting on you");
    // The project's asks, and not the chat that waits in another window's project.
    expect(chats()).toEqual(["steward 3", "steward 5", "steward 9"]);
  });

  it("is the one segmented switch the Chats view draws", () => {
    draw(ASKS, strip("alpha"));
    const group = screen.getByRole("radiogroup", { name: "Show what needs you in" });
    expect(group.className).toBe("ui-levels ui-levels-small");
    expect(scopes().getAllByRole("radio")[0].className).toBe("ui-level");
  });

  it("offers only the scopes the window can list, and no switch for one", () => {
    const { unmount } = draw(ASKS);
    expect(screen.queryByRole("radiogroup")).toBeNull();
    expect(chats()).toEqual(["steward 3", "steward 5", "steward 9"]);
    unmount();
    draw(ASKS, strip("alpha"));
    expect(scopes().getAllByRole("radio")).toHaveLength(2);
  });

  it("lists this workspace's asks alone, and follows the strip", async () => {
    const { again } = draw(ASKS, strip("alpha"));
    await userEvent.click(scopes().getByRole("radio", { name: /^This workspace/ }));
    expect(chats()).toEqual(["steward 3"]);
    again(strip("beta"));
    expect(chats()).toEqual(["steward 5"]);
    expect(picked()).toBe("This workspace, 1 waiting on you");
  });

  it("lists the asks of no workspace at the plane root", async () => {
    draw(ASKS, strip(ROOT));
    await userEvent.click(scopes().getByRole("radio", { name: /^This workspace/ }));
    expect(chats()).toEqual(["steward 9"]);
  });

  it("remembers the pick for the project", async () => {
    const { unmount } = draw(ASKS, strip("alpha"));
    await userEvent.click(scopes().getByRole("radio", { name: /^This workspace/ }));
    expect(keptInboxScope(PLANE)).toBe("workspace");
    unmount();
    draw(ASKS, strip("alpha"));
    expect(picked()).toBe("This workspace, 1 waiting on you");
    expect(chats()).toEqual(["steward 3"]);
  });

  it("lists what waits in other projects under All projects", async () => {
    const there = elsewhere({
      asking: [away],
      waitingHere: [
        { plane: PLANE, project: "plane", count: 3 },
        {
          plane: "/home/dev/web",
          project: "web",
          count: 2,
          asks: [
            { session: 2, ask: "x", says: "Run npm test", source: "permission", chain: ["dev 2"] },
            { session: 4, ask: "y", says: "Which port?", source: "question", chain: ["dev 4"] },
          ],
        },
      ],
    });
    draw(ASKS, { ...strip("alpha"), elsewhere: there });
    expect(scopes().getByRole("radio", { name: "All projects, 6 waiting on you" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: OTHER_PROJECTS })).toBeNull();

    await userEvent.click(scopes().getByRole("radio", { name: /^All projects/ }));
    expect(screen.getByRole("region", { name: "steward 7 · infra · ops" })).toBeTruthy();
    const others = screen.getByRole("region", { name: OTHER_PROJECTS });
    // Each ask of the other project, read here and answered there.
    expect(within(others).getByText("Run npm test")).toBeTruthy();
    expect(within(others).getByText("Which port?")).toBeTruthy();
    expect(
      within(others)
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual(["Go to project"]);
    await userEvent.click(within(others).getByRole("button", { name: "Go to project web" }));
    expect(there.onOpen).toHaveBeenLastCalledWith("/home/dev/web");
    // This project's own asks say which project they are in, beside the others'.
    expect(document.querySelector('.inbox-chat[data-session="3"] .inbox-where')?.textContent).toBe(
      "plane · alpha",
    );
  });
});

describe("an empty scope", () => {
  it("says how many wait in the wider scope, and widens in one press", async () => {
    keepInboxScope(PLANE, "workspace");
    draw([permission(5, "b"), question(9)], strip("alpha"));
    expect(chats()).toEqual([]);
    expect(screen.getByText("Nothing is waiting on you in this workspace.")).toBeTruthy();
    expect(screen.getByText("2 wait in this project.")).toBeTruthy();
    expect(screen.queryByText(NOTHING_WAITS)).toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Show this project" }));
    expect(chats()).toEqual(["steward 5", "steward 9"]);
    expect(keptInboxScope(PLANE)).toBe("project");
  });

  it("goes past an empty project to all projects", async () => {
    keepInboxScope(PLANE, "workspace");
    draw([], { ...strip("alpha"), elsewhere: elsewhere({ asking: [away] }) });
    expect(screen.getByText("1 waits in all projects.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Show all projects" }));
    expect(screen.getByRole("region", { name: "steward 7 · infra · ops" })).toBeTruthy();
  });

  it("offers what the window knows waits wider while its own asks are not read yet", async () => {
    draw(undefined, { ...strip("alpha"), elsewhere: elsewhere({ asking: [away] }) });
    expect(screen.getByText(NOT_READ_YET)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Show all projects" }));
    expect(screen.getByRole("region", { name: "steward 7 · infra · ops" })).toBeTruthy();
    expect(screen.getByText(NOT_READ_YET)).toBeTruthy();
  });

  it("is the Inbox's own empty sentence where no scope lists anything", () => {
    keepInboxScope(PLANE, "workspace");
    draw([], { ...strip("alpha"), elsewhere: elsewhere() });
    expect(screen.getByText(NOTHING_WAITS)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Show / })).toBeNull();
    // Nothing to narrow, so no switch: as the Chats view draws none over an empty list.
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
    part(document, `.inbox-chat[data-session="${session}"] li.inbox-ask`);

  it("has one primary line, a secondary line of where, and a trailing kind and age", () => {
    draw([{ ...permission(3, "a"), since: Date.now() / 1000 - 5 * 60 }, question(9)], {
      ...strip("alpha"),
      personaOf: (session) => (session === 3 ? "steward" : null),
    });
    const ask = row(3);
    const primary = part(ask, ".inbox-primary");
    expect(primary.textContent).toBe("Run cargo test");
    expect(ask.getAttribute("aria-labelledby")).toBe(primary.id);
    // Where: workspace, then persona. The project is said only among other projects.
    const where = part(ask, ".inbox-secondary");
    expect(where.textContent).toBe("alpha · steward");
    expect(part(row(9), ".inbox-secondary").textContent).toBe("Plane root");
    // Kind and age, last in the row's head.
    const meta = part(ask, ".inbox-meta");
    expect(meta.textContent).toBe("Asks your permission · 5m");
    expect(part(ask, ".inbox-says").lastElementChild).toBe(meta);
    expect(within(ask).getByText("5m").tagName).toBe("TIME");
  });

  it("marks the kind with an icon beside its words, never by colour alone", () => {
    draw([permission(3, "a"), question(9)], strip("alpha"));
    const decision = part(row(3), ".inbox-kind");
    expect(decision.getAttribute("data-kind")).toBe("decision");
    expect(decision.querySelector("svg")).not.toBeNull();
    expect(decision.getAttribute("aria-hidden")).toBe("true");
    expect(part(row(3), ".inbox-meta").textContent).toContain("Asks your permission");
    expect(part(row(9), ".inbox-kind").getAttribute("data-kind")).toBe("reply");
  });

  it("cuts a long line to two with the whole of it in the tooltip", async () => {
    const long = `Should I ${"keep going with the migration ".repeat(20)}or stop?`;
    draw([question(9, long)], strip(ROOT));
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
    const update = part(document, "li.inbox-update");
    expect(update.querySelector(".inbox-primary")).not.toBeNull();
    expect(update.querySelector(".inbox-meta time")).not.toBeNull();
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

  it("draws each rank with the size, weight and colour it had before the roles", () => {
    draw([{ ...permission(3, "a"), since: Date.now() / 1000 - 5 * 60 }], {
      ...strip("alpha"),
      personaOf: () => "steward",
      updates: {
        rows: [update("u1", false), update("u2", true)],
        onMarkAllRead: () => undefined,
        onDismissAll: () => undefined,
      },
    });
    const ask = part(document, '.inbox-chat[data-session="3"] li.inbox-ask');
    const primary = part(ask, ".inbox-primary");
    const where = part(ask, ".inbox-secondary");
    const meta = part(ask, ".inbox-meta");
    expect([...primary.classList]).toEqual(
      expect.arrayContaining(["ui-text-body", "ui-text-strong"]),
    );
    expect(where.classList).toContain("ui-text-secondary");
    expect(meta.classList).toContain("ui-text-meta");
    // #1919's declarations: md / 600 / primary; sm / secondary; xs / quiet (#2902).
    expect(drawn(primary)).toEqual(["var(--font-size-md)", "600", "var(--text-primary)"]);
    expect(drawn(where)).toEqual(["var(--font-size-sm)", "400", "var(--text-secondary)"]);
    expect(drawn(meta)).toEqual(["var(--font-size-xs)", "400", "var(--text-quiet)"]);
    // The kind's words inside the meta are the meta's, by inheritance.
    expect(drawnWith(part(meta, ".inbox-source"), "font-size")).toBe("inherit");
    expect(drawnWith(part(meta, ".inbox-source"), "color")).toBe("inherit");

    const [fresh, read] = [...document.querySelectorAll("li.inbox-update")];
    expect(fresh.hasAttribute("data-read")).toBe(false);
    expect(read.hasAttribute("data-read")).toBe(true);
    expect(drawn(part(fresh, ".inbox-primary"))).toEqual([
      "var(--font-size-md)",
      "600",
      "var(--text-primary)",
    ]);
    expect(drawn(part(fresh, ".inbox-meta"))).toEqual([
      "var(--font-size-xs)",
      "400",
      "var(--text-quiet)",
    ]);
    expect(drawn(part(fresh, ".inbox-unread")).slice(1)).toEqual(["600", "var(--text-primary)"]);
    // One read steps back to the regular weight and the secondary text, at the same size: no
    // rank of the scale, so it wears no role and a role cannot outrank its rule.
    const quiet = part(read, ".inbox-primary");
    expect([...quiet.classList].filter((name) => name.startsWith("ui-text-"))).toEqual([]);
    expect(drawn(quiet)).toEqual(["var(--font-size-md)", "400", "var(--text-secondary)"]);
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
    keepInboxScope(PLANE, "workspace");
    draw(ASKS, strip("alpha"));
    expect(chats()).toEqual(["steward 3"]);
    act(() => landOnGroupIn(PLANE, 5));
    expect(chats()).toEqual(["steward 3", "steward 5", "steward 9"]);
    await waitFor(() =>
      expect(document.activeElement?.closest(".inbox-chat")?.getAttribute("data-session")).toBe(
        "5",
      ),
    );
  });

  it("leaves the scope alone where it lists the chat", () => {
    keepInboxScope(PLANE, "workspace");
    draw(ASKS, strip("alpha"));
    act(() => landOnGroupIn(PLANE, 3));
    expect(keptInboxScope(PLANE)).toBe("workspace");
    expect(document.activeElement?.closest(".inbox-chat")?.getAttribute("data-session")).toBe("3");
  });
});
