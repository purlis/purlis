// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { Moved } from "./bindings";
import { onTheWire } from "./chatEvents.testkit";
import { aTabForEachChat } from "./laidOut.testkit";
import { placementFor } from "./ui/tooltip/Tooltip";
import { setSkillStudioOn } from "./skillStudio";
import { findRow, findTree } from "./ui/tree/tree.testkit";

/**
 * **A chat's pane head, against the whole window** (#4760, spec #4750 stories 54, 55, 60 and
 * 61; the accepted prototype `prototype/pane-chat-actions`, variant A).
 *
 * The head shows the chat at a glance (its name, its state and its sandbox) and then, at its
 * end, two groups (#5203, spec #5191): the chat's actions, its one state button, the bell and ⋯
 * last, then, after a thin divider, the pane's controls, Split right, Split down and Close last.
 * Its right-click is the chat's list, then the pane's rows. A narrow pane drops its parts in the
 * ruled order (`paneHead.ts`), and the bell, ⋯, Split right and Close never go.
 *
 * **The pane foot** (#5202, spec #5191, look A), under the terminal, says where the chat runs,
 * harness · workspace, and its gauge: `ctx` with its fill as a thin bar under the number, then
 * `cache`. They left the head, with the row of gauge bars.
 */

/** What the stand-in terminal was asked to do from its pane's head (#4760). */
const ran: string[] = [];

vi.mock("./SessionPane", async () => {
  const { useContext, useEffect } = await import("react");
  const { PaneTerminalAt } = await import("./paneTerminal");
  const { listsLinks } = await import("./paneLinksOpen");
  return {
    // A terminal with something selected, which says to its pane's head what it can do.
    SessionPane: ({ plane, session }: { plane: string; session: number }) => {
      const at = useContext(PaneTerminalAt);
      // Its Links in this pane… (#6332), which the window's rows open.
      useEffect(
        () => listsLinks(plane, session, () => ran.push(`links:${session}`)),
        [plane, session],
      );
      useEffect(() => {
        if (at === null) return;
        at.current = { selected: () => true, run: (id) => ran.push(id) };
        return () => {
          at.current = null;
        };
      }, [at]);
      return <div data-testid="pane">session {session}</div>;
    },
  };
});

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  usage = null;
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT = {
  session: 4,
  name: "claude 4",
  cwd: `${ALPHA}/repo`,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
  sandbox: "floor",
  sandbox_why: null,
  browser_sites: null,
};

/** A plain shell in the same place: no harness and no profile (`isShell`). */
const SHELL = { ...CHAT, name: "zsh", harness: null, profile: null };

/** Chat 4 mid-turn, then waiting for you: what the board says as its turn runs and ends. */
const RUNNING: Moved = {
  plane: PLANE,
  session: 4,
  state: "running",
  needs_you: false,
  queue: [],
  moved_at: 1,
  reports: [],
  refusals: [],
  children: [],
  sequence: 1,
} as unknown as Moved;
const WAITING = { ...RUNNING, state: "waiting", moved_at: 2, sequence: 2 } as Moved;

/** What the core says of chat 4's usage: none by default, as before its first turn. */
let usage: unknown = null;

function core(chat: typeof CHAT | typeof SHELL = CHAT, listed: Promise<void> = Promise.resolve()) {
  mockIPC(
    (cmd, args) => {
      if (cmd === "chat_usage") return usage;
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [chat];
      // The chat in a tab of its own on alpha, as the stored layout keeps it (#4758).
      if (cmd === "reopened_layout") return aTabForEachChat([chat], () => "alpha");
      if (cmd === "plane_sidebar")
        return listed.then(() => ({
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [chat] }],
        }));
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      if (cmd === "asks_waiting") return { plane: PLANE, asks: [] };
      if (cmd === "mcp_failures") return [];
      if (cmd === "smart_closing") return [];
      if (cmd === "set_skill_studio_on") return (args as { on: boolean }).on;
      return null;
    },
    { shouldMockEvents: true },
  );
}

async function aPane(chat: typeof CHAT | typeof SHELL = CHAT) {
  core(chat);
  render(<App />);
  await screen.findByTestId("pane");
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** The chat's name, as the window shows it: its profile's word before its own. */
const NAME = "claude claude 4";

const frame = () => screen.getByTestId("pane").closest(".pane-frame") as HTMLElement;
const head = () => frame().querySelector(".pane-head") as HTMLElement;
const foot = () =>
  frame().querySelector('[data-scope="pane-foot"][data-part="root"]') as HTMLElement;
/** The head's controls, start to end, by their names: what a screen reader tabs through. */
const controls = () =>
  [...head().querySelectorAll(".pane-actions button")].map(
    (one) => one.getAttribute("aria-label") ?? one.textContent ?? "",
  );

describe("a chat's pane head", () => {
  it("names the chat and says its sandbox, and leaves harness · workspace to the foot", async () => {
    await aPane();

    await waitFor(() => expect(within(head()).getByText(NAME)).toHaveClass("pane-name"));
    // Led by the chat's persona and state icon, the one its row and tab wear (#4762).
    const line = head().querySelector(".pane-chips") as HTMLElement;
    expect(line.firstElementChild).toHaveClass("pane-persona");
    expect(line.firstElementChild?.querySelector('[data-scope="chat-mark"]')).not.toBeNull();
    expect(head().querySelector(".pane-where")).toBeNull();
    expect(within(head()).queryByText("alpha")).toBeNull();
    // The badge moved from the tab to the head (#4760) and on to the foot (#5204): named for a
    // screen reader.
    expect(within(head()).queryByRole("img", { name: "floor" })).toBeNull();
    expect(within(foot()).getByRole("img", { name: "floor" })).toBeInTheDocument();
    const strip = document.querySelector('[data-strip="Tabs"]') as HTMLElement;
    expect(strip.querySelector(".tab-sandbox")).toBeNull();
  });

  it("and its row in the Chats list says the same sandbox (#4760)", async () => {
    await aPane();

    const tree = await findTree("Chats of this project");
    const row = await findRow(tree, new RegExp(`^${NAME}`));
    expect(within(row).getByRole("img", { name: "floor" })).toBeInTheDocument();
  });

  it("ends with the bell and ⋯, a divider, then Split right and Split down, its tab's only pane having no Close (#5203, #5318)", async () => {
    await aPane();

    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));
    const [bell, more, right, down, ...rest] = controls();
    expect(bell).toMatch(/^Pane notices:/);
    expect(more).toBe(`More for ${NAME}`);
    expect([right, down]).toEqual(["Split right", "Split down"]);
    expect(rest).toEqual([]);
    // The thin divider parts the chat's actions from the pane's controls, unheard and untabbed.
    const actions = [
      ...head().querySelectorAll('.pane-actions button, .pane-actions [data-scope="separator"]'),
    ];
    const line = actions.findIndex((one) => one.getAttribute("data-scope") === "separator");
    expect(actions[line - 1]).toHaveAccessibleName(`More for ${NAME}`);
    expect(actions[line + 1]).toHaveAccessibleName("Split right");
    expect(actions[line]).toHaveAttribute("aria-hidden", "true");
  });

  it("draws the bell as an icon button, and opens every head hint below the head (#5189)", async () => {
    await aPane();
    await waitFor(() => expect(controls()[0]).toMatch(/^Pane notices:/));

    const bell = within(head()).getByRole("button", { name: /^Pane notices:/ });
    // The size of Split right beside it: an icon button, as wide as it is tall.
    expect(bell).toHaveAttribute("data-icon-only");
    expect(bell).toHaveAttribute("data-size", "sm");
    // Below the head: a hint at a control's end stood on the row, over the next control.
    for (const one of within(head()).getAllByRole("button"))
      expect(placementFor(one)).toBe("bottom");
  });

  it("shows Stop while the chat works, first of the chat's actions, and nothing once it is quiet", async () => {
    await aPane();
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));

    await act(() => emit(...onTheWire("chat-moved", RUNNING)));
    const stop = await within(head()).findByRole("button", { name: `Stop chat ${NAME}` });
    expect(stop).toHaveTextContent("Stop");
    expect(controls()[0]).toBe(`Stop chat ${NAME}`);
    expect(controls()[1]).toMatch(/^Pane notices:/);
    expect(controls()[2]).toBe(`More for ${NAME}`);

    await act(() => emit(...onTheWire("chat-moved", WAITING)));
    await waitFor(() =>
      expect(within(head()).queryByRole("button", { name: `Stop chat ${NAME}` })).toBeNull(),
    );
    expect(controls()[1]).toBe(`More for ${NAME}`);
  });

  it("opens the chat's list, then a line, then the pane's rows, on a right-click", async () => {
    await aPane();
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));

    fireEvent.contextMenu(head().querySelector(".pane-chips") as HTMLElement);
    const menu = await screen.findByRole("menu");
    const items = [...menu.querySelectorAll('[role^="menuitem"]')].map(
      (one) => one.getAttribute("aria-label") ?? one.textContent,
    );
    // The chat's own rows first: Rename… is its list's, and End… ends its group.
    const rename = items.findIndex((one) => one?.startsWith("Rename"));
    const end = items.findIndex((one) => one?.startsWith("End…"));
    const split = items.indexOf("Split right");
    expect(rename).toBeGreaterThanOrEqual(0);
    expect(end).toBeGreaterThan(rename);
    expect(split).toBeGreaterThan(end);
    expect(items.slice(split)).toEqual(
      expect.arrayContaining(["Split right", "Split down", "Send F2 to the chat in front"]),
    );
    // Nothing of a tab's and nothing of the window's.
    expect(items).not.toContain("Pinned");
    expect(items).not.toContain("New tab");
  });

  it("puts the terminal's Copy and Paste among the pane's rows, before Send F2, and runs them on it", async () => {
    await aPane();
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));
    ran.length = 0;

    fireEvent.contextMenu(head().querySelector(".pane-chips") as HTMLElement);
    const menu = await screen.findByRole("menu");
    const items = [...menu.querySelectorAll('[role^="menuitem"]')].map(
      (one) => one.getAttribute("aria-label") ?? one.textContent,
    );
    const split = items.indexOf("Split right");
    expect(items.slice(split)).toEqual(
      expect.arrayContaining(["Copy", "Paste", "Send F2 to the chat in front"]),
    );
    expect(items.indexOf("Copy")).toBeGreaterThan(items.indexOf("Split down"));
    expect(items.indexOf("Paste")).toBe(items.indexOf("Copy") + 1);
    expect(items.indexOf("Send F2 to the chat in front")).toBeGreaterThan(items.indexOf("Paste"));
    // Not Select all: the terminal's own right-click keeps that.
    expect(items).not.toContain("Select all");

    fireEvent.click(within(menu).getByRole("menuitem", { name: "Copy" }));
    await waitFor(() => expect(ran).toEqual(["terminal.copy"]));
  });
});

describe("Links in this pane… from the pane's head, its ⋯ and the palette (#6332)", () => {
  /** The row, under the list's Open ▸ (the list is ten rows at most), and the key it says. */
  const linksRow = async (menu: HTMLElement) => {
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Open" }));
    const menus = await screen.findAllByRole("menu");
    const open = menus[menus.length - 1];
    const row = await within(open).findByRole("menuitem", { name: "Links in this pane…" });
    // The key is the registry's `pane.links`, drawn and said (Ctrl+Shift+L off a Mac).
    expect(row).toHaveAttribute("aria-keyshortcuts", "Control+Shift+L");
    expect(row.querySelector('[data-part="shortcut"]')).toHaveAttribute("data-key", "Ctrl+Shift+L");
    return row;
  };

  it("is on the head's right-click, and opens this pane's list", async () => {
    await aPane();
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));
    ran.length = 0;
    fireEvent.contextMenu(head().querySelector(".pane-chips") as HTMLElement);
    fireEvent.click(await linksRow(await screen.findByRole("menu")));
    await waitFor(() => expect(ran).toEqual(["links:4"]));
  });

  it("is on the head's ⋯, and opens this pane's list", async () => {
    await aPane();
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));
    ran.length = 0;
    fireEvent.click(within(head()).getByRole("button", { name: `More for ${NAME}` }));
    fireEvent.click(await linksRow(await screen.findByRole("menu")));
    await waitFor(() => expect(ran).toEqual(["links:4"]));
  });

  it("is on a shell's pane too, whose links are a pane's as a chat's are", async () => {
    await aPane(SHELL);
    await waitFor(() => expect(controls()).toContainEqual(expect.stringMatching(/^More for /)));
    ran.length = 0;
    fireEvent.contextMenu(head().querySelector(".pane-chips") as HTMLElement);
    fireEvent.click(await linksRow(await screen.findByRole("menu")));
    await waitFor(() => expect(ran).toEqual(["links:4"]));
  });

  it("is a palette row, named for the pane's chat", async () => {
    await aPane();
    ran.length = 0;
    fireEvent.keyDown(window, { key: "F2" });
    const dialog = await screen.findByRole("dialog", { name: "Command palette" });
    fireEvent.change(within(dialog).getByRole("combobox"), {
      target: { value: "links in" },
    });
    const list = await within(dialog).findByRole("listbox", { name: "Actions" });
    fireEvent.click(await within(list).findByRole("option", { name: /^Links in .*claude 4…/ }));
    await waitFor(() => expect(ran).toEqual(["links:4"]));
  });
});

describe("a chat wrapping up (#4760)", () => {
  /** The core telling the window a step of chat 4's smart close. */
  const step = (phase: string) =>
    act(() => emit("smart-close", { plane: PLANE, session: 4, phase, record: null }));

  it("keeps its pane, whose head breathes and offers Cancel smart close, and turns no tab into a chip", async () => {
    await aPane();
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));

    await step("sent");

    await waitFor(() =>
      expect(head().querySelector('[data-mark="wrapping-up"]')).toHaveClass("breathing"),
    );
    // The head's one state button, first of its actions, and the pane is still the chat's.
    expect(controls()[0]).toBe(`Cancel smart close of ${NAME}`);
    expect(screen.getByTestId("pane")).toHaveTextContent("session 4");
    // The strip's chip is gone (#4760): the chat's tab stays a tab, with its name.
    const strip = document.querySelector('[data-strip="Tabs"]') as HTMLElement;
    expect(strip.querySelector(".tab[data-chip]")).toBeNull();
    expect([...strip.querySelectorAll(".tab-name")].map((name) => name.textContent)).toContain(
      NAME,
    );

    await step("cancelled");
    await waitFor(() => expect(head().querySelector('[data-mark="wrapping-up"]')).toBeNull());
    expect(controls()[1]).toBe(`More for ${NAME}`);
  });

  it("breathes on its row in the Chats list, and the mark goes when it is cancelled", async () => {
    await aPane();
    const tree = await findTree("Chats of this project");
    const row = await findRow(tree, new RegExp(`^${NAME}`));
    expect(row.querySelector('[data-mark="wrapping-up"]')).toBeNull();

    await step("sent");

    await waitFor(() => expect(row.querySelector('[data-mark="wrapping-up"]')).not.toBeNull());
    expect(row.querySelector('[data-mark="wrapping-up"]')).toHaveClass("breathing");

    await step("cancelled");
    await waitFor(() => expect(row.querySelector('[data-mark="wrapping-up"]')).toBeNull());
  });
});

describe("what a chat uses now, on its pane head (#5524)", () => {
  it("draws the skill run open in the chat on its pane head, and none on its tab", async () => {
    mockIPC(
      (cmd, args) => {
        if (cmd === "skill_studio_on") return true;
        if (cmd === "set_skill_studio_on") return (args as { on: boolean }).on;
        if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
        if (cmd === "opened_chats") return [CHAT];
        if (cmd === "reopened_layout") return aTabForEachChat([CHAT], () => "alpha");
        if (cmd === "plane_sidebar")
          return {
            root: PLANE,
            personas: [],
            persona: null,
            unfiled: [],
            workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [CHAT] }],
          };
        if (cmd === "asks_waiting") return { plane: PLANE, asks: [] };
        return null;
      },
      { shouldMockEvents: true },
    );
    // The studio is on: what the window heard of it before this test is kept (`skillStudio.ts`).
    await setSkillStudioOn(true);
    render(<App />);
    await screen.findByTestId("pane");

    await act(() =>
      emit(
        ...onTheWire("chat-skill-run", {
          plane: PLANE,
          session: 4,
          run: {
            skill: "deploy",
            name: "deploy",
            id: "01RUN",
            tool_calls: 2,
            tokens: null,
            level: null,
            copy_workspace: null,
          },
        }),
      ),
    );

    const chip = await screen.findByTestId("pane-skill-run");
    expect(head()).toContainElement(chip);
    expect(chip.closest('[data-strip="Tabs"]')).toBeNull();
    await setSkillStudioOn(false);
  });
});

describe("a narrow pane's head", () => {
  /** How wide every pane is now, and what is told when that changes. */
  let width = 0;
  let resized: (() => void)[] = [];
  afterEach(() => {
    resized = [];
  });

  /** Makes every pane `to` pixels wide, telling whatever watches their size. */
  async function narrowedTo(to: number) {
    width = to;
    await act(async () => {
      for (const tell of resized) tell();
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }

  /** Draws the window with every pane `at` pixels wide, as the layout engine would. */
  async function aPaneOf(at: number) {
    width = at;
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(private readonly told: (entries: unknown[], seen: unknown) => void) {}
        observe() {
          resized.push(() => this.told([], this));
          this.told([], this);
        }
        disconnect() {}
        unobserve() {}
      },
    );
    const real = HTMLElement.prototype.getBoundingClientRect;
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
      this: HTMLElement,
    ) {
      return this.classList.contains("pane-frame")
        ? ({
            width,
            height: 400,
            top: 0,
            left: 0,
            right: width,
            bottom: 400,
            x: 0,
            y: 0,
          } as DOMRect)
        : real.call(this);
    });
    await aPane();
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));
  }

  it("keeps every part in a pane 520px wide", async () => {
    await aPaneOf(520);

    expect(within(foot()).getByRole("img", { name: "floor" })).toBeInTheDocument();
    expect(controls()).toContain("Split down");
  });

  it("keeps the bell, ⋯, Split right and Close as it narrows", async () => {
    await aPaneOf(500);

    expect(controls()).toContain("Split down");
    // Harness · workspace is the foot's, which drops nothing here (#5206 orders its drops).
    expect(foot().querySelector(".pane-where")).toHaveTextContent(/^claude\s*·\s*alpha$/);
  });

  it("drops Split down last, and still keeps the bell, ⋯ and Split right", async () => {
    await aPaneOf(280);

    const [bell, more, right, ...rest] = controls();
    expect(bell).toMatch(/^Pane notices:/);
    expect(more).toBe(`More for ${NAME}`);
    expect(right).toBe("Split right");
    expect(rest).toEqual([]);
  });

  describe("what the chat uses now (#5614)", () => {
    afterEach(async () => {
      await setSkillStudioOn(false);
    });

    /** Chat 4 has the `deploy` skill's run open, with the studio on to say it. */
    async function aSkillRunOpen() {
      await setSkillStudioOn(true);
      await act(() =>
        emit(
          ...onTheWire("chat-skill-run", {
            plane: PLANE,
            session: 4,
            run: {
              skill: "deploy",
              name: "deploy",
              id: "01RUN",
              tool_calls: 2,
              tokens: null,
              level: null,
              copy_workspace: null,
            },
          }),
        ),
      );
    }

    it("keeps its skill run while the head has room for it, and drops it next after the state's word", async () => {
      await aPaneOf(450);
      await aSkillRunOpen();
      expect(await within(head()).findByTestId("pane-skill-run")).toBeInTheDocument();

      await narrowedTo(449);

      await waitFor(() => expect(head().querySelector(".pane-using")).toBeNull());
      expect(within(head()).queryByTestId("pane-skill-run")).toBeNull();
      // The chat's name and its mark stay: under 470 the mark at the head's start says the state.
      expect(within(head()).getByText(NAME)).toHaveClass("pane-name");
      expect(head().querySelector('.pane-persona [data-scope="chat-mark"]')).not.toBeNull();
    });
  });

  /** The glyphs drawn on the head's chat side, by their lucide names, in order. */
  const glyphs = () =>
    [...head().querySelectorAll(".pane-chips svg")].map(
      (one) => [...one.classList].find((name) => /^lucide-(?!icon$)/.test(name)) ?? "?",
    );

  it.each([
    ["with its mark and no word", 400],
    ["with no state of its own", 360],
  ])("draws each state's glyph once, %s (#5446)", async (_, width) => {
    await aPaneOf(width);

    for (const moved of [RUNNING, WAITING]) {
      await act(() => emit(...onTheWire("chat-moved", moved)));
      await waitFor(() =>
        expect(head().querySelector('[data-scope="chat-mark"][data-part="badge"]')).not.toBeNull(),
      );
      const drawn = glyphs();
      expect(drawn).toEqual([...new Set(drawn)]);
    }
  });
});

describe("a chat's pane foot (#5202)", () => {
  const USAGE = {
    context: { value: 62, tone: "ok" },
    cache: { value: 91, tone: "ok" },
    rebuilds: null,
    turns: [
      { cache: { value: 90, tone: "ok" }, context: null, written: null },
      { cache: { value: 91, tone: "ok" }, context: null, written: null },
    ],
    cold: null,
  };

  it("is drawn under the terminal, last in the pane, so Tab reaches it after the terminal", async () => {
    await aPane();

    await waitFor(() => expect(foot()).not.toBeNull());
    expect(frame().lastElementChild).toBe(foot());
    const body = frame().querySelector(".pane-body") as HTMLElement;
    expect(body.compareDocumentPosition(foot()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(head().contains(foot())).toBe(false);
  });

  it("says harness · workspace at its start", async () => {
    await aPane();

    await waitFor(() =>
      expect(foot().querySelector(".pane-where")).toHaveTextContent(/^claude\s*·\s*alpha$/),
    );
    expect(foot().querySelector('[data-part="start"] .pane-where')).not.toBeNull();
  });

  it("is drawn before the chat's first usage arrives, with no gauge, so the terminal never moves for it", async () => {
    await aPane();

    await waitFor(() => expect(foot()).not.toBeNull());
    expect(within(foot()).queryByTestId("chat-gauge")).toBeNull();
    expect(foot().querySelector('[data-part="end"]')).not.toBeNull();
  });

  it("ends with ctx, its fill a bar under its number, then cache; the head has neither, and no row of bars is drawn", async () => {
    usage = USAGE;
    await aPane();

    const gauge = await within(foot()).findByTestId("chat-gauge");
    expect(gauge.closest('[data-part="end"]')).not.toBeNull();
    const parts = [...gauge.querySelectorAll('[data-part="gauge"]')].map((one) =>
      one.textContent?.replace(/\s+/g, " ").trim(),
    );
    expect(parts).toEqual(["ctx 62%", "cache 91%"]);
    const fill = gauge.querySelector('[data-part="fill"]') as HTMLElement;
    expect(fill.closest('[data-part="gauge"]')?.textContent).toContain("ctx");
    expect(fill.style.inlineSize).toBe("62%");
    expect(head().querySelector(".chat-gauge")).toBeNull();
    expect(within(head()).queryByText("ctx")).toBeNull();
    expect(document.querySelector('[data-testid="gauge-trend"]')).toBeNull();
  });

  it("is drawn on no shell's pane, whose head keeps its workspace (spec #5191, Out of Scope)", async () => {
    await aPane(SHELL);

    await waitFor(() =>
      expect(head().querySelector(".pane-head-where")).toHaveTextContent("alpha"),
    );
    expect(foot()).toBeNull();
  });

  /** The window with the plane's listing held back, and what lets it through. */
  async function beforeTheListing(chat: typeof CHAT | typeof SHELL) {
    let list = () => {};
    core(
      chat,
      new Promise<void>((resolve) => {
        list = resolve;
      }),
    );
    render(<App />);
    await screen.findByTestId("pane");
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    return () => act(async () => list());
  }

  it("is there from a chat's first frame, so its terminal gets one size from the start (story 46)", async () => {
    const listed = await beforeTheListing(CHAT);
    const reserved = foot();
    expect(reserved).not.toBeNull();

    await listed();
    // The listing fills the foot in: the same line, and nothing rows the terminal again.
    await waitFor(() => expect(reserved.querySelector(".pane-where")).not.toBeNull());
    expect(foot()).toBe(reserved);
  });

  it("is never drawn on a shell, listed or not, as the open chats say what it is (story 20)", async () => {
    const listed = await beforeTheListing(SHELL);
    expect(foot()).toBeNull();

    await listed();
    await waitFor(() => expect(head().querySelector(".pane-head-where")).not.toBeNull());
    expect(foot()).toBeNull();
  });
});
