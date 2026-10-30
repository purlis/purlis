// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { OUTSIDE, type Said } from "./actions";
import type { StaleClone } from "./bindings";
import { TitleBarSwitch } from "./TitleBarSwitch";
import { forgetRailMode } from "./railMode";
import { forgetRepoClones } from "./repoClones";

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
  // A clone a test left under way would be skipped by the next test's ask.
  forgetRepoClones();
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
 *  core does, with no vision, and the store keeps a pin written. `missing` are pins whose
 *  workspace is gone (#3651); while `refusing` holds, the store refuses a pin. */
function core({
  clone,
  repos = ["api"],
  missing = [],
  refusing,
  unlisted = false,
  stale = [],
  behind = [],
}: {
  /** How `clone_repo` answers each repo: the core's lines, or a refusal it throws. */
  clone?: (repo: string) => Promise<string[]>;
  /** The repos the new-workspace dialog offers, while `clone` is given. */
  repos?: string[];
  missing?: string[];
  refusing?: { pin: boolean };
  /** A workspace made is not in the plane's listing: removed or renamed since (#3651). */
  unlisted?: boolean;
  /** The project's stale repos (#3650), until a fix clears them. */
  stale?: StaleClone[];
  /** The workspaces behind the current layout, until `workspace-reinit` mends them (#3650). */
  behind?: string[];
} = {}) {
  const pinned: string[] = ["alpha"];
  const gone = [...missing];
  const made: string[] = [];
  let staleNow = stale;
  let behindNow = behind;
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: ["alpha", ...(unlisted ? [] : made)].map((name) => ({
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
      return {
        project: false,
        workspaces: pinned,
        missing: gone,
        order: [...pinned, ...gone],
        certain: true,
      };
    if (cmd === "pin_workspace") {
      const name = String(given.workspace);
      if (refusing?.pin === true) throw new Error("the store is read-only");
      if (given.pinned === true && !pinned.includes(name)) pinned.push(name);
      if (given.pinned === false && gone.includes(name)) gone.splice(gone.indexOf(name), 1);
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
    if (cmd === "reachable_repos")
      return {
        repos:
          clone === undefined
            ? []
            : repos.map((name) => ({ name, path: `acme/${name}`, description: "" })),
        trouble: [],
      };
    if (cmd === "take_repos") return null;
    if (cmd === "clone_repo" && clone !== undefined) return clone(String(given.repo));
    if (cmd === "stale_clones") return { stale: staleNow, unread: [] };
    if (cmd === "fix_stale_clone") {
      // The one repo fixed, as the core fixes one at a time.
      staleNow = staleNow.filter((one) => one.repo !== String(given.repo));
      return `${String(given.repo)} is on main.`;
    }
    if (cmd === "alerts_everywhere")
      return [
        {
          plane: PLANE,
          alerts:
            behindNow.length === 0
              ? []
              : [
                  {
                    severity: "warn",
                    subject: "reinit",
                    detail: `${behindNow.length} workspace is behind the current layout: ${behindNow.join(", ")}`,
                    workspaces: behindNow,
                    way: { kind: "fix", id: "workspace-reinit" },
                  },
                ],
          stopped: null,
        },
      ];
    if (cmd === "plane_doctor_fix") {
      behindNow = [];
      return { fix: String(given.fix), refused: null, said: [], complete: true };
    }
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    return null;
  });
  return { asked, pinned, gone };
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
      }${one.dataset.variant === "unpinned" ? "+" : ""}${one.dataset.variant === "dormant" ? "~" : ""}`,
  );

const opened = () =>
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );

/** Makes `name` from the ⌄'s last row, as a person does, picking `repos` to clone into it. */
async function make(name: string, repos: string[] = []) {
  await userEvent.click(within(bar()).getByRole("button", { name: "All workspaces" }));
  const list = await screen.findByRole("listbox", { name: /^Workspaces in / });
  await userEvent.click(within(list).getByRole("option", { name: "New workspace…" }));
  const dialog = await screen.findByRole("dialog", { name: /workspace/i });
  await userEvent.type(within(dialog).getByLabelText("Name"), name);
  for (const repo of repos)
    await userEvent.click(await within(dialog).findByRole("checkbox", { name: repo }));
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

/** Every line on screen that says `words`, wherever it is drawn. */
const linesSaying = (words: RegExp) =>
  [...document.querySelectorAll<HTMLElement>("[data-cause]")].filter((one) =>
    words.test(one.textContent ?? ""),
  );

describe("a dormant pin's notice hangs from its dormant chip (#3651)", () => {
  it("draws the gone pin as a dormant chip after the live pins, its notice behind its dot", async () => {
    core({ missing: ["delta"] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "delta~"]));
    const chip = within(bar()).getByRole("button", { name: "delta" });
    expect(chip).toHaveAccessibleDescription(/gone, kept dormant/);
    // Found at launch, not caused by a press: no card comes up by itself.
    expect(queryCard("delta")).toBeNull();
    const dot = await waitFor(() => {
      const found = queryDot("delta");
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });

    await userEvent.click(dot);
    const card = await findCard("delta");
    expect(card).toHaveTextContent("delta is gone, kept dormant");
    expect(within(card).getByRole("button", { name: "Forget" })).toBeInTheDocument();
    // The same line is listed in the Notices centre too, outside the card.
    expect(linesSaying(/delta is gone, kept dormant/).some((one) => !card.contains(one))).toBe(
      true,
    );
  });

  it("is dismissed by its ×, in the Notices centre too; the chip's press brings it back", async () => {
    core({ missing: ["delta"] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "delta~"]));
    const chip = within(bar()).getByRole("button", { name: "delta" });
    await userEvent.click(chip);
    const card = await findCard("delta");

    await userEvent.click(within(card).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(queryCard("delta")).toBeNull());
    expect(queryDot("delta")).toBeNull();
    await waitFor(() => expect(linesSaying(/delta is gone, kept dormant/)).toHaveLength(0));
    // The chip stays: the pin is still dormant, only its line was put away.
    expect(chips()).toEqual(["alpha*", "delta~"]);

    await userEvent.click(within(bar()).getByRole("button", { name: "delta" }));
    expect(await findCard("delta")).toHaveTextContent("delta is gone, kept dormant");
  });

  it("forgets the pin from the card, and 'Forgot the pin' stays in the Notices centre", async () => {
    const { asked } = core({ missing: ["delta"] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "delta~"]));
    await userEvent.click(within(bar()).getByRole("button", { name: "delta" }));
    const card = await findCard("delta");

    await userEvent.click(within(card).getByRole("button", { name: "Forget" }));
    await waitFor(() =>
      expect(
        asked.some(
          (one) =>
            one.cmd === "pin_workspace" &&
            one.args.workspace === "delta" &&
            one.args.pinned === false,
        ),
      ).toBe(true),
    );
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    const forgot = await waitFor(() => {
      const found = linesSaying(/Forgot the pin to delta/);
      expect(found).toHaveLength(1);
      return found[0];
    });
    // Its chip is gone, so it hangs from none: it is in no card.
    expect(forgot.closest("[data-chip]")).toBeNull();
    expect(within(forgot).getByRole("button", { name: "Undo" })).toBeInTheDocument();
  });
});

describe("a dormant chip and the keyboard (#3651's review)", () => {
  it("gives the keyboard to the chip in front when Forget takes the dormant chip with its card", async () => {
    core({ missing: ["delta"] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "delta~"]));
    // From the keyboard alone: the strip's arrows reach the dormant chip, Enter opens its card.
    within(bar()).getByRole("button", { name: "delta" }).focus();
    await userEvent.keyboard("{Enter}");
    const card = await findCard("delta");
    await waitFor(() => expect(card.contains(document.activeElement)).toBe(true));
    within(card).getByRole("button", { name: "Forget" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    expect(queryCard("delta")).toBeNull();
    // Never the page: the strip's stop, the chip of the workspace in front.
    await waitFor(() =>
      expect(document.activeElement).toBe(within(bar()).getByRole("button", { name: "alpha" })),
    );
  });

  it("opens no menu and goes nowhere: a press shows its notice, and alpha stays in front", async () => {
    core({ missing: ["delta"] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "delta~"]));
    // The control: a live pin's chip has its workspace's menu.
    fireEvent.contextMenu(within(bar()).getByRole("button", { name: "alpha" }));
    expect(await screen.findByRole("menu")).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());

    const gone = within(bar()).getByRole("button", { name: "delta" });
    fireEvent.contextMenu(gone);
    await new Promise((settle) => setTimeout(settle, 50));
    expect(screen.queryByRole("menu")).toBeNull();
    await userEvent.click(gone);
    expect(await findCard("delta")).toHaveTextContent("delta is gone, kept dormant");
    expect(chips()).toEqual(["alpha*", "delta~"]);
  });

  it("loses its dot when the Notices centre dismisses its line", async () => {
    core({ missing: ["delta"] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*", "delta~"]));
    await waitFor(() => expect(queryDot("delta")).not.toBeNull());
    const listed = await waitFor(() => {
      const found = linesSaying(/delta is gone, kept dormant/);
      expect(found).toHaveLength(1);
      return found[0];
    });
    await userEvent.click(within(listed).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(queryDot("delta")).toBeNull());
    expect(chips()).toEqual(["alpha*", "delta~"]);
  });
});

describe("a notice about the current, unpinned workspace (#3651)", () => {
  it("hangs from its dashed chip, and stays on it once it is pinned", async () => {
    const refusing = { pin: true };
    core({ refusing });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma");

    // The store refused the pin: the workspace is made, in front, on its dashed chip.
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma*+"]));
    const card = await findCard("gamma");
    expect(card).toHaveTextContent("Workspace gamma is ready");
    expect(card).toHaveAttribute("data-chip", "gamma");

    refusing.pin = false;
    await userEvent.click(within(bar()).getByRole("button", { name: /^gamma/ }));
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma*"]));
    // Pinned, the notice is still attached to gamma's chip: its card, or its dot.
    await waitFor(() => expect(queryCard("gamma") ?? queryDot("gamma")).not.toBeNull());
  });
});

describe("a notice about a workspace with no chip (#3651, D-3648-16)", () => {
  it("is listed in the Notices centre alone, with Open, which goes there", async () => {
    core({ refusing: { pin: true } });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma");
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma*+"]));
    await findCard("gamma");

    // Back to alpha: gamma, never pinned, has no chip now.
    await userEvent.click(within(bar()).getByRole("button", { name: "alpha" }));
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    expect(queryCard("gamma")).toBeNull();
    expect(queryDot("gamma")).toBeNull();
    const kept = await results();
    expect(within(kept).getByText("Workspace gamma is ready")).toBeInTheDocument();
    // Nothing anywhere else says it: it floats over no chip.
    expect(document.querySelector('[data-chip="gamma"]')).toBeNull();

    await userEvent.click(within(kept).getByRole("button", { name: "Open gamma" }));
    await waitFor(() => expect(chips()).toEqual(["alpha", "gamma*+"]));
    // Its chip is back: the notice hangs from it again, so Open is no longer offered.
    await waitFor(() =>
      expect(within(kept).queryByRole("button", { name: "Open gamma" })).toBeNull(),
    );
  });

  it("offers no Open for a workspace the project no longer has", async () => {
    core({ refusing: { pin: true }, unlisted: true });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma");
    const kept = await results();
    await waitFor(() =>
      expect(within(kept).getByText("Workspace gamma is ready")).toBeInTheDocument(),
    );
    // Wherever the window landed, back to alpha: gamma has no chip, and no listing either.
    if (!chips().includes("alpha*"))
      await userEvent.click(within(bar()).getByRole("button", { name: "alpha" }));
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    expect(within(kept).queryByRole("button", { name: "Open gamma" })).toBeNull();
  });

  it("offers no Open while the workspace has a chip", async () => {
    core();
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma");
    await findCard("gamma");
    expect(within(await results()).queryByRole("button", { name: "Open gamma" })).toBeNull();
  });
});

/** A clone the test lets land when it says: `land` with the core's lines, or `refuse`. */
function heldClone() {
  const held: { repo: string; land: (lines: string[]) => void; refuse: (why: string) => void }[] =
    [];
  const clone = (repo: string) =>
    new Promise<string[]>((land, refuse) => held.push({ repo, land, refuse }));
  return { clone, held };
}

describe("a new workspace's clone, on its chip (#3649)", () => {
  it("counts on the card, then turns into its result in place, newest first", async () => {
    const { clone, held } = heldClone();
    core({ clone });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma", ["api"]);

    const card = await findCard("gamma");
    await waitFor(() => expect(card).toHaveTextContent("Cloning 1 repo…"));
    // Stacked over the ready notice, newest first, with a line that says so.
    expect(card).toHaveTextContent("2 notices, newest first");
    expect(card.textContent?.indexOf("Cloning 1 repo…")).toBeLessThan(
      card.textContent?.indexOf("Workspace gamma is ready") ?? -1,
    );
    // Nothing of it in the page's flow.
    expect(document.querySelector(".window-work > .action-result")).toBeNull();

    await waitFor(() => expect(held).toHaveLength(1));
    held[0].land(["✓ api cloned"]);
    await waitFor(() => expect(card).toHaveTextContent("Cloned 1 repo"));
    // In place: still two notices, the clone's still first, and the card never went.
    expect(card).not.toHaveTextContent("Cloning 1 repo…");
    expect(card).toHaveTextContent("2 notices, newest first");
    expect(card.textContent?.indexOf("Cloned 1 repo")).toBeLessThan(
      card.textContent?.indexOf("Workspace gamma is ready") ?? -1,
    );
    expect(queryCard("gamma")).toBe(card);
  });

  it("says a failure with Retry and the repo settings, and the dot takes ▲", async () => {
    const { clone, held } = heldClone();
    core({ clone });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma", ["api"]);
    const card = await findCard("gamma");
    await waitFor(() => expect(held).toHaveLength(1));
    held[0].refuse("api: clone failed — no access.");

    await waitFor(() => expect(card).toHaveTextContent("Could not clone api into gamma"));
    expect(card).toHaveTextContent("api: clone failed — no access.");
    expect(within(card).getByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(
      within(card).getByRole("button", { name: "Open gamma's repo settings" }),
    ).toBeInTheDocument();

    // Hidden, its dot is the most urgent kind among the two: a warning, said as one.
    await userEvent.click(document.body);
    await waitFor(() => expect(queryCard("gamma")).toBeNull());
    const dot = within(bar()).getByRole("button", {
      name: "Show the 2 notices on gamma, most urgent: warning",
    });
    expect(dot).toHaveAttribute("data-kind", "warning");
    expect(dot).toHaveAttribute("data-part", "dot-warning");
    expect(dot.querySelector("svg.lucide-triangle")).not.toBeNull();
    expect(within(bar()).getByRole("button", { name: "gamma" })).toHaveAccessibleDescription(
      "2 notices, most urgent: warning",
    );
  });

  it("clones again from Retry, on the same notice, and the ▲ goes once it lands", async () => {
    const { clone, held } = heldClone();
    const { asked } = core({ clone });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma", ["api"]);
    const card = await findCard("gamma");
    await waitFor(() => expect(held).toHaveLength(1));
    held[0].refuse("api: clone failed — no access.");
    await userEvent.click(await within(card).findByRole("button", { name: "Retry" }));

    await waitFor(() => expect(card).toHaveTextContent("Cloning 1 repo…"));
    await waitFor(() => expect(held).toHaveLength(2));
    held[1].land(["✓ api cloned"]);
    await waitFor(() => expect(card).toHaveTextContent("Cloned 1 repo"));
    expect(card).toHaveTextContent("2 notices, newest first");
    expect(asked.filter((one) => one.cmd === "clone_repo")).toHaveLength(2);

    await userEvent.click(document.body);
    await waitFor(() => expect(queryCard("gamma")).toBeNull());
    expect(
      within(bar()).getByRole("button", { name: "Show the 2 notices on gamma" }),
    ).toHaveAttribute("data-kind", "info");
  });
});

describe("a new workspace's clone, counted and retried (#3649's review)", () => {
  it("counts each repo as it lands, on the same notice, then says them cloned", async () => {
    const { clone, held } = heldClone();
    core({ clone, repos: ["api", "web"] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma", ["api", "web"]);
    const card = await findCard("gamma");
    await waitFor(() => expect(card).toHaveTextContent("Cloning 2 repos…"));
    await waitFor(() => expect(held).toHaveLength(1));
    held[0].land(["✓ api cloned"]);
    await waitFor(() => expect(card).toHaveTextContent("1 of 2 done."));
    expect(card).toHaveTextContent("2 notices, newest first");
    await waitFor(() => expect(held).toHaveLength(2));
    held[1].land(["✓ web cloned"]);
    await waitFor(() => expect(card).toHaveTextContent("Cloned 2 repos"));
    expect(card).not.toHaveTextContent("1 of 2 done.");
    expect(card).toHaveTextContent("2 notices, newest first");
  });

  it("clones again only the repo that failed", async () => {
    const { clone, held } = heldClone();
    const { asked } = core({ clone, repos: ["api", "docs", "web"] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma", ["api", "docs", "web"]);
    const card = await findCard("gamma");
    await waitFor(() => expect(held).toHaveLength(1));
    held[0].land(["✓ api cloned"]);
    await waitFor(() => expect(held).toHaveLength(2));
    held[1].refuse("docs: no access.");
    await waitFor(() => expect(held).toHaveLength(3));
    held[2].land(["✓ web cloned"]);
    await userEvent.click(await within(card).findByRole("button", { name: "Retry" }));
    await waitFor(() => expect(held).toHaveLength(4));
    expect(held[3].repo).toBe("docs");
    held[3].land(["✓ docs cloned"]);
    await waitFor(() => expect(card).toHaveTextContent("Cloned 1 repo"));
    expect(asked.filter((one) => one.cmd === "clone_repo").map((one) => one.args.repo)).toEqual([
      "api",
      "docs",
      "web",
      "docs",
    ]);
  });

  it("keeps the keyboard in the card when Retry, pressed from it, goes (WCAG 2.4.3)", async () => {
    const { clone, held } = heldClone();
    core({ clone });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma", ["api"]);
    const card = await findCard("gamma");
    await waitFor(() => expect(held).toHaveLength(1));
    held[0].refuse("api: clone failed — no access.");
    const retry = await within(card).findByRole("button", { name: "Retry" });
    retry.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(card).toHaveTextContent("Cloning 1 repo…"));
    expect(document.activeElement).not.toBe(document.body);
    expect(card.contains(document.activeElement)).toBe(true);
  });

  it("keeps the keyboard on the line in the Notices centre when Retry, pressed there, goes", async () => {
    const { clone, held } = heldClone();
    core({ clone });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await make("gamma", ["api"]);
    await findCard("gamma");
    await waitFor(() => expect(held).toHaveLength(1));
    held[0].refuse("api: clone failed — no access.");
    const kept = await results();
    const retry = await within(kept).findByRole("button", { name: "Retry" });
    retry.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(kept).toHaveTextContent("Cloning 1 repo…"));
    expect(document.activeElement).not.toBe(document.body);
    expect(kept.contains(document.activeElement)).toBe(true);
  });
});

/** alpha's repo `web`, left on a chat's branch (#1985). */
const WEB_OFF_MAIN: StaleClone = {
  workspace: "alpha",
  repo: "web",
  branch: { branch: "chat-3", default: "main" },
  remote: null,
};

describe("a workspace's stale repo, on its chip (#3650)", () => {
  it("hangs from the chip behind a ▲ dot, and its Switch acts from the card", async () => {
    const { asked } = core({ stale: [WEB_OFF_MAIN] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    // Found as the project is read, not caused by a press: behind its dot, a warning's.
    const dot = await waitFor(() => {
      const found = within(bar()).queryByRole("button", { name: /^Show the notice on alpha/ });
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(dot).toHaveAttribute("data-kind", "warning");
    await userEvent.click(dot);
    const card = await findCard("alpha");
    expect(card).toHaveTextContent("web in alpha is on chat-3, not main.");
    // Still listed in the Notices centre, outside the card.
    expect(linesSaying(/web in alpha is on chat-3/).some((one) => !card.contains(one))).toBe(true);

    await userEvent.click(within(card).getByRole("button", { name: "Switch to main" }));
    await waitFor(() =>
      expect(
        asked.some(
          (one) =>
            one.cmd === "fix_stale_clone" &&
            one.args.workspace === "alpha" &&
            one.args.repo === "web" &&
            one.args.fix === "branch",
        ),
      ).toBe(true),
    );
    // Fixed: gone from the card and the Notices centre, which read the repos again.
    await waitFor(() => expect(linesSaying(/web in alpha is on chat-3/)).toHaveLength(0));
  });

  it("is dismissed in both places from the card's ×, by the one record", async () => {
    core({ stale: [WEB_OFF_MAIN] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await waitFor(() =>
      expect(
        within(bar()).queryByRole("button", { name: /^Show the notice on alpha/ }),
      ).not.toBeNull(),
    );
    await userEvent.click(within(bar()).getByRole("button", { name: /^Show the notice on alpha/ }));
    const card = await findCard("alpha");
    await userEvent.click(within(card).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(queryCard("alpha")).toBeNull());
    await waitFor(() => expect(linesSaying(/web in alpha is on chat-3/)).toHaveLength(0));
    expect(within(bar()).queryByRole("button", { name: /^Show the notice on alpha/ })).toBeNull();
  });
});

describe("a workspace behind the current layout, on its chip (#3650)", () => {
  it("hangs from the chip with its Update, which mends it from the card", async () => {
    const { asked } = core({ behind: ["alpha"] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    const dot = await waitFor(() => {
      const found = within(bar()).queryByRole("button", { name: /^Show the notice on alpha/ });
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(dot).toHaveAttribute("data-kind", "warning");
    await userEvent.click(dot);
    const card = await findCard("alpha");
    expect(card).toHaveTextContent("alpha is behind the current layout.");
    // It goes when it is mended, as the Inbox's alert does: no Dismiss of its own.
    expect(within(card).queryByRole("button", { name: "Dismiss" })).toBeNull();

    await userEvent.click(within(card).getByRole("button", { name: "Update workspace layout" }));
    await waitFor(() =>
      expect(
        asked.some((one) => one.cmd === "plane_doctor_fix" && one.args.fix === "workspace-reinit"),
      ).toBe(true),
    );
    await waitFor(() => expect(queryCard("alpha")).toBeNull());
    expect(within(bar()).queryByRole("button", { name: /^Show the notice on alpha/ })).toBeNull();
  });
});

/** alpha's repo `api`, on a chat's branch too: a second line on the same card (#3930's review). */
const API_OFF_MAIN: StaleClone = { ...WEB_OFF_MAIN, repo: "api" };

/** The dot on alpha's chip, once the project's findings are read. */
const alphaDot = () =>
  waitFor(() => {
    const found = within(bar()).queryByRole("button", { name: /^Show the .* on alpha/ });
    expect(found).not.toBeNull();
    return found as HTMLElement;
  });

describe("the keyboard on a card whose lines go one by one (#3930's review, WCAG 2.4.3)", () => {
  it("stays in the card when one of two stale repos is switched and its line goes", async () => {
    core({ stale: [WEB_OFF_MAIN, API_OFF_MAIN] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    (await alphaDot()).focus();
    await userEvent.keyboard("{Enter}");
    const card = await findCard("alpha");
    await waitFor(() => expect(card.contains(document.activeElement)).toBe(true));
    const web = card.querySelector<HTMLElement>('[data-cause^="stale-clone:alpha/web"]');
    expect(web).not.toBeNull();
    within(web as HTMLElement)
      .getByRole("button", { name: "Switch to main" })
      .focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(linesSaying(/web in alpha is on chat-3/)).toHaveLength(0));
    // api's line keeps the card up, and the keyboard is still in it, never on the page.
    expect(card.isConnected).toBe(true);
    await waitFor(() => expect(card.contains(document.activeElement)).toBe(true));
    expect(document.activeElement).not.toBe(document.body);
  });

  it("stays in the card when the layout is updated and a stale repo is left", async () => {
    core({ stale: [WEB_OFF_MAIN], behind: ["alpha"] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    (await alphaDot()).focus();
    await userEvent.keyboard("{Enter}");
    const card = await findCard("alpha");
    await waitFor(() => expect(card).toHaveTextContent("alpha is behind the current layout."));
    within(card).getByRole("button", { name: "Update workspace layout" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(card).not.toHaveTextContent("alpha is behind the current layout."));
    expect(card.isConnected).toBe(true);
    await waitFor(() => expect(card.contains(document.activeElement)).toBe(true));
    expect(document.activeElement).not.toBe(document.body);
  });

  it("puts a card that holds only the layout line away by Esc, back behind its dot", async () => {
    core({ behind: ["alpha"] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    (await alphaDot()).focus();
    await userEvent.keyboard("{Enter}");
    const card = await findCard("alpha");
    await waitFor(() => expect(card).toHaveFocus());
    await userEvent.keyboard("{Escape}");
    // Hidden, not dismissed: the line goes only when the layout is updated.
    await waitFor(() => expect(queryCard("alpha")).toBeNull());
    expect(await alphaDot()).toHaveAttribute("data-kind", "warning");
    // And the keyboard is on the chip it hung from (D-3648-21).
    await waitFor(() =>
      expect(document.activeElement).toBe(within(bar()).getByRole("button", { name: "alpha" })),
    );
  });
});

/** alpha's repo `web`, on a remote the forge says moved (#1985, #2117). */
const WEB_MOVED: StaleClone = {
  workspace: "alpha",
  repo: "web",
  branch: null,
  remote: {
    origin: "github.com/acme/web",
    now: "https://github.com/purlis/web.git",
    pushes_follow: true,
    pushes_kept: [],
  },
};

describe("a workspace's repo on a moved remote, on its chip (#3930's review)", () => {
  it("sets the address the card shows, from the card", async () => {
    const { asked } = core({ stale: [WEB_MOVED] });
    opened();
    await waitFor(() => expect(chips()).toEqual(["alpha*"]));
    await userEvent.click(await alphaDot());
    const card = await findCard("alpha");
    expect(card).toHaveTextContent(
      "and the forge says it moved to https://github.com/purlis/web.git",
    );
    await userEvent.click(within(card).getByRole("button", { name: "Use the new address" }));
    await waitFor(() =>
      expect(
        asked.some(
          (one) =>
            one.cmd === "fix_stale_clone" &&
            one.args.repo === "web" &&
            one.args.fix === "remote" &&
            one.args.shown === "https://github.com/purlis/web.git",
        ),
      ).toBe(true),
    );
    await waitFor(() => expect(linesSaying(/web in alpha fetches from/)).toHaveLength(0));
  });
});
