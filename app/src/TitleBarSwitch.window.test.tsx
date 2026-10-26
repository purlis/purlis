// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { onAMac } from "./tabKeys";
import { forgetRailMode, RAIL_MODE } from "./railMode";
import { forgetTheSlide } from "./railSlide";
import { FRAME, MOMENTUM, played, STROKE, type Traced } from "./swipeTraces.testkit";

/**
 * **The project in front and its workspaces in the title bar** (#2916, spec #2897 R90–R115), on
 * the real window with the core mocked: the chips of the project's pins and the dashed one of an
 * unpinned current workspace, the ⌄'s popover, ⌘P, ⌘1–⌘9, the one-step swipe, the two-column
 * switcher of a hidden rail, and choosing a project landing on the workspace it was last on.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

/** What the workspace slide is asked to play: the Web Animations jsdom has not got (R114). */
const slid = vi.hoisted(() => ({ calls: [] as { parts: HTMLElement | null; step: number }[] }));
vi.mock("./workspaceSlide", async (original) => {
  const real = await original<typeof import("./workspaceSlide")>();
  return {
    ...real,
    slideIn: (parts: HTMLElement | null, step: 1 | -1) => slid.calls.push({ parts, step }),
  };
});

afterEach(() => {
  slid.calls = [];
  cleanup();
  clearMocks();
  forgetRailMode();
  forgetTheSlide();
});

const ONE = "/home/dev/one";
const TWO = "/home/dev/two";
const NAMES = ["alpha", "beta", "gamma"];

/** A chat in `workspace` of `plane`. */
function chat(plane: string, session: number, workspace: string) {
  return {
    session,
    name: `${workspace}.${session}`,
    cwd: `${plane}/workspaces/${workspace}`,
    harness: null,
    in_front: session === 1,
    resumed: null,
    fresh: null,
    profile: null,
    persona: null,
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
  };
}

/**
 * Two projects, `one` in front, each with three workspaces and one chat in `alpha`; `one` has
 * `alpha` and `gamma` pinned, in that order. The store keeps a pin written, as the core does.
 */
function core() {
  const pinned = new Map<string, string[]>([
    [ONE, ["alpha", "gamma"]],
    [TWO, []],
  ]);
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    const plane = (given.plane as string | undefined) ?? "";
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore")
      return { windows: [{ planes: [ONE, TWO], active: 0 }], dropped: [] };
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "plane_sidebar")
      return {
        root: plane,
        workspaces: NAMES.map((name) => ({
          name,
          path: `${plane}/workspaces/${name}`,
          vision: "",
          todos: [],
          chats: name === "alpha" ? [chat(plane, 1, "alpha")] : [],
        })),
        personas: [],
        persona: null,
        unfiled: [],
      };
    if (cmd === "opened_chats") return [chat(plane, 1, "alpha")];
    if (cmd === "plane_pins") {
      const order = pinned.get(plane) ?? [];
      return { project: false, workspaces: order, missing: [], order, certain: true };
    }
    if (cmd === "pin_workspace") {
      const was = pinned.get(plane) ?? [];
      const name = given.workspace as string;
      pinned.set(plane, given.pinned ? [...was, name] : was.filter((one) => one !== name));
      return null;
    }
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    return null;
  });
  return { asked, pinned };
}

const bar = () => screen.getByTestId("title-bar");
/** The strip of pins, by its role and name; found in the DOM while a dialog hides the rest. */
const pins = () =>
  within(bar()).queryByRole("toolbar", { name: "Pinned workspaces" }) ??
  (bar().querySelector<HTMLElement>(
    '[role="toolbar"][aria-label="Pinned workspaces"]',
  ) as HTMLElement);
/** The chips' names, `*` on the current one, `+` on the dashed one. */
const chips = () =>
  [...pins().querySelectorAll<HTMLElement>('[data-part="workspace"]')].map(
    (one) =>
      `${one.querySelector('[data-part="name"]')?.textContent}${
        one.getAttribute("aria-current") === "true" ? "*" : ""
      }${one.dataset.variant === "unpinned" ? "+" : ""}`,
  );
const opened = () =>
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
/** The title bar's workspace popover's key (the operator's ruling of 2026-10-24): ⌥⌘P on a Mac,
 *  Ctrl+Alt+Shift+P elsewhere, by the physical key. */
const switcherKey = () =>
  fireEvent.keyDown(
    window,
    onAMac()
      ? { key: "π", code: "KeyP", metaKey: true, altKey: true }
      : { key: "P", code: "KeyP", ctrlKey: true, altKey: true, shiftKey: true },
  );
/** ⌘P, which stays the palette's finder of files and projects. */
const paletteKey = () =>
  fireEvent.keyDown(
    window,
    onAMac() ? { key: "p", metaKey: true } : { key: "P", ctrlKey: true, shiftKey: true },
  );
const pinKey = (place: number) =>
  fireEvent.keyDown(
    window,
    onAMac() ? { key: String(place), metaKey: true } : { key: String(place), ctrlKey: true },
  );
/** The rows of the popover's list named `label`, `*` on the one Enter would go to. */
const rows = (label: string) =>
  within(screen.getByRole("listbox", { name: label }))
    .queryAllByRole("option")
    .map(
      (row) =>
        `${row.querySelector("[data-titlebar-name]")?.textContent}${
          row.getAttribute("aria-selected") === "true" ? "*" : ""
        }`,
    );

describe("the title bar names the project and its pins (R92, R101, R110)", () => {
  it("reads the project, a slash, and its pins as chips in the order they were pinned", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    expect(within(bar()).getByTestId("title-project")).toHaveTextContent(/^one$/);
    expect(bar().querySelector('[data-part="slash"]')).toHaveTextContent("/");
    // A chip has no ×: a pin is let go from the popover's toggle (R110).
    expect(within(pins()).queryByRole("button", { name: /unpin|close/i })).toBeNull();
    // The workspace strip under the bar is gone (#2916).
    expect(screen.queryByRole("tablist", { name: "Workspaces" })).toBeNull();
  });

  it("goes to a pin on its chip, and its tooltip says its key (R102)", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    await userEvent.click(within(pins()).getByRole("button", { name: "gamma" }));
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma*"]));
  });
});

describe("the ⌄ and its workspace popover (R106–R109)", () => {
  it("lists the root, the pins first, then the rest, and a row that makes one", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    const all = within(pins()).getByRole("button", { name: "All workspaces" });
    expect(all).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(all);
    const filter = await screen.findByRole("combobox", { name: "Go to a workspace in one…" });
    await waitFor(() => expect(filter).toHaveFocus());
    expect(all).toHaveAttribute("aria-expanded", "true");
    expect(rows("Workspaces in one")).toEqual([
      "Plane root",
      "alpha*",
      "gamma",
      "beta",
      "New workspace…",
    ]);
  });

  it("chooses the first row typing finds, stops at the ends, and goes on Enter", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    await userEvent.click(within(pins()).getByRole("button", { name: "All workspaces" }));
    await screen.findByRole("combobox", { name: "Go to a workspace in one…" });
    await userEvent.keyboard("be");
    expect(rows("Workspaces in one")).toEqual(["beta*", "New workspace…"]);
    await userEvent.keyboard("{ArrowUp}{ArrowUp}");
    expect(rows("Workspaces in one")).toEqual(["beta*", "New workspace…"]);
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    // Not pinned: the dashed chip follows the pins (R105).
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma", "beta*+"]));
  });

  it("says when nothing matches, and closes on Escape (R99, R109)", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    await userEvent.click(within(pins()).getByRole("button", { name: "All workspaces" }));
    await screen.findByRole("combobox", { name: "Go to a workspace in one…" });
    await userEvent.keyboard("zzz");
    expect(screen.getByText("No workspace matches.")).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
  });

  it("pins and unpins from a row's toggle, and stays open (R107, R109, R110)", async () => {
    const { pinned } = core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    await userEvent.click(within(pins()).getByRole("button", { name: "All workspaces" }));
    await screen.findByRole("combobox", { name: "Go to a workspace in one…" });
    await userEvent.click(screen.getByRole("button", { name: "Pin beta to the title bar" }));
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma", "beta"]));
    expect(pinned.get(ONE)).toEqual(["alpha", "gamma", "beta"]);
    await userEvent.click(screen.getByRole("button", { name: "Unpin gamma" }));
    await waitFor(() => expect(chips()).toEqual(["alpha*", "beta"]));
    expect(screen.getByRole("listbox", { name: "Workspaces in one" })).toBeInTheDocument();
  });

  it("opens and closes on ⌥⌘P (R106, as the ruling of 2026-10-24 amends it)", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    switcherKey();
    expect(
      await screen.findByRole("combobox", { name: "Go to a workspace in one…" }),
    ).toBeInTheDocument();
    switcherKey();
    await waitFor(() => expect(screen.queryByRole("combobox")).toBeNull());
  });
});

describe("the keys (the ruling of 2026-10-24)", () => {
  it("leaves ⌘P to the palette's finder of files and projects", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    paletteKey();
    expect(await screen.findByRole("dialog", { name: /project/i })).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: /^Go to a workspace in / })).toBeNull();
  });

  it("takes nothing from under a dialog: ⌥⌘P and ⌘1–⌘9 wait while one is up", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    // The dialog family's own dialog, as New workspace… opens it from the ⌄.
    await userEvent.click(within(pins()).getByRole("button", { name: "All workspaces" }));
    await userEvent.click(
      within(await screen.findByRole("listbox", { name: "Workspaces in one" })).getByRole(
        "option",
        { name: "New workspace…" },
      ),
    );
    const dialog = await screen.findByRole("dialog", { name: /workspace/i });
    switcherKey();
    pinKey(2);
    expect(screen.queryByRole("combobox", { name: /^Go to a workspace in / })).toBeNull();
    expect(chips()).toEqual(["alpha*", "gamma"]);
    expect(dialog).toBeInTheDocument();
  });
});

describe("the keyboard, back where it was (R109)", () => {
  it("goes back to what had it when Escape closes the popover", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    const rail = screen.getByRole("tab", { name: "one" });
    rail.focus();
    switcherKey();
    const filter = await screen.findByRole("combobox", { name: "Go to a workspace in one…" });
    await waitFor(() => expect(filter).toHaveFocus());
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(rail).toHaveFocus());
  });

  it("goes back to what had it after a switch from the popover", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    const rail = screen.getByRole("tab", { name: "one" });
    rail.focus();
    switcherKey();
    await screen.findByRole("combobox", { name: "Go to a workspace in one…" });
    await userEvent.keyboard("gamma{Enter}");
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma*"]));
    await waitFor(() => expect(rail).toHaveFocus());
  });
});

describe("the dashed chip (R105)", () => {
  it("pins the current workspace while it is not pinned", async () => {
    const { pinned } = core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    switcherKey();
    await screen.findByRole("combobox", { name: "Go to a workspace in one…" });
    await userEvent.keyboard("beta{Enter}");
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma", "beta*+"]));
    await userEvent.click(within(pins()).getByRole("button", { name: "beta" }));
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma", "beta*"]));
    expect(pinned.get(ONE)).toEqual(["alpha", "gamma", "beta"]);
  });
});

describe("⌘1–⌘9 (R111)", () => {
  it("goes to the project's pins by their place", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    pinKey(2);
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma*"]));
    pinKey(1);
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    // No third pin: the key goes on to whoever else wants it.
    pinKey(3);
    expect(chips()).toEqual(["alpha*", "gamma"]);
  });
});

describe("a sideways swipe over the title bar (R112)", () => {
  /** One gesture: `count` wheel events of `dx` each, over `on`. */
  const swipe = (on: Element, dx: number, count = 6) => {
    for (let n = 0; n < count; n++) fireEvent.wheel(on, { deltaX: dx, deltaY: 0 });
  };

  /**
   * **A trace's wheel events over `on`, each stamped with its own time**, so a gesture's length
   * is the trace's and never how long the test took between two of its events (#3195).
   */
  const replay = (on: Element, trace: readonly Traced[]) => {
    for (const { dx, at } of trace) {
      const wheel = new WheelEvent("wheel", { deltaX: dx, deltaY: 0, bubbles: true });
      Object.defineProperty(wheel, "timeStamp", { value: at });
      fireEvent(on, wheel);
    }
  };

  it("steps exactly one pin per gesture, momentum and all", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    // Once the bar has drawn its ⌄, every pin it swipes to is the project's to go to.
    await screen.findByRole("button", { name: "All workspaces" });
    const [stroke, momentum] = [
      played([STROKE], 1000),
      played([MOMENTUM], 1000 + STROKE.length * FRAME),
    ];
    replay(bar(), stroke);
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma*"]), { timeout: 5000 });
    // The rest of the same gesture steps nothing more, however long the wait before it was.
    replay(bar(), momentum);
    // A second stroke, the fingers down again just after the tail, steps once more (#3327).
    replay(bar(), played([STROKE], momentum[momentum.length - 1].at + FRAME * 3));
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]), { timeout: 5000 });
    expect(slid.calls.map(({ step }) => step)).toEqual([1, 1]);
  });

  it("is the title bar's only: anywhere else a sideways scroll is what is under it", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    swipe(screen.getByTestId("status-line"), 80);
    expect(chips()).toEqual(["alpha*", "gamma"]);
  });
});

describe("a workspace switch slides the workspace in (R114)", () => {
  it("slides the regions, not the bars, the way the switch went", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    pinKey(2);
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma*"]));
    await waitFor(() => expect(slid.calls).toHaveLength(1));
    const [{ parts, step }] = slid.calls;
    expect(step).toBe(1);
    expect(parts).toHaveClass("regions");
    // The activity bars are outside what slides.
    expect(parts?.querySelector('[role="tablist"][aria-label="Attention"]') ?? null).toBeNull();
    // A swipe round the end of the pins slides on, the way it went, not back.
    for (let n = 0; n < 6; n++) fireEvent.wheel(bar(), { deltaX: 40, deltaY: 0 });
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    await waitFor(() => expect(slid.calls).toHaveLength(2));
    expect(slid.calls[1].step).toBe(1);
  });
});

describe("with the rail hidden, the project's crumb is the switcher (R93–R100)", () => {
  async function hidden() {
    RAIL_MODE.keep("auto");
    const held = core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    return held;
  }

  it("opens two columns: the projects, and the workspaces of the one chosen", async () => {
    await hidden();
    const crumb = within(bar()).getByTestId("title-project");
    expect(crumb).toHaveAttribute("aria-haspopup", "dialog");
    await userEvent.click(crumb);
    const filter = await screen.findByRole("combobox", { name: "Go to a project or workspace…" });
    await waitFor(() => expect(filter).toHaveFocus());
    expect(rows("Projects")).toEqual(["one*", "two"]);
    expect(rows("Workspaces in one")).toEqual(["Plane root", "alpha", "gamma", "beta"]);
  });

  it("only shows a project's workspaces on a press, and goes on a workspace's (R97)", async () => {
    await hidden();
    await userEvent.click(within(bar()).getByTestId("title-project"));
    await screen.findByRole("combobox", { name: "Go to a project or workspace…" });
    await userEvent.click(
      within(screen.getByRole("listbox", { name: "Projects" })).getByRole("option", {
        name: "two",
      }),
    );
    expect(rows("Projects")).toEqual(["one", "two*"]);
    expect(within(bar()).getByTestId("title-project")).toHaveTextContent("one");
    await userEvent.click(
      within(screen.getByRole("listbox", { name: "Workspaces in two" })).getByRole("option", {
        name: "beta",
      }),
    );
    await waitFor(() =>
      expect(within(bar()).getByTestId("title-project")).toHaveTextContent("two"),
    );
    await waitFor(() => expect(chips()).toEqual(["beta*+"]));
  });

  it("finds a project by a workspace's name, and steps across with → (R98, R100)", async () => {
    await hidden();
    switcherKey();
    await screen.findByRole("combobox", { name: "Go to a project or workspace…" });
    await userEvent.keyboard("{ArrowDown}");
    expect(rows("Projects")).toEqual(["one", "two*"]);
    await userEvent.keyboard("{ArrowRight}{ArrowDown}{ArrowDown}{Enter}");
    await waitFor(() =>
      expect(within(bar()).getByTestId("title-project")).toHaveTextContent("two"),
    );
    await waitFor(() => expect(chips()).toEqual(["gamma*+"]));
  });

  it("pins a workspace of a project behind the one in front, and stays open (R96, R109)", async () => {
    const { pinned } = await hidden();
    await userEvent.click(within(bar()).getByTestId("title-project"));
    await screen.findByRole("combobox", { name: "Go to a project or workspace…" });
    await userEvent.click(
      within(screen.getByRole("listbox", { name: "Projects" })).getByRole("option", {
        name: "two",
      }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Pin beta to the title bar" }));
    await waitFor(() => expect(pinned.get(TWO)).toEqual(["beta"]));
    expect(await screen.findByRole("button", { name: "Unpin beta" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("listbox", { name: "Workspaces in two" })).toBeInTheDocument();
  });

  it("says when no project matches (R99)", async () => {
    await hidden();
    switcherKey();
    await screen.findByRole("combobox", { name: "Go to a project or workspace…" });
    await userEvent.keyboard("zzz");
    expect(screen.getByText("No project matches.")).toBeInTheDocument();
  });
});

describe("choosing a project lands on the workspace it was last on (R81)", () => {
  it("comes back to the workspace left there", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "gamma"]));
    pinKey(2);
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma*"]));
    await userEvent.click(await screen.findByRole("tab", { name: "two" }));
    await waitFor(() =>
      expect(within(bar()).getByTestId("title-project")).toHaveTextContent("two"),
    );
    await userEvent.click(screen.getByRole("tab", { name: "one" }));
    await waitFor(() =>
      expect(within(bar()).getByTestId("title-project")).toHaveTextContent("one"),
    );
    expect(chips()).toEqual(["alpha", "gamma*"]);
  });
});
