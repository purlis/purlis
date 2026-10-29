// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { OUTSIDE, type Said } from "./actions";
import { TitleBarSwitch } from "./TitleBarSwitch";
import { forgetRailMode } from "./railMode";

/**
 * **A workspace's notices hang from its chip** (#3648, the tracer of spec #3647), on the real
 * window with the core mocked: making a workspace opens it and pins it, even with a chat in
 * front, and its "ready" notice hangs from its chip as a card that takes no keyboard, pushes
 * nothing, hides on a press elsewhere and leaves a dot, and is dismissed by × or Esc, in the
 * Notices centre too.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

/** jsdom's own `scrollIntoView`, if it has one, put back after a test that stood one in. */
const SCROLL_INTO_VIEW = Element.prototype.scrollIntoView;

afterEach(() => {
  cleanup();
  clearMocks();
  forgetRailMode();
  vi.restoreAllMocks();
  Element.prototype.scrollIntoView = SCROLL_INTO_VIEW;
});

/** A box per element: the strip 0–300 across, `alpha`'s chip wherever `chipAt` says now. */
function laidOut(chipAt: () => number) {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    const strip = this.getAttribute("role") === "toolbar";
    const chip = this.getAttribute("data-part") === "workspace" && this.textContent === "alpha";
    const left = strip ? 0 : chip ? chipAt() : 0;
    const width = strip ? 300 : 60;
    return {
      x: left,
      y: 0,
      left,
      top: 0,
      right: left + width,
      bottom: 28,
      width,
      height: 28,
      toJSON: () => ({}),
    } as DOMRect;
  });
}

const PLANE = "/home/dev/one";

/** The chat in `alpha`, in front. */
const CHAT = {
  session: 1,
  name: "alpha.1",
  cwd: `${PLANE}/workspaces/alpha`,
  harness: null,
  in_front: true,
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

/** One project with `alpha` pinned and its chat in front; `workspace_create` makes one, as the
 *  core does, with no vision, and the store keeps a pin written. */
function core() {
  const pinned: string[] = ["alpha"];
  const made: string[] = [];
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: ["alpha", ...made].map((name) => ({
          name,
          path: `${PLANE}/workspaces/${name}`,
          vision: "",
          todos: [],
          chats: name === "alpha" ? [CHAT] : [],
        })),
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
    if (cmd === "opened_chats") return [CHAT];
    if (cmd === "plane_pins")
      return { project: false, workspaces: pinned, missing: [], order: pinned, certain: true };
    if (cmd === "pin_workspace") {
      const name = String(given.workspace);
      if (given.pinned === true && !pinned.includes(name)) pinned.push(name);
      return null;
    }
    if (cmd === "workspace_create") {
      const name = String(given.name);
      made.push(name);
      return [
        `✓ Workspace '${name}' ready (LOCAL) → workspaces/${name}/`,
        '⬢ No vision yet — ask the developer what this workspace is for, then record it: purlis workspace vision "<the goal>"',
      ];
    }
    if (cmd === "reachable_repos") return { repos: [], trouble: [] };
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
/** The strip's chips' names, `*` on the current one, `+` on the dashed one: not the plane
 *  root's house, which stands outside the strip (#3654). */
const chips = () =>
  [
    ...bar().querySelectorAll<HTMLElement>(
      '[role="toolbar"] [data-scope="titlebar"][data-part="workspace"]',
    ),
  ].map(
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

/** Makes `name` from the ⌄'s last row, as a person does. */
async function make(name: string) {
  await userEvent.click(within(bar()).getByRole("button", { name: "All workspaces" }));
  const list = await screen.findByRole("listbox", { name: /^Workspaces in / });
  await userEvent.click(within(list).getByRole("option", { name: "New workspace…" }));
  const dialog = await screen.findByRole("dialog", { name: /workspace/i });
  await userEvent.type(within(dialog).getByLabelText("Name"), name);
  await userEvent.click(within(dialog).getByRole("button", { name: "Create workspace" }));
}

describe("making a workspace opens it (#3648)", () => {
  it("switches the window to it with a chat in front, and pins its chip", async () => {
    const { pinned } = core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    expect(await screen.findByTestId("pane")).toHaveTextContent("session 1");

    await make("gamma");

    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma*"]));
    expect(pinned).toEqual(["alpha", "gamma"]);
    // Its chats are none, so no chat stands in front of it: alpha's is not left on screen.
    expect(screen.queryByTestId("pane")).toBeNull();
  });
});

/** The card that hangs from `workspace`'s chip, once it is drawn. */
const findCard = (workspace: string) =>
  screen.findByRole("region", { name: `Notices for ${workspace}` });
const queryCard = (workspace: string) =>
  screen.queryByRole("region", { name: `Notices for ${workspace}` });
/** The dot a hidden notice leaves on `workspace`'s chip. */
const queryDot = (workspace: string) =>
  within(bar()).queryByRole("button", { name: new RegExp(`^Show the notice on ${workspace}`) });
/** The Notices centre's Results, where every result is listed too. */
const results = () => screen.findByRole("region", { name: "Results" });

describe("the ready notice hangs from its chip (#3648)", () => {
  it("as a card over the window, in no page's flow, and without Open", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma");

    const card = await findCard("gamma");
    expect(card).toHaveTextContent("Workspace gamma is ready");
    expect(card).toHaveTextContent(
      "It has no vision yet. Add one so its chats know what it is for.",
    );
    // A popover of the family, hung from the chip, and nothing at the top of the content.
    expect(card.closest('[data-scope="popover"][data-part="root"]')).toHaveAttribute(
      "data-variant",
      "notice",
    );
    expect(card).toHaveAttribute("data-chip", "gamma");
    expect(card.closest(".window-work")).toBeNull();
    expect(document.querySelector(".window-work > .action-result")).toBeNull();
    // What already happened is not offered: no Open.
    expect(within(card).queryByRole("button", { name: /^Open/ })).toBeNull();
    expect(within(card).getByRole("button", { name: "Add a vision" })).toBeInTheDocument();
    // Details opens in place, in the card.
    await userEvent.click(within(card).getByRole("button", { name: "Details" }));
    expect(card).toHaveTextContent("✓ Workspace 'gamma' ready (LOCAL) → workspaces/gamma/");
    expect(document.body).not.toHaveTextContent("ask the developer");
  });

  it("takes no keyboard, and is said once, politely", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma");

    const card = await findCard("gamma");
    expect(card.contains(document.activeElement)).toBe(false);
    const saying = [...document.querySelectorAll('[role="status"]')].filter((one) =>
      one.textContent?.includes("Workspace gamma is ready"),
    );
    expect(saying).toHaveLength(1);
    expect(card.contains(saying[0])).toBe(true);
  });

  it("is reached by F6", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma");
    const card = await findCard("gamma");

    await userEvent.keyboard("{F6}");
    await waitFor(() => expect(card.contains(document.activeElement)).toBe(true));
    // And F6 goes on from it, round the window's regions.
    await userEvent.keyboard("{F6}");
    await waitFor(() => expect(card.contains(document.activeElement)).toBe(false));
  });

  it("hides on a press elsewhere and leaves a dot on the chip, which brings it back", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma");
    await findCard("gamma");
    expect(queryDot("gamma")).toBeNull();

    await userEvent.click(document.body);
    await waitFor(() => expect(queryCard("gamma")).toBeNull());
    const dot = queryDot("gamma");
    expect(dot).not.toBeNull();
    // Hidden, not dismissed: still in the Notices centre.
    expect(within(await results()).getByText("Workspace gamma is ready")).toBeInTheDocument();
    // The chip says what its dot shows.
    const chip = within(bar()).getByRole("button", { name: "gamma" });
    expect(chip).toHaveAccessibleDescription(/1 notice/);

    await userEvent.click(dot as HTMLElement);
    expect(await findCard("gamma")).toHaveTextContent("Workspace gamma is ready");
    expect(queryDot("gamma")).toBeNull();
  });

  it("is dismissed by its ×, in the Notices centre too", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma");
    const card = await findCard("gamma");
    expect(within(await results()).getByText("Workspace gamma is ready")).toBeInTheDocument();

    await userEvent.click(within(card).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(queryCard("gamma")).toBeNull());
    expect(queryDot("gamma")).toBeNull();
    // Gone from the Notices centre's Results too, which has nothing left to list.
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Results" })?.textContent ?? "").not.toContain(
        "Workspace gamma is ready",
      ),
    );
  });

  it("is dismissed by Esc while the keyboard is in it", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma");
    const card = await findCard("gamma");

    within(card).getByRole("button", { name: "Add a vision" }).focus();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(queryCard("gamma")).toBeNull());
    expect(queryDot("gamma")).toBeNull();
  });

  it("goes from the chip when the Notices centre dismisses it", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma");
    await findCard("gamma");

    const kept = await results();
    await userEvent.click(within(kept).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(queryCard("gamma")).toBeNull());
    expect(queryDot("gamma")).toBeNull();
  });
});

/** The title bar alone, with one notice about `on` (a pin, or the plane root), its card up or
 *  hidden, as the window holds it. */
function Bar({ on, startOpen }: { on: string; startOpen: boolean }) {
  const [open, setOpen] = useState<string | undefined>(startOpen ? on : undefined);
  const [said, setSaid] = useState<Said | undefined>({
    from: "x",
    refused: false,
    title: on === OUTSIDE ? "The root moved" : `About ${on}`,
    words: "",
    workspace: on,
  });
  return (
    <TitleBarSwitch
      projects={[
        {
          plane: PLANE,
          name: "one",
          waiting: 0,
          workspaces: { names: ["alpha"], pins: ["alpha"], focused: "alpha", waiting: {} },
        },
      ]}
      inFront={PLANE}
      railShown
      does={{
        go: () => undefined,
        choose: () => undefined,
        pin: () => undefined,
        create: () => undefined,
      }}
      notices={{
        on: new Map(said === undefined ? [] : [[on, [{ id: 1, kind: "info" as const, said }]]]),
        open: said === undefined ? undefined : open,
        show: setOpen,
        hide: () => setOpen(undefined),
        dismiss: () => setSaid(undefined),
        onSettings: () => undefined,
      }}
    />
  );
}

describe("a dot pressed from the keyboard (#3648's review, WCAG 2.4.3)", () => {
  it("hands the keyboard to the card it brings back, not to the page", async () => {
    render(<Bar on="alpha" startOpen={false} />);
    const dot = screen.getByRole("button", { name: "Show the notice on alpha" });
    dot.focus();
    await userEvent.keyboard("{Enter}");
    const card = await screen.findByRole("region", { name: "Notices for alpha" });
    await waitFor(() => expect(card).toHaveFocus());
  });

  it("gives the keyboard to the chip once the card it went into is dismissed", async () => {
    // A chip in sight is left where it is: no scroll (#3648's round 4, note 4).
    laidOut(() => 100);
    const into = vi.fn();
    Element.prototype.scrollIntoView = into;
    render(<Bar on="alpha" startOpen={false} />);
    screen.getByRole("button", { name: "Show the notice on alpha" }).focus();
    await userEvent.keyboard("{Enter}");
    const card = await screen.findByRole("region", { name: "Notices for alpha" });
    await waitFor(() => expect(card).toHaveFocus());
    await userEvent.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Notices for alpha" })).toBeNull(),
    );
    // The dot went with its notice: the chip it hung from has the keyboard, never the page.
    await waitFor(() => expect(screen.getByRole("button", { name: "alpha" })).toHaveFocus());
    expect(into).not.toHaveBeenCalled();
  });
});

describe("a card whose chip scrolls out of the strip's sight (#3648's review)", () => {
  it("is hidden, and leaves the dot for when the chip comes back", async () => {
    let chipAt = 100;
    laidOut(() => chipAt);
    render(<Bar on="alpha" startOpen />);
    await screen.findByRole("region", { name: "Notices for alpha" });
    chipAt = -120;
    fireEvent.scroll(screen.getByRole("toolbar", { name: "Pinned workspaces" }));
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Notices for alpha" })).toBeNull(),
    );
    expect(screen.getByRole("button", { name: "Show the notice on alpha" })).toBeInTheDocument();
  });

  it("hands the keyboard it held to the chip, brought back into sight (WCAG 2.4.11)", async () => {
    let chipAt = 100;
    laidOut(() => chipAt);
    const into = vi.fn();
    Element.prototype.scrollIntoView = into;
    render(<Bar on="alpha" startOpen={false} />);
    screen.getByRole("button", { name: "Show the notice on alpha" }).focus();
    await userEvent.keyboard("{Enter}");
    const card = await screen.findByRole("region", { name: "Notices for alpha" });
    await waitFor(() => expect(card).toHaveFocus());
    // The strip scrolled under the pointer: the chip leaves sight, and the card goes.
    chipAt = -120;
    fireEvent.scroll(screen.getByRole("toolbar", { name: "Pinned workspaces" }));
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Notices for alpha" })).toBeNull(),
    );
    const chip = screen.getByRole("button", { name: "alpha" });
    await waitFor(() => expect(chip).toHaveFocus());
    expect(into).toHaveBeenCalledWith({ inline: "nearest", block: "nearest" });
  });
});

describe("a notice about the plane root (the dispatcher's ruling on #3648)", () => {
  it("hangs from the root's house chip, outside the strip", async () => {
    render(<Bar on={OUTSIDE} startOpen />);
    const card = await screen.findByRole("region", { name: "Notices for Plane root" });
    expect(card).toHaveTextContent("The root moved");
    expect(card).toHaveAttribute("data-chip", OUTSIDE);
  });

  it("leaves its dot on the house chip, which brings the card back", async () => {
    render(<Bar on={OUTSIDE} startOpen={false} />);
    const dot = screen.getByRole("button", { name: "Show the notice on Plane root" });
    expect(screen.getByRole("toolbar", { name: "Pinned workspaces" }).contains(dot)).toBe(false);
    await userEvent.click(dot);
    expect(
      await screen.findByRole("region", { name: "Notices for Plane root" }),
    ).toBeInTheDocument();
  });

  it("is a Tab stop of its own on the house, which says it has a notice", async () => {
    render(<Bar on={OUTSIDE} startOpen={false} />);
    const dot = screen.getByRole("button", { name: "Show the notice on Plane root" });
    // Outside the strip, no arrow walk reaches it: it is a Tab stop, as the house is.
    expect(dot).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("button", { name: "Plane root" })).toHaveAccessibleDescription(
      "1 notice",
    );
    dot.focus();
    await userEvent.keyboard("{Enter}");
    const card = await screen.findByRole("region", { name: "Notices for Plane root" });
    await waitFor(() => expect(card).toHaveFocus());
  });
});
