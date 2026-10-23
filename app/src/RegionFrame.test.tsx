// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode, useEffect, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { REGION_CHROME, RegionFrame, stepRegions } from "./RegionFrame";
import { PanelSection } from "./PanelSection";
import { ActivityCount } from "./ActivityBar";
import { drawnWith } from "./cascade.testkit";
import { wears } from "./textRoles.testkit";
import {
  DEFAULT_ARRANGEMENT,
  viewMoved,
  type Arrangement,
  type RegionId,
  type PanelViewId,
  type Side,
  type ViewId,
} from "./regions";
import { Flame, type LucideIcon } from "lucide-react";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

/**
 * **The frame, against the real `react-resizable-panels`** — where a region's content lands,
 * what order it is in, and what moving one costs.
 *
 * jsdom gives every element a size of zero, so the library defers its own layout and no panel
 * is ever given a width here. Everything that is about a *size* is in `RegionFrame.sizes.test`,
 * which mocks the library to read what it was told; this file is about the tree.
 */

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const CONTENT: Partial<Record<RegionId, React.ReactNode>> = {
  navigation: <div data-testid="c-navigation">navigation</div>,
  aside: <div data-testid="c-aside">aside</div>,
};

/** The frame, with a button that rearranges it — standing in for whatever eventually does. */
function Harness({ from, to }: { from: Arrangement; to?: Arrangement }) {
  const [arrangement, setArrangement] = useState(from);
  return (
    <>
      <button onClick={() => to !== undefined && setArrangement(to)}>rearrange</button>
      <RegionFrame
        arrangement={arrangement}
        content={CONTENT}
        centre={<div data-testid="centre">centre</div>}
        onResized={vi.fn()}
      />
    </>
  );
}

/** The panel a slot is drawn in. `react-resizable-panels` puts the id on `data-testid`. */
const slot = (side: Side) => screen.getByTestId(`region-${side}`);

const rearrange = () => userEvent.click(screen.getByRole("button", { name: "rearrange" }));

const on = (id: RegionId, side: Side, order = 0, collapsed = false) => ({
  id,
  side,
  order,
  collapsed,
});

afterEach(cleanup);

describe("where a region is drawn", () => {
  it("puts each one in the slot the arrangement names", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    expect(within(slot("left")).getByTestId("c-navigation")).toBeInTheDocument();
    expect(within(slot("right")).getByTestId("c-aside")).toBeInTheDocument();
  });

  it("has a slot on each side, and the Panel under the three of them (#1737)", () => {
    render(<Harness from={DEFAULT_ARRANGEMENT} />);

    expect([...document.querySelectorAll("[data-panel]")].map((one) => one.id)).toEqual([
      "region-upper",
      "region-left",
      "region-centre",
      "region-right",
      "region-panel",
      // The Panel's halves, in a group of their own (#1738).
      "panel-half-left",
      "panel-half-right",
    ]);
    // Full width: the sidebars and the editor are above it, inside one panel of the same group.
    const upper = screen.getByTestId("region-upper");
    for (const side of ["left", "centre", "right"]) {
      expect(upper).toContainElement(screen.getByTestId(`region-${side}`));
    }
    expect(upper).not.toContainElement(screen.getByTestId("region-panel"));
  });

  it("draws it on the other side when the arrangement says so, and no JSX moves", () => {
    // The whole point of the ticket: the attention region on the left is a change to the data.
    render(<Harness from={[on("navigation", "right"), on("aside", "left")]} />);

    expect(within(slot("left")).getByTestId("c-aside")).toBeInTheDocument();
    expect(within(slot("right")).queryByTestId("c-aside")).not.toBeInTheDocument();
  });

  it("draws two regions in one slot, in the order the arrangement gives them", () => {
    render(<Harness from={[on("navigation", "left", 1), on("aside", "left", 0)]} />);

    const drawn = within(slot("left"))
      .getAllByTestId(/^c-/)
      .map((one) => one.dataset.testid);
    expect(drawn).toEqual(["c-aside", "c-navigation"]);
  });

  it("draws nothing in a slot every region has left, and keeps the slot", () => {
    render(<Harness from={[on("navigation", "right", 1), on("aside", "right")]} />);

    expect(within(slot("left")).queryAllByTestId(/^c-/)).toEqual([]);
    expect(within(slot("right")).getByTestId("c-navigation")).toBeInTheDocument();
  });
});

describe("a region put away", () => {
  it("is unmounted rather than sized to nothing, so it is out of the tab order too", async () => {
    render(
      <Harness
        from={DEFAULT_ARRANGEMENT}
        to={[on("navigation", "left", 0, true), on("aside", "right")]}
      />,
    );
    expect(screen.getByTestId("c-navigation")).toBeInTheDocument();

    await rearrange();

    expect(screen.queryByTestId("c-navigation")).not.toBeInTheDocument();
  });

  it("leaves its handle in the group with nothing to drag", async () => {
    render(
      <Harness
        from={DEFAULT_ARRANGEMENT}
        to={[on("navigation", "left", 0, true), on("aside", "right")]}
      />,
    );

    await rearrange();

    // Taking the separator out from under a live group is charter-app#141's throw. It stays,
    // and `edge-gone` is what makes it invisible and unhittable.
    const handle = document.querySelector('[aria-controls="region-left"]');
    expect(handle).not.toBeNull();
    expect(handle).toHaveClass("edge-gone");
  });
});

describe("what a rearrangement may not cost", () => {
  it("never adds a panel to a live group, nor takes one out", async () => {
    // charter-app#141: a panel leaving a live group throws *"Panel constraints not found for
    // index 3"* from a document listener no `try` of ours can reach. The slots are fixed so
    // that the data is free to move; this is the assertion that says so.
    render(
      <Harness
        from={DEFAULT_ARRANGEMENT}
        to={[on("navigation", "right", 1, true), on("aside", "left")]}
      />,
    );
    const before = [...document.querySelectorAll("[data-panel]")].map((one) => one.id);

    await rearrange();

    expect([...document.querySelectorAll("[data-panel]")].map((one) => one.id)).toEqual(before);
    expect(before).toEqual([
      "region-upper",
      "region-left",
      "region-centre",
      "region-right",
      "region-panel",
      "panel-half-left",
      "panel-half-right",
    ]);
  });

  it("never takes the terminal panes with it", async () => {
    // The centre is the product. A layout change that remounted it would tear down every
    // xterm in the window and reopen it — which is what keying the group on the arrangement,
    // the other way of doing this, would have done.
    render(
      <Harness
        from={DEFAULT_ARRANGEMENT}
        to={[on("navigation", "right", 1), on("aside", "right")]}
      />,
    );
    const centre = screen.getByTestId("centre");

    await rearrange();

    expect(screen.getByTestId("centre")).toBe(centre);
  });
});

/** The views of both sides, as a window hands them over. */
const VIEWS: Partial<Record<ViewId, React.ReactNode>> = {
  chats: <div data-testid="v-chats">chats</div>,
  explorer: <div data-testid="v-explorer">explorer</div>,
  search: <div data-testid="v-search">search</div>,
  changes: <div data-testid="v-changes">changes</div>,
  repos: <div data-testid="v-repos">repos</div>,
  todos: <div data-testid="v-todos">todos</div>,
  memory: (
    <PanelSection
      testid="panel-memory"
      mark={Flame}
      title="Memory"
      actions={<button type="button">New memory</button>}
    >
      <div data-testid="v-memory">memory</div>
    </PanelSection>
  ),
  inbox: <div data-testid="v-inbox">inbox</div>,
  personas: <div data-testid="v-personas">personas</div>,
  sessions: <div data-testid="v-sessions">sessions</div>,
  vaults: <div data-testid="v-vaults">vaults</div>,
  "panel:ext/stats/burn": <div data-testid="v-burn">burn</div>,
};

/** The frame with views, an activity bar and a badge, rearranged by `to` on the button. */
function WithViews({
  from,
  to,
  onPick = vi.fn(),
  badge,
  badges,
  panels = [],
}: {
  from: Arrangement;
  to?: Arrangement;
  onPick?: (view: ViewId) => void;
  badge?: React.ReactNode;
  /** Badges by view, in place of {@link badge}'s one on Chats. */
  badges?: Partial<Record<ViewId, React.ReactNode>>;
  panels?: { view: PanelViewId; name: string; mark: LucideIcon }[];
}) {
  const [arrangement, setArrangement] = useState(from);
  return (
    <>
      <button onClick={() => to !== undefined && setArrangement(to)}>rearrange</button>
      <RegionFrame
        arrangement={arrangement}
        content={CONTENT}
        views={VIEWS}
        panels={panels}
        badges={badges ?? { chats: badge }}
        onPick={onPick}
        centre={<div data-testid="centre">centre</div>}
        onResized={vi.fn()}
      />
    </>
  );
}

const bar = () => screen.getByRole("tablist", { name: "Navigation" });

describe("the activity bar of a region with views (#1673)", () => {
  it("is a vertical tab list beside the side, one tab per view, each named and with a tooltip", () => {
    render(<WithViews from={DEFAULT_ARRANGEMENT} />);

    expect(bar()).toHaveAttribute("aria-orientation", "vertical");
    const tabs = within(bar()).getAllByRole("tab");
    expect(tabs.map((tab) => tab.getAttribute("aria-label"))).toEqual([
      "Chats",
      "Explorer",
      "Search",
    ]);
    for (const tab of tabs) expect(tipOf(tab)).toBeTruthy();
    // Outside the slot, so it stays when the slot is put away.
    expect(slot("left").contains(bar())).toBe(false);
  });

  it("selects the open view's tab and shows that view alone, the other mounted and hidden", () => {
    render(<WithViews from={DEFAULT_ARRANGEMENT} />);

    expect(screen.getByRole("tab", { name: "Chats" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "Explorer" })).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("tabpanel", { name: "Chats" })).toContainElement(
      screen.getByTestId("v-chats"),
    );
    expect(screen.getByTestId("v-explorer")).toBeInTheDocument();
    expect(screen.queryByRole("tabpanel", { name: "Explorer" })).toBeNull();
    // Each tab says which panel it controls.
    expect(screen.getByRole("tab", { name: "Chats" })).toHaveAttribute(
      "aria-controls",
      screen.getByRole("tabpanel", { name: "Chats" }).id,
    );
  });

  it("draws a hidden view as nothing, though the stylesheet lays the views out as boxes", () => {
    render(<WithViews from={DEFAULT_ARRANGEMENT} />);

    // `hidden` is the browser's `display: none`, which any `display` a class sets outranks: the
    // real window drew the Explorer under the Chats view until App.css said it again (train 34).
    const open = screen.getByRole("tabpanel", { name: "Chats" });
    const shut = screen.getByTestId("v-explorer").closest('[role="tabpanel"]') as HTMLElement;
    expect(shut).toHaveAttribute("hidden");
    expect(drawnWith(open, "display")).toBe("flex");
    expect(drawnWith(shut, "display")).toBe("none");
  });

  it("says a press of a tab, and the content does not change until the arrangement does", async () => {
    const onPick = vi.fn();
    render(<WithViews from={DEFAULT_ARRANGEMENT} onPick={onPick} />);

    await userEvent.click(screen.getByRole("tab", { name: "Explorer" }));
    await userEvent.click(screen.getByRole("tab", { name: "Chats" }));

    expect(onPick.mock.calls).toEqual([["explorer"], ["chats"]]);
  });

  it("is reached with the arrows and pressed with Enter, as a tab list is", async () => {
    const onPick = vi.fn();
    render(<WithViews from={DEFAULT_ARRANGEMENT} onPick={onPick} />);

    screen.getByRole("tab", { name: "Chats" }).focus();
    await userEvent.keyboard("{ArrowDown}");
    expect(screen.getByRole("tab", { name: "Explorer" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");

    expect(onPick).toHaveBeenCalledWith("explorer");
  });

  it("keeps the views mounted when the side is put away, and selects no tab", async () => {
    render(
      <WithViews
        from={DEFAULT_ARRANGEMENT}
        to={[on("navigation", "left", 0, true), on("aside", "right")]}
      />,
    );
    const chats = screen.getByTestId("v-chats");

    await rearrange();

    expect(screen.getByTestId("v-chats")).toBe(chats);
    expect(within(slot("left")).queryByRole("tabpanel")).toBeNull();
    expect(within(bar()).queryAllByRole("tab", { selected: true })).toEqual([]);
  });

  it("goes with the region to the other side", async () => {
    render(
      <WithViews
        from={DEFAULT_ARRANGEMENT}
        to={[on("navigation", "right", 1), on("aside", "right")]}
      />,
    );

    await rearrange();

    expect(bar().closest("[data-side]")).toHaveAttribute("data-side", "right");
  });

  it("draws a view's badge on its tab, and the tab is described by it", () => {
    render(
      <WithViews
        from={[on("navigation", "left", 0, true), on("aside", "right")]}
        badge={<ActivityCount count={2} said="2 chats need you" tone="needs-you" />}
      />,
    );

    const chats = screen.getByRole("tab", { name: "Chats" });
    expect(chats).toHaveTextContent("2");
    expect(chats).toHaveAccessibleDescription("2 chats need you");
  });
});

describe("the right side's activity bar (#1678)", () => {
  const right = () => screen.getByRole("tablist", { name: "Attention" });

  it("holds the Inbox, Personas and Vaults, opens on the Inbox, and sits at the right edge (#1738)", () => {
    render(<WithViews from={DEFAULT_ARRANGEMENT} />);

    expect(
      within(right())
        .getAllByRole("tab")
        .map((tab) => tab.getAttribute("aria-label")),
    ).toEqual(["Inbox", "Personas", "Vaults"]);
    expect(right().closest("[data-side]")).toHaveAttribute("data-side", "right");
    expect(within(slot("right")).getByRole("tabpanel", { name: "Inbox" })).toContainElement(
      screen.getByTestId("v-inbox"),
    );
    // The others are mounted and hidden.
    expect(screen.getByTestId("v-personas")).toBeInTheDocument();
    expect(screen.queryByRole("tabpanel", { name: "Personas" })).toBeNull();
  });

  it("gives each extension's panel a tab after purlis's own, named and marked as it says", () => {
    render(
      <WithViews
        from={DEFAULT_ARRANGEMENT}
        panels={[{ view: "panel:ext/stats/burn", name: "Burn rate", mark: Flame }]}
      />,
    );

    const tabs = within(right()).getAllByRole("tab");
    expect(tabs.map((tab) => tab.getAttribute("aria-label")).at(-1)).toBe("Burn rate");
    // An icon's tooltip: its name, then what a press does.
    expect(tipOf(tabs.at(-1))).toEqual("Burn rateShow the Burn rate view");
  });

  it("shows an extension's panel when it is the open view", () => {
    render(
      <WithViews
        from={[on("navigation", "left"), { ...on("aside", "right"), view: "panel:ext/stats/burn" }]}
        panels={[{ view: "panel:ext/stats/burn", name: "Burn rate", mark: Flame }]}
      />,
    );

    expect(screen.getByRole("tabpanel", { name: "Burn rate" })).toContainElement(
      screen.getByTestId("v-burn"),
    );
  });

  it("opens on the Inbox when the panel picked last is not contributed any more", () => {
    render(
      <WithViews
        from={[on("navigation", "left"), { ...on("aside", "right"), view: "panel:ext/stats/burn" }]}
      />,
    );

    expect(screen.getByRole("tabpanel", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.queryByTestId("v-burn")).toBeNull();
  });
});

/**
 * **The Panel and its left half** (spec #1736, P-1, P-5; #1737): a full-width slot under both
 * sidebars and the editor, its halves switched by the Lower icons of each side's activity bar.
 */
describe("the Panel (#1737)", () => {
  const lower = () => screen.getByRole("tablist", { name: "Panel, left half" });

  it("puts the Lower icons below the left bar's own, after a divider, Repos and Changes", () => {
    render(<WithViews from={DEFAULT_ARRANGEMENT} />);

    expect(
      within(lower())
        .getAllByRole("tab")
        .map((tab) => tab.getAttribute("aria-label")),
    ).toEqual(["Repos", "Changes"]);
    // One icon column per side: the upper group, the divider, then the lower group.
    const column = bar().closest(".activity-column");
    expect(column).toHaveAttribute("data-side", "left");
    expect(column).toContainElement(lower());
    const order = [...(column?.children ?? [])].map((one) =>
      one.classList.contains("activity-divider") ? "divider" : one.getAttribute("data-region"),
    );
    expect(order).toEqual(["navigation", "divider", "panel-left"]);
    expect(tipOf(within(lower()).getByRole("tab", { name: "Repos" }))).toEqual(
      "ReposPut the panel's left half away",
    );
  });

  it("draws the left half's open view in the Panel", () => {
    render(<WithViews from={DEFAULT_ARRANGEMENT} />);

    expect(
      within(screen.getByTestId("region-panel")).getByRole("tabpanel", { name: "Repos" }),
    ).toContainElement(screen.getByTestId("v-repos"));
  });

  it("takes the Panel off the window when both halves are away, keeping the slot and Repos", async () => {
    const away = DEFAULT_ARRANGEMENT.map((one) =>
      one.id.startsWith("panel-") ? { ...one, collapsed: true } : one,
    );
    render(<WithViews from={DEFAULT_ARRANGEMENT} to={away} />);
    const repos = screen.getByTestId("v-repos");

    await rearrange();

    expect(screen.queryByRole("tabpanel", { name: "Repos" })).toBeNull();
    expect(screen.getByTestId("v-repos")).toBe(repos);
    expect(document.querySelector('[aria-controls="region-upper"]')).toHaveClass("edge-gone");
    expect(within(lower()).queryAllByRole("tab", { selected: true })).toEqual([]);
  });

  it("says a press of a Lower icon, as of any other", async () => {
    const onPick = vi.fn();
    render(<WithViews from={DEFAULT_ARRANGEMENT} onPick={onPick} />);

    await userEvent.click(within(lower()).getByRole("tab", { name: "Repos" }));

    expect(onPick).toHaveBeenCalledWith("repos");
  });

  it("draws a Lower icon's badge while its half is away, through the bars' one badge path (P-9)", async () => {
    const away = DEFAULT_ARRANGEMENT.map((one) =>
      one.id === "panel-left" ? { ...one, collapsed: true } : one,
    );
    render(
      <WithViews
        from={DEFAULT_ARRANGEMENT}
        to={away}
        badges={{ changes: <ActivityCount count={4} said="4 uncommitted files" /> }}
      />,
    );

    await rearrange();

    expect(screen.queryByRole("tabpanel", { name: "Changes" })).toBeNull();
    const changes = within(lower()).getByRole("tab", { name: "Changes" });
    expect(changes).toHaveTextContent("4");
    expect(changes).toHaveAccessibleDescription("4 uncommitted files");
  });
});

/**
 * **The Panel's right half** (#1738, P-2, P-5, P-11): Memory, Todos and Sessions on the right
 * bar's Lower icons, each half with a one-line header of its own and no tabs inside it.
 */
describe("the Panel's right half (#1738)", () => {
  const lowerRight = () => screen.getByRole("tablist", { name: "Panel, right half" });
  const half = (side: Side) =>
    screen
      .getByTestId("region-panel")
      .querySelector(`.panel-half[data-side="${side}"]`) as HTMLElement;

  it("puts Memory, Todos and Sessions on the right bar's Lower icons, after a divider", () => {
    render(<WithViews from={DEFAULT_ARRANGEMENT} />);

    expect(
      within(lowerRight())
        .getAllByRole("tab")
        .map((tab) => tab.getAttribute("aria-label")),
    ).toEqual(["Memory", "Todos", "Sessions"]);
    const column = lowerRight().closest(".activity-column");
    expect(column).toHaveAttribute("data-side", "right");
    const order = [...(column?.children ?? [])].map((one) =>
      one.classList.contains("activity-divider") ? "divider" : one.getAttribute("data-region"),
    );
    expect(order).toEqual(["aside", "divider", "panel-right"]);
    expect(tipOf(within(lowerRight()).getByRole("tab", { name: "Memory" }))).toEqual(
      "MemoryPut the panel's right half away",
    );
  });

  it("draws Memory in the right half beside Repos in the left, the sidebar keeping neither", () => {
    render(<WithViews from={DEFAULT_ARRANGEMENT} />);

    expect(within(half("right")).getByRole("tabpanel", { name: "Memory" })).toContainElement(
      screen.getByTestId("v-memory"),
    );
    expect(within(half("left")).getByRole("tabpanel", { name: "Repos" })).toBeInTheDocument();
    expect(within(slot("right")).queryByTestId("v-memory")).toBeNull();
    // Todos and Sessions are mounted in the half, hidden.
    expect(half("right")).toContainElement(screen.getByTestId("v-todos"));
    expect(screen.queryByRole("tabpanel", { name: "Todos" })).toBeNull();
  });

  it("heads each half with one line: the open view's title, its own actions and a chevron (P-11)", async () => {
    const onPick = vi.fn();
    render(<WithViews from={DEFAULT_ARRANGEMENT} onPick={onPick} />);

    const head = half("right").querySelector(".half-head") as HTMLElement;
    expect(within(head).getByRole("heading", { name: "Memory" })).toBeInTheDocument();
    // The half's title is the one section label (#1920), as a sidebar section's is.
    wears(within(head).getByRole("heading", { name: "Memory" }), "label", "the half's title");
    wears(
      within(half("left")).getByRole("heading", { name: "Repos" }),
      "label",
      "the other half's title",
    );
    // The view's own action is on the half's line, and its title is said once, there.
    expect(within(head).getByRole("button", { name: "New memory" })).toBeInTheDocument();
    expect(within(half("right")).getAllByRole("heading", { name: "Memory" })).toHaveLength(1);
    const chevron = within(head).getByRole("button", { name: "Put the panel's right half away" });
    // No tabs inside a half: its Lower icons are the only switch.
    expect(within(half("right")).queryAllByRole("tablist")).toEqual([]);
    expect(within(half("right")).queryAllByRole("tab")).toEqual([]);

    await userEvent.click(chevron);

    expect(onPick).toHaveBeenCalledWith("memory");
    // The left half's line too, with no actions of its own.
    const left = half("left").querySelector(".half-head") as HTMLElement;
    expect(within(left).getByRole("heading", { name: "Repos" })).toBeInTheDocument();
    expect(
      within(left)
        .getAllByRole("button")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Put the panel's left half away"]);
  });

  it("titles the half after the view that is open, and draws only that view's actions", async () => {
    const todos = DEFAULT_ARRANGEMENT.map((one) =>
      one.id === "panel-right" ? { ...one, view: "todos" as const } : one,
    );
    render(<WithViews from={DEFAULT_ARRANGEMENT} to={todos} />);

    await rearrange();

    const head = half("right").querySelector(".half-head") as HTMLElement;
    expect(within(head).getByRole("heading", { name: "Todos" })).toBeInTheDocument();
    expect(within(head).queryByRole("button", { name: "New memory" })).toBeNull();
  });

  it("draws the other half alone when one is put away (P-5)", async () => {
    const away = DEFAULT_ARRANGEMENT.map((one) =>
      one.id === "panel-left" ? { ...one, collapsed: true } : one,
    );
    render(<WithViews from={DEFAULT_ARRANGEMENT} to={away} />);

    await rearrange();

    expect(half("left")).toHaveAttribute("hidden");
    expect(half("right")).not.toHaveAttribute("hidden");
    // No handle between a half and nothing.
    expect(screen.getByTestId("region-panel").querySelector("[data-separator]")).toHaveClass(
      "edge-gone",
    );
  });
});

describe("a half whose only views are panels no extension contributes now (#1752)", () => {
  const half = (side: Side) =>
    screen
      .getByTestId("region-panel")
      .querySelector(`.panel-half[data-side="${side}"]`) as HTMLElement;
  /** The right half holding only Burn rate, moved in while its extension contributed it. */
  const onlyBurn = DEFAULT_ARRANGEMENT.map((one) =>
    one.id === "panel-right"
      ? { ...one, views: ["panel:ext/stats/burn" as const], view: "panel:ext/stats/burn" as const }
      : one,
  );

  it("is not drawn, so the left half has the Panel to itself", () => {
    render(<WithViews from={onlyBurn} />);

    expect(half("right")).toHaveAttribute("hidden");
    expect(half("left")).not.toHaveAttribute("hidden");
    expect(screen.queryByRole("tablist", { name: "Panel, right half" })).toBeNull();
  });

  it("is drawn with its panel while the extension contributes it", () => {
    render(
      <WithViews
        from={onlyBurn}
        panels={[{ view: "panel:ext/stats/burn", name: "Burn rate", mark: Flame }]}
      />,
    );

    expect(half("right")).not.toHaveAttribute("hidden");
    expect(within(half("right")).getByTestId("v-burn")).toBeInTheDocument();
  });
});

describe("every region F6 stops in is a named region (#1751)", () => {
  it("names the sidebars by the catalogue, the editor Editor, and each half of the Panel", () => {
    render(<WithViews from={DEFAULT_ARRANGEMENT} />);

    // Named ones: a region with no name is no landmark (a slot put away, the Panel's own).
    expect(
      screen
        .getAllByRole("region")
        .map((one) => one.getAttribute("aria-label"))
        .filter((name) => name !== null)
        .sort(),
    ).toEqual(["Attention", "Editor", "Navigation", "Panel, left half", "Panel, right half"]);
    expect(screen.getByRole("region", { name: "Editor" })).toContainElement(
      screen.getByTestId("centre"),
    );
    expect(screen.getByRole("region", { name: "Navigation" })).toContainElement(
      screen.getByTestId("v-chats"),
    );
  });

  it("is no region while its side is put away", async () => {
    const away = DEFAULT_ARRANGEMENT.map((one) =>
      one.id === "navigation" ? { ...one, collapsed: true } : one,
    );
    render(<WithViews from={DEFAULT_ARRANGEMENT} to={away} />);

    await rearrange();

    expect(screen.queryByRole("region", { name: "Navigation" })).toBeNull();
    expect(screen.getByRole("region", { name: "Editor" })).toBeInTheDocument();
  });
});

describe("F6 into the editor lands on the pane, past its tab strip (DS-6, #1836)", () => {
  /** The editor as `PlaneView` draws it: the chat strip, then the panes under it. */
  const editor = (
    <div className="editor">
      <header className="bar" {...{ [REGION_CHROME]: "" }}>
        <button type="button" tabIndex={0}>
          New tab
        </button>
      </header>
      <div className="panes">
        <button type="button" tabIndex={0}>
          Open the first chat
        </button>
      </div>
    </div>
  );

  it("passes over the strip to the first stop below it", () => {
    render(
      <RegionFrame
        arrangement={DEFAULT_ARRANGEMENT}
        content={CONTENT}
        views={VIEWS}
        centre={editor}
        onResized={vi.fn()}
      />,
    );
    const centre = screen.getByRole("region", { name: "Editor" });

    // F6 from the palette walks the cycle; whichever region it starts in, it comes to the editor.
    for (let step = 0; step < 6 && !centre.contains(document.activeElement); step++)
      act(() => stepRegions(false));

    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Open the first chat" }),
    );
  });
});

describe("a moved view keeps where it was scrolled (#1752)", () => {
  const searchOpen = DEFAULT_ARRANGEMENT.map((one) =>
    one.id === "navigation" ? { ...one, view: "search" as const } : one,
  );
  const panelOf = () => screen.getByTestId("v-search").closest('[role="tabpanel"]') as HTMLElement;

  it("is scrolled back in the region it moved to", async () => {
    render(<WithViews from={searchOpen} to={viewMoved(searchOpen, "search", "panel-left")} />);
    const before = panelOf();
    before.scrollTop = 120;
    fireEvent.scroll(before);

    await rearrange();

    // The same element, put in the Panel (#1752): it lost its scroll on the way, and has it back.
    const after = panelOf();
    expect(after).toBe(before);
    expect(after.closest(".panel-half")).not.toBeNull();
    expect(after.scrollTop).toBe(120);
  });

  it("starts at the top when drawn again where it was", async () => {
    const { unmount } = render(<WithViews from={searchOpen} />);
    panelOf().scrollTop = 80;
    fireEvent.scroll(panelOf());
    unmount();

    render(<WithViews from={searchOpen} />);

    expect(panelOf().scrollTop).toBe(0);
  });
});

describe("a moved view keeps what it held (#1752)", () => {
  /** How many times each typed view was mounted: a move must not mount it again. */
  const mounted = new Map<string, number>();
  /** A view whose words are its own state, as a search box's query is. */
  function Typed({ name }: { name: string }) {
    const [text, setText] = useState("");
    useEffect(() => {
      mounted.set(name, (mounted.get(name) ?? 0) + 1);
    }, [name]);
    return (
      <input aria-label={`${name} words`} value={text} onChange={(e) => setText(e.target.value)} />
    );
  }
  const searchOpen = DEFAULT_ARRANGEMENT.map((one) =>
    one.id === "navigation" ? { ...one, view: "search" as const } : one,
  );
  /** The frame with Search and Memory holding typed words, moved by `rearrange`. */
  function Moving({ from, to }: { from: Arrangement; to: Arrangement }) {
    const [arrangement, setArrangement] = useState(from);
    return (
      <>
        <button onClick={() => setArrangement(to)}>rearrange</button>
        <RegionFrame
          arrangement={arrangement}
          content={CONTENT}
          views={{
            ...VIEWS,
            search: <Typed name="search" />,
            memory: (
              <PanelSection
                testid="panel-memory"
                mark={Flame}
                title="Memory"
                actions={<button type="button">New memory</button>}
              >
                <Typed name="memory" />
              </PanelSection>
            ),
          }}
          centre={<div data-testid="centre">centre</div>}
          onResized={vi.fn()}
        />
      </>
    );
  }
  afterEach(() => mounted.clear());
  /** Types into a view's box from the keyboard. Not by a click: in jsdom every element is at
   *  the origin, inside the handles' press band, so a click lands on a handle. */
  const typeInto = async (name: string, text: string) => {
    const box = screen.getByRole("textbox", { name });
    act(() => box.focus());
    await userEvent.keyboard(text);
    expect(box).toHaveValue(text);
  };

  it("keeps the words typed into a view moved from its sidebar to the Panel, mounted once", async () => {
    render(<Moving from={searchOpen} to={viewMoved(searchOpen, "search", "panel-left")} />);
    await typeInto("search words", "deploy");
    const was = mounted.get("search");

    await rearrange();

    const box = screen.getByRole("textbox", { name: "search words" });
    expect(box.closest(".panel-half")).not.toBeNull();
    expect(box).toHaveValue("deploy");
    expect(mounted.get("search")).toBe(was);
  });

  it("keeps a view moved from its half to the sidebar, and its actions follow it", async () => {
    const memoryOpen = DEFAULT_ARRANGEMENT.map((one) =>
      one.id === "panel-right" ? { ...one, view: "memory" as const } : one,
    );
    const moved = viewMoved(memoryOpen, "memory", "aside").map((one) =>
      one.id === "aside" ? { ...one, view: "memory" as const } : one,
    );
    render(<Moving from={memoryOpen} to={moved} />);
    await typeInto("memory words", "kept");
    // In its half, the view's actions are on the half's line.
    expect(screen.getByRole("button", { name: "New memory" }).closest(".half-head")).not.toBeNull();
    const was = mounted.get("memory");

    await rearrange();

    const box = screen.getByRole("textbox", { name: "memory words" });
    expect(box.closest(".slot-right")).not.toBeNull();
    expect(box).toHaveValue("kept");
    expect(mounted.get("memory")).toBe(was);
    // In the sidebar, on its own heading.
    const button = screen.getByRole("button", { name: "New memory" });
    expect(button.closest(".half-head")).toBeNull();
    expect(button.closest('[data-scope="panel-section"][data-part="head"]')).not.toBeNull();
  });

  it("keeps the keyboard in a view moved while the person is in it", () => {
    // An element taken out of the document and put back loses the keyboard in a browser: a
    // move made while the keyboard is in the view (a key, the palette's row once closed) gives
    // it back where the view now is, never to the page.
    const frame = (arrangement: Arrangement) => (
      <StrictMode>
        <RegionFrame
          arrangement={arrangement}
          content={CONTENT}
          views={{ ...VIEWS, search: <Typed name="search" /> }}
          centre={<div data-testid="centre">centre</div>}
          onResized={vi.fn()}
        />
      </StrictMode>
    );
    const { rerender } = renderBare(frame(searchOpen));
    const box = screen.getByRole("textbox", { name: "search words" });
    act(() => box.focus());

    rerender(frame(viewMoved(searchOpen, "search", "panel-left")));

    expect(box.closest(".panel-half")).not.toBeNull();
    expect(box).toHaveFocus();
  });
});

describe("the right bar in Tab order (#1686, D-1686-2)", () => {
  /** What has the keyboard: a tab by its name, else the element's kind and place. */
  const said = () => {
    const at = document.activeElement as HTMLElement;
    if (at.getAttribute("role") === "tab") return `tab ${at.getAttribute("aria-label")}`;
    if (at.getAttribute("role") === "separator")
      return at.closest(".panel-halves")
        ? "separator between the halves"
        : at.parentElement?.classList.contains("region-row")
          ? "separator in the row"
          : "separator over the Panel";
    return `${at.tagName.toLowerCase()} ${at.textContent ?? ""}`.trim();
  };

  it("comes straight after the right sidebar, before the Panel, and back the same way", async () => {
    render(<WithViews from={DEFAULT_ARRANGEMENT} />);
    // The handle between the editor and the right sidebar, whose Inbox has nothing to press.
    const row = screen.getByTestId("centre").closest(".region-row") as HTMLElement;
    const handles = [...row.querySelectorAll<HTMLElement>(":scope > [role='separator']")];
    handles[handles.length - 1].focus();

    const forward: string[] = [];
    for (let press = 0; press < 4; press++) {
      await userEvent.tab();
      forward.push(said());
    }
    expect(forward).toEqual([
      "tab Inbox",
      "tab Memory",
      "separator over the Panel",
      "separator between the halves",
    ]);

    const back: string[] = [];
    for (let press = 0; press < 3; press++) {
      await userEvent.tab({ shift: true });
      back.push(said());
    }
    expect(back).toEqual(["separator over the Panel", "tab Memory", "tab Inbox"]);
    await userEvent.tab({ shift: true });
    expect(said()).toBe("separator in the row");
  });
});
