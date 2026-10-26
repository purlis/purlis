// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  Fragment,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactElement,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { Group, Panel, usePanelRef, type Layout } from "react-resizable-panels";
import { Separator } from "./shownSeparator";
import { useProjectShown } from "./projectShown";
import { X } from "lucide-react";
import { ActivityBar, VIEW_MARKS, type PanelTab, type Tabbed } from "./ActivityBar";
import {
  CATALOGUE,
  drawnNow,
  HALVES,
  halvesSplit,
  inSlots,
  leastOf,
  movesTo,
  openView,
  placeOf,
  shownIn,
  SIDES,
  sizeForgotten,
  slotLength,
  SLOT_IDS,
  SLOTS,
  SPLIT,
  startingLength,
  tabsOf,
  VIEWS,
  type Arrangement,
  type Placement,
  type RegionId,
  type Sash,
  type Side,
  type SlotId,
  type ViewId,
} from "./regions";
import { isPanelView, NO_PREVIEWS, type ViewPlace } from "./sideViews";
import { useArrived } from "./lib/arrived";
import { HalfActions } from "./halfHead";
import { lastUsed, narrowed, narrowPicked, useNarrow } from "./narrowPanel";
import { useSidesOfTheFront } from "./railRoom";
import { cycleOf, f6Of, stepped, STOPS, type Stop } from "./regionCycle";
import { moveAlong, sequenceIn } from "./tabSequence";
import { inADialog } from "./paneKeyboard";
import { CHAT_KEYBOARD } from "./actions";
import { forgetFrameScroll, useViewScroll } from "./viewScroll";
import { IconButton } from "./ui/button/Button";
import { PanelSectionTitle, panelRoot } from "./ui/panel/Panel";
import { ActivityBarFrame } from "./ui/activity/ActivityItem";

/**
 * **The window, drawn from the arrangement** (ADR 0038, and `regions.ts` for why the
 * arrangement is data).
 *
 * Four slots and nothing else: a slot on the left, the centre, a slot on the right, and the
 * **Panel** under all three, full width (#1737, P-1), its halves side by side in it. (A slot
 * along the bottom held the State region until #1676 made it the left's Changes view; the
 * Panel is the bottom slot back, under both sidebars this time.) The panel list is the same on
 * every render of a window's life, and that is the load-bearing part — see `regions.ts` for
 * charter-app#141's throw. What the *data* decides
 * is which slot a region's content goes in, what order it is in, whether it is drawn at all,
 * and how big its slot starts.
 *
 * So moving a region is moving its content between two panels that both already exist. Nothing
 * is added to a live group, nothing is taken out of one, no constraint changes, and the centre
 * — the terminal panes, which are the product — is never remounted by a layout change.
 */
export function RegionFrame({
  arrangement: kept,
  content,
  views,
  panels = [],
  previews = NO_PREVIEWS,
  badges,
  keys,
  onPick,
  centre,
  onResized,
  onReset,
  onSplit,
  menu,
  heads,
  keyboard = true,
}: {
  arrangement: Arrangement;
  /** What each region draws. Kept out of the arrangement because a React component is not
   *  something a stored document can hold. A region with views draws {@link views} instead,
   *  when they are given. */
  content: Partial<Record<RegionId, ReactNode>>;
  /**
   * **What each view draws** (#1673). A region whose catalogue entry has views, given them
   * here, is drawn as an activity bar at its side's edge and its views in its slot: the open
   * one shown, the others mounted and hidden, and all of them mounted and hidden while the
   * region is put away — collapsing hides and never unmounts, so a view keeps its scroll, its
   * folds and its filter. The right-hand side's views (#1678) arrive the same way.
   */
  views?: Partial<Record<ViewId, ReactNode>>;
  /**
   * **The approved extensions' panels, each a view** (#1678), in the order they go on the bar,
   * after purlis's own views of the region that takes them, with the name and mark each
   * declared. What each draws is in {@link views} under its id.
   */
  panels?: readonly PanelTab[];
  /** **purlis's own views behind a preview that is on now** (#3266, `sideViews.PREVIEWED_VIEWS`):
   *  one left out is drawn nowhere, and its region opens on its first view meanwhile. */
  previews?: readonly ViewId[];
  /** What each view's tab carries beside its icon: a count, drawn while the side is away too. */
  badges?: Partial<Record<ViewId, ReactNode>>;
  /** How each view's key is spelled, for its tab's tooltip. */
  keys?: Partial<Record<ViewId, string>>;
  /** A view's tab was pressed: the window answers with the arrangement (`regions.picked`). */
  onPick?: (view: ViewId) => void;
  centre: ReactNode;
  /** Told how big each slot was left, once a drag has settled. */
  onResized: (sizes: Partial<Record<SlotId, number>>) => void;
  /** A handle was double-clicked: its slot goes back to its default size, or the Panel's halves
   *  back to half each (P-12). */
  onReset?: (sash: Sash) => void;
  /** Told the left half's share of the Panel's width, once a drag of the handle between the
   *  halves has settled (#1738). */
  onSplit?: (share: number) => void;
  /** **A view's icon with its context menu around it** (#1740): told the view and where it can
   *  move — its side's other place, or nothing for the Inbox, which is pinned. */
  menu?: (view: ViewId, to: ViewPlace | undefined, tab: ReactElement) => ReactNode;
  /** **What the side panel's header says for each view** (R123, #2917), in the label's
   *  capitals: the view's name where nothing is given; `null` for a view that draws its own
   *  heading (Chats, which names the workspace and the project on its title's line). */
  heads?: Partial<Record<ViewId, ReactNode | null>>;
  /** Whether this window's keys are this frame's: F6 (P-13) is held by the project in front. */
  keyboard?: boolean;
}) {
  // **The window's keys are this frame's only while its project is in front**, read here and not
  // taken from `keyboard` alone: a project kept behind another is not drawn again
  // (`projectShown.tsx`), so the `keyboard` it was last drawn with still says it is in front.
  const holdsKeys = useProjectShown() && keyboard;
  // **A narrow window draws one half of the Panel at a time** (P-7, `narrowPanel.ts`): the one
  // used last. What is drawn below is that; what is stored is `kept`, as the person left it.
  const narrow = useNarrow();
  const [last, setLast] = useState<Side>("left");
  const [before, setBefore] = useState(kept);
  if (before !== kept) {
    // A half opened by anything — its Lower icon, a view's key, a palette row — is the one used
    // last. Derived while rendering, as React has a value that follows a prop.
    setBefore(kept);
    setLast(lastUsed(before, kept, last));
  }
  const present = [...panels.map((one) => one.view), ...previews];
  // Without the moved panels no extension contributes now (`regions.drawnNow`): a half left
  // with none of its views is not drawn, and F6 skips it.
  const now = drawnNow(kept, present);
  const arrangement = narrow ? narrowed(now, last) : now;
  const slots = inSlots(arrangement);
  // The sidebars drawn, which the project rail makes room for (`railRoom.ts`, #3077).
  useSidesOfTheFront(SIDES.filter((side) => shownIn(slots[side]).length > 0).length);
  const base = useId();
  const ids = {
    panelOf: (view: ViewId) => `${base}view-${view}`,
    tabOf: (view: ViewId) => `${base}tab-${view}`,
  };
  const withViews = (one: Placement) =>
    views !== undefined && CATALOGUE[one.id].views !== undefined;
  /** A view's icon pressed, or a half's chevron: in a narrow window, a half not drawn is drawn
   *  first, and its open view's icon puts nothing away (`narrowPicked`). */
  const pick = (view: ViewId) => {
    if (narrow) {
      const said = narrowPicked(kept, view, last, present);
      setLast(said.last);
      if (!said.forward) return;
    }
    onPick?.(view);
  };
  /** A region's views, as its bar draws them now (#1740): purlis's own and the panels, each
   *  where the arrangement has it. */
  const tabbed = (id: RegionId): Tabbed[] =>
    tabsOf(arrangement, id, present).flatMap((view): Tabbed[] => {
      if (!isPanelView(view)) return [{ view, name: VIEWS[view].name, mark: VIEW_MARKS[view] }];
      const panel = panels.find((one) => one.view === view);
      return panel === undefined ? [] : [panel];
    });
  /** Each icon's menu: the move its view has, where the arrangement says it can go. */
  const menuOf =
    menu === undefined
      ? undefined
      : (view: ViewId, tab: ReactElement) => {
          const to = movesTo(arrangement, view);
          return menu(view, to === undefined ? undefined : placeOf(to), tab);
        };
  /** A region's bar: a column of its icons, at the frame's start on the left and its end on the
   *  right (#3144). */
  const barOf = (one: Placement, side: Side, away?: string) => (
    <ActivityBar
      key={one.id}
      side={side}
      variant="bar"
      region={one.id}
      name={CATALOGUE[one.id].name}
      away={CATALOGUE[one.id].stays === true ? null : away}
      views={tabbed(one.id)}
      open={one.collapsed ? undefined : openView(one, present)}
      keys={keys}
      badges={badges}
      {...ids}
      onPick={pick}
      menu={menuOf}
    />
  );
  /** A side's own bar and its Lower icons (the bars of its halves of the Panel): the top group,
   *  then the bottom group. A half with no views has no icons. */
  const barsOf = (side: Side) => ({
    // A sidebar every view has moved out of draws no icons (#1740), as an empty half does not.
    upper: slots[side]
      .filter((one) => withViews(one) && tabbed(one.id).length > 0)
      .map((one) => barOf(one, side)),
    lower: slots.panel
      .filter((one) => one.side === side && withViews(one) && tabbed(one.id).length > 0)
      .map((one) => barOf(one, side, `Put the panel's ${side} half away`)),
  });
  /**
   * **A side's bar** (R117, #2917; #3144): a 44px column the whole height beside its side panel
   * and its half of the Panel, its own icons at the top and the Lower icons at its foot. The
   * right one is at the window's edge; the left one is the frame's first column, beside the
   * project rail, as the switcher prototype's variant D draws it with "Bar: column". It is
   * drawn whether or not its side is out, so putting the side away or bringing it back moves
   * no icon.
   */
  const column = (side: Side) => {
    const { upper, lower } = barsOf(side);
    if (upper.length === 0 && lower.length === 0) return null;
    return (
      <ActivityBarFrame
        variant="bar"
        className="activity-column"
        data-side={side}
        top={upper}
        bottom={lower}
      />
    );
  };
  /**
   * **What a side panel draws above its views** (R123, R125): on the left, the open view's
   * header in the label's capitals (its icons are the left column's, #3144); on the right, the
   * open view's title and the × that closes it. Nothing while the side is away.
   */
  const sideTop = (side: Side): ReactNode => {
    const first = shownIn(slots[side]).find(withViews);
    if (first === undefined) return null;
    const open = openView(first, present);
    const named = tabbed(first.id).find((one) => one.view === open)?.name;
    if (side === "left") {
      const head = open === undefined ? undefined : heads?.[open];
      if (open === undefined || head === null) return null;
      return (
        <div className="side-head" data-side={side}>
          <PanelSectionTitle>{head ?? named}</PanelSectionTitle>
        </div>
      );
    }
    if (open === undefined || named === undefined) return null;
    return (
      <div className="side-head" data-side={side}>
        <h2 className="side-title">{named}</h2>
        <IconButton
          size="sm"
          aria-label={`Close ${named}`}
          tooltip={`Close ${named}`}
          onPress={() => {
            // The keyboard back on the icon that opened it (WCAG 2.4.3): the head goes with it.
            document.getElementById(ids.tabOf(open))?.focus();
            pick(open);
          }}
          icon={<X className="node-icon" aria-hidden="true" />}
          className="side-close"
        />
      </div>
    );
  };
  // **Each view's own element, made once for the frame's life** (#1752, D-1752-4): the tab
  // panel its content is drawn into through a portal from here, so a view moved between its
  // sidebar and its half of the Panel is that element put somewhere else, and nothing in it is
  // mounted again. What the person typed, folded or picked in it stays, as the scroll does.
  // One function for the frame's life, as `base` is: the regions' effects that place the
  // elements run again only when what they hold changes.
  const [hostOf] = useState(() => {
    const hosts = new Map<ViewId, HTMLDivElement>();
    return (view: ViewId): HTMLDivElement => {
      let host = hosts.get(view);
      if (host === undefined) {
        host = document.createElement("div");
        host.setAttribute("role", "tabpanel");
        host.id = ids.panelOf(view);
        host.setAttribute("aria-labelledby", ids.tabOf(view));
        host.dataset.view = view;
        host.className = "region-view";
        // Not a Tab stop: where F6 lands in a view with nothing to press (P-13).
        host.tabIndex = -1;
        host.hidden = true;
        hosts.set(view, host);
      }
      return host;
    };
  });
  // Each view's own place on its half's line (P-11), held here, where its content is drawn:
  // state, so a view drawn before its line was mounted is drawn again with somewhere to put its
  // actions.
  const [actions, setActions] = useState<Partial<Record<ViewId, HTMLElement | null>>>({});
  // One callback per view for the frame's life: a new one each render would be called again on
  // every render, and each call is a render.
  const [actionRef] = useState(() => {
    const made = new Map<ViewId, (element: HTMLElement | null) => void>();
    return (view: ViewId) => {
      let ref = made.get(view);
      if (ref === undefined) {
        ref = (element) =>
          setActions((was) => (was[view] === element ? was : { ...was, [view]: element }));
        made.set(view, ref);
      }
      return ref;
    };
  });
  // **Where the keyboard was in a view being moved** (#1752): an element taken out of the
  // document loses the keyboard, so a region moving a view's element notes it here first, and
  // the region it lands in gives it back. Let go once every region has placed its views.
  const [focusMoving] = useState((): FocusMoving => {
    let was: HTMLElement | null = null;
    return {
      noted: (element) => {
        const at = document.activeElement;
        if (at instanceof HTMLElement && at !== document.body && element.contains(at)) was = at;
      },
      landed: (host) => {
        if (was === null || !host.contains(was)) return;
        const it = was;
        was = null;
        if (!host.hidden && document.activeElement !== it) it.focus({ preventScroll: true });
      },
      forget: () => {
        was = null;
      },
    };
  });
  useLayoutEffect(() => focusMoving.forget());
  const drawn: SlotContent = {
    content,
    views,
    ids,
    withViews,
    tabbed,
    present,
    onPick: pick,
    frame: base,
    hostOf,
    actionRef,
    focusMoving,
    sideTop,
  };
  /** Every view drawn now, with the region it is in: each one's content, drawn once. */
  const placedViews =
    views === undefined
      ? []
      : arrangement
          .filter(withViews)
          .flatMap((one) => tabbed(one.id).map(({ view }) => ({ view, region: one.id })));
  // What this frame's views kept goes with it (`viewScroll.ts`).
  useEffect(() => () => forgetFrameScroll(base), [base]);
  /** The Panel's slot, held here so a double-click of its handle can size it (P-12). */
  const panelSlot = usePanelRef();
  const resetPanel = () => {
    panelSlot.current?.resize(
      slotLength("panel", inSlots(sizeForgotten(arrangement, "panel")).panel),
    );
    onReset?.("panel");
  };

  // **How big each slot starts, read once.** `defaultSize` is a constraint, and a constraint
  // that changes re-registers the panel — charter-app#141 again. The arrangement the window
  // launched with is what sizes the slots; a size remembered mid-session is remembered for the
  // next launch and does not move the slot under the operator's hands.
  const [started] = useState(() => inSlots(arrangement));

  // **F6 and Shift+F6** (P-13, `regionCycle.ts`): the keyboard to the next region drawn, or the
  // one before. Claimed on the window, capture-phase, by the project in front, as the side keys
  // are; never under a dialog, and never from a chat's terminal, whose F6 it is.
  const frame = useRef<HTMLDivElement>(null);
  const order = cycleOf(arrangement).join(" ");
  useEffect(() => {
    if (!holdsKeys) return;
    const cycle = order.split(" ") as Stop[];
    const key = (e: KeyboardEvent) => {
      const which = f6Of(e);
      if (which === undefined || inADialog()) return;
      const on = e.target;
      if (on instanceof Element && on.closest(`[${CHAT_KEYBOARD}]`) !== null) return;
      const root = frame.current;
      if (root === null) return;
      e.preventDefault();
      e.stopPropagation();
      stepIn(root, cycle, which.back);
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [holdsKeys, order]);

  // **The right bar straight after the right sidebar in Tab order** (#1686, D-1686-2): Tab from
  // the right view goes to its bar, then to the Panel. The bar stays outside the groups, at the
  // window's edge (charter-app#141), so the frame walks Tab itself at the two places the DOM's
  // order differs, and leaves every other press to the engine.
  useEffect(() => {
    const root = frame.current;
    if (root === null) return;
    const key = (e: globalThis.KeyboardEvent) => {
      if (e.key !== "Tab" || e.ctrlKey || e.metaKey || e.altKey || inADialog()) return;
      const on = document.activeElement;
      if (!(on instanceof HTMLElement) || on.closest(`[${CHAT_KEYBOARD}]`) !== null) return;
      const stops = drawnStops(root);
      const order = tabOrderOf(root, stops);
      const at = order.indexOf(on);
      if (at < 0) return;
      const step = e.shiftKey ? -1 : 1;
      const want = order[at + step] as HTMLElement | undefined;
      const natural = stops[stops.indexOf(on) + step] as HTMLElement | undefined;
      if (want === natural) return;
      e.preventDefault();
      if (want !== undefined) want.focus();
      else moveAlong(root, e.shiftKey);
    };
    root.addEventListener("keydown", key);
    return () => root.removeEventListener("keydown", key);
  }, []);

  const settled = (layout: Layout, meta: { isUserInteraction: boolean }) => {
    // `onLayoutChanged` also fires on mount, on a collapse and on any other imperative call.
    // Only a drag or a resize key is the operator saying how big they want it; everything else
    // would write back a measurement charter itself caused — including the zero a collapsed
    // slot reports, which is not a width to come back to.
    if (!meta.isUserInteraction) return;
    // One answer for both groups: the row of the sidebars and the editor reports the widths,
    // the column of that row and the Panel reports the Panel's height, each under its own id.
    const sizes: Partial<Record<SlotId, number>> = {};
    for (const slot of SLOT_IDS) {
      const measured = layout[panelOf(slot)];
      if (typeof measured === "number") sizes[slot] = measured;
    }
    if (Object.keys(sizes).length > 0) onResized(sizes);
  };

  // **The bars are outside the groups**, at the window's edges as VS Code has them, so a side
  // put away leaves its bar — and the bar's badges — on screen. They are not panels: nothing
  // here adds to or takes from a group's panel list (charter-app#141).
  //
  // **The Panel is a column's second panel** (#1737): the sidebars and the editor are one panel
  // above it, so it runs under all three, and an activity bar the whole height beside both.
  return (
    <div className="region-frame" ref={frame} data-cycle={order}>
      {/* **The left bar** (#3144): the frame's first column, beside the project rail, drawn
          whether or not its side panel is out, so a press brings the side back on that view and
          nothing moves when it comes or goes. */}
      {column("left")}
      <Group className="regions" orientation="vertical" onLayoutChanged={settled}>
        <Panel id="region-upper" className="region-upper" minSize="20%">
          <Group className="region-row" orientation="horizontal" onLayoutChanged={settled}>
            <Slot side="left" placed={slots.left} started={started.left} drawn={drawn} />
            <Edge open={shownIn(slots.left).length > 0} named="Resize the left sidebar" />
            {/* A named region to a screen reader, as each sidebar and half is (#1751): F6 lands
                in it as "Editor". */}
            <Panel
              id="region-centre"
              className="region-centre"
              minSize="20%"
              role="region"
              aria-label={EDITOR}
              // The content panel is drawn under the session tabs docked on it (`ui/tabs`,
              // #2914): the region holds the strip and the panel, and draws no ground itself.
            >
              {centre}
            </Panel>
            <Edge open={shownIn(slots.right).length > 0} named="Resize the right sidebar" />
            <Slot side="right" placed={slots.right} started={started.right} drawn={drawn} />
          </Group>
        </Panel>
        <Edge
          open={shownIn(slots.panel).length > 0}
          named="Resize the Panel"
          onReset={resetPanel}
        />
        <Slot
          side="panel"
          placed={slots.panel}
          started={started.panel}
          drawn={drawn}
          handle={panelSlot}
        >
          <Halves
            placed={slots.panel}
            started={started.panel}
            drawn={drawn}
            onSplit={onSplit}
            onReset={() => onReset?.(HALVES)}
          />
        </Slot>
      </Group>
      {column("right")}
      {/* Each view's content, drawn once wherever its region puts it (#1752): after the
          regions, so each tab panel is in its place before what is drawn into it. */}
      {placedViews.map(({ view, region }) => (
        <ViewHost
          key={view}
          view={view}
          region={region}
          host={hostOf(view)}
          frame={base}
          actions={CATALOGUE[region].half === true ? (actions[view] ?? null) : undefined}
        >
          {views?.[view]}
        </ViewHost>
      ))}
    </div>
  );
}

/** What a slot draws its regions from. */
type SlotContent = {
  content: Partial<Record<RegionId, ReactNode>>;
  views?: Partial<Record<ViewId, ReactNode>>;
  ids: { panelOf: (view: ViewId) => string; tabOf: (view: ViewId) => string };
  withViews: (one: Placement) => boolean;
  tabbed: (id: RegionId) => Tabbed[];
  /** The extensions' panels there are now, which decide whether one picked last can open. */
  present: readonly ViewId[];
  /** A view's tab, or a half's chevron, was pressed. */
  onPick?: (view: ViewId) => void;
  /** This frame, for what its views keep across a move (`viewScroll.ts`). */
  frame: string;
  /** Each view's own tab panel, which its content is drawn into wherever the view is (#1752). */
  hostOf: (view: ViewId) => HTMLDivElement;
  /** Where a view in a half of the Panel puts its actions on the half's line. */
  actionRef: (view: ViewId) => (element: HTMLElement | null) => void;
  /** Where the keyboard was in a view whose element is being moved, for the region it lands in
   *  to give back. */
  focusMoving: FocusMoving;
  /** What a side panel draws above its views: its tabs and header, or its title and ×. */
  sideTop: (side: Side) => ReactNode;
};

/** The keyboard kept across a view's move (#1752): see `RegionFrame`'s `focusMoving`. */
type FocusMoving = {
  /** Notes where the keyboard is, where it is in `element`, before `element` is moved. */
  noted: (element: Element) => void;
  /** Gives the keyboard back in `host`, where it was there and `host` is shown. */
  landed: (host: HTMLElement) => void;
  /** Lets go of what was noted: every region has placed its views. */
  forget: () => void;
};

/**
 * A region with views, in its slot: each view a tab panel, the open one shown and the rest
 * mounted and `hidden` — out of the tab order and the accessibility tree, and still holding
 * what the person did in it. Put away, every one of them is hidden.
 *
 * **The tab panels are the views' own elements** ({@link SlotContent.hostOf}), put here in the
 * bar's order: their content is drawn once, by the frame ({@link ViewHost}), so a view that
 * moves here from another region arrives as it was (#1752).
 */
function RegionViews({
  placed,
  drawn,
  swaps = false,
}: {
  placed: Placement;
  drawn: SlotContent;
  /** A view picked in place of another fades its list in (R122): the side panel's. */
  swaps?: boolean;
}) {
  const open = placed.collapsed ? undefined : openView(placed, drawn.present);
  const box = useRef<HTMLDivElement>(null);
  // The view shown last while the region was out: a change from one to another is a swap.
  const shownBefore = useRef(open);
  const order = drawn
    .tabbed(placed.id)
    .map(({ view }) => view)
    .join("\n");
  const { hostOf, focusMoving } = drawn;
  // Before the frame's portals draw into them (they come later in the tree), so a view's own
  // effects find it in the document.
  useLayoutEffect(() => {
    const root = box.current;
    if (root === null) return;
    const views = order === "" ? [] : (order.split("\n") as ViewId[]);
    const hosts = views.map(hostOf);
    hosts.forEach((host, at) => {
      host.hidden = views[at] !== open;
      // A list hidden or moved has finished arriving: shown again, it does not replay (R151).
      if (host.hidden) swapOut(host);
      // Moved only where it is not in its place already: an element taken out of the document
      // and put back loses its scroll.
      if (root.children[at] !== host) {
        swapOut(host);
        focusMoving.noted(host);
        root.insertBefore(host, root.children[at] ?? null);
      }
      // The keyboard back where the view landed, open: never into a hidden view.
      focusMoving.landed(host);
    });
    // A view that left this region and went nowhere else is drawn nowhere.
    for (const child of [...root.children])
      if (!hosts.includes(child as HTMLDivElement)) {
        focusMoving.noted(child);
        root.removeChild(child);
      }
    const before = shownBefore.current;
    shownBefore.current = open;
    // **Only the list that just took another's place moves** (R122, R151): never on a mount, a
    // re-render, the side coming back or a view moved here.
    if (swaps && before !== undefined && open !== undefined && before !== open)
      swapIn(hostOf(open));
  }, [order, open, hostOf, focusMoving, swaps]);
  return (
    <div className="region-views" data-region={placed.id} hidden={placed.collapsed} ref={box} />
  );
}

/**
 * **One view's content, drawn once by the frame into the view's own tab panel** (#1752,
 * D-1752-4), wherever the arrangement puts it: a move changes which region holds the element,
 * never where the content is in React's tree, so nothing in it is mounted again. In a half of
 * the Panel it is told where its actions go on the half's line ({@link HalfActions}); in a
 * sidebar it is told nothing, and draws them on its own heading. Where it scrolled is put back
 * after a move, since an element taken out of the document loses its scroll.
 */
function ViewHost({
  view,
  region,
  host,
  frame,
  actions,
  children,
}: {
  view: ViewId;
  region: RegionId;
  host: HTMLDivElement;
  frame: string;
  /** Where its actions go: a half's line, `null` before the line is mounted; `undefined` in a
   *  sidebar. Always a provider, so a move never changes what its content is drawn under. */
  actions: HTMLElement | null | undefined;
  children: ReactNode;
}) {
  // A view's element is its own for the frame's life (`hostOf`), and this is keyed by the view.
  const element = useRef<HTMLElement | null>(host);
  useViewScroll(frame, view, region, element);
  return createPortal(
    <HalfActions.Provider value={actions}>{children}</HalfActions.Provider>,
    host,
  );
}

/**
 * **The Panel's halves, side by side, and the handle between them** (#1738, P-5, P-12).
 *
 * A group of its own inside the Panel's slot, with the same rule as the window's: both halves'
 * panels are always there, with constraints that never change, and a half that is put away is
 * collapsed by the library, so the other takes the whole width. Half each until the handle is
 * dragged, a fifth at least ({@link SPLIT}); a double-click puts it back at half each, charter's
 * reset ({@link useHandleReset}) and not the library's, which would go back to the split the window launched with.
 */
function Halves({
  placed,
  started,
  drawn,
  onSplit,
  onReset,
}: {
  placed: Placement[];
  started: Placement[];
  drawn: SlotContent;
  onSplit?: (share: number) => void;
  onReset: () => void;
}) {
  const left = usePanelRef();
  const right = usePanelRef();
  // Read once, as the slots' sizes are: a constraint that changes re-registers the panel.
  const [first] = useState(() => halvesSplit(started));
  const split = halvesSplit(placed);
  const both = split.left > 0 && split.right > 0;
  const settled = (layout: Layout, meta: { isUserInteraction: boolean }) => {
    if (!meta.isUserInteraction) return;
    const share = layout[halfOf("left")];
    if (typeof share === "number") onSplit?.(share);
  };
  const reset = () => {
    left.current?.resize(`${SPLIT.size}%`);
    onReset();
  };
  const between = useHandleReset(reset);
  const half = (side: Side, handle: ReturnType<typeof usePanelRef>) => (
    <Half
      side={side}
      placed={placed.find((one) => one.side === side)}
      starts={first[side]}
      wanted={split[side]}
      drawn={drawn}
      handle={handle}
    />
  );
  return (
    <Group className="panel-halves" orientation="horizontal" onLayoutChanged={settled}>
      {half("left", left)}
      <Separator
        className={both ? undefined : "edge-gone"}
        disableDoubleClick
        elementRef={between}
        aria-label="Resize the Panel's halves"
      />
      {half("right", right)}
    </Group>
  );
}

/** The panel each half is drawn in, and the id its share is reported under. */
const halfOf = (side: Side): string => `panel-half-${side}`;

/**
 * One half of the Panel: its line, then its views (P-11). The line is the open view's title,
 * that view's own actions (each view hands them over through {@link HalfActions}) and a chevron
 * that puts the half away, as a press of its open Lower icon does. No tabs: the Lower icons are
 * the one switch.
 */
function Half({
  side,
  placed,
  starts,
  wanted,
  drawn,
  handle,
}: {
  side: Side;
  placed: Placement | undefined;
  /** Its share of the width on the window's first frame. */
  starts: number;
  /** Its share now: what it comes back at. */
  wanted: number;
  drawn: SlotContent;
  handle: ReturnType<typeof usePanelRef>;
}) {
  const shown = placed !== undefined && shownIn([placed]).length > 0;
  const before = useRef(shown);
  useEffect(() => {
    const it = handle.current;
    const changed = before.current !== shown;
    before.current = shown;
    if (it === null) return;
    if (!shown) {
      it.collapse();
      return;
    }
    if (!changed) return;
    it.expand();
    it.resize(`${wanted}%`);
  }, [handle, shown, wanted]);
  // **Only a half that just opened arrives** (R127): never at launch, never the other half, and
  // never again once it has arrived (a project brought back in front, S7).
  const [arrived, arrivedEnd] = useArrival(shown);
  const halfBox = useRef<HTMLDivElement>(null);
  // Heard as the browser says it, as the slot's is: the event itself, not React's name for it.
  useEffect(() => {
    const element = halfBox.current;
    if (element === null) return;
    element.addEventListener("animationend", arrivedEnd);
    return () => element.removeEventListener("animationend", arrivedEnd);
  }, [arrivedEnd]);
  const open = placed === undefined || !shown ? undefined : openView(placed, drawn.present);
  const views = placed === undefined ? [] : drawn.tabbed(placed.id);
  const title = views.find((one) => one.view === open)?.name;
  return (
    <Panel
      id={halfOf(side)}
      panelRef={handle}
      collapsible
      collapsedSize="0%"
      defaultSize={`${starts}%`}
      minSize={`${SPLIT.least}%`}
      maxSize="100%"
    >
      <div
        className={arrived ? "panel-half arrived" : "panel-half"}
        ref={halfBox}
        // A half of the bottom panel is a panel of its own (`ui/panel`).
        {...panelRoot("bottom")}
        data-side={side}
        // A named region to a screen reader (P-13): its side's half, whichever view is open.
        role="region"
        aria-label={placed === undefined ? undefined : CATALOGUE[placed.id].name}
        hidden={!shown}
      >
        {placed !== undefined && (
          <>
            <div className="half-head">
              <PanelSectionTitle>{title}</PanelSectionTitle>
              {views.map(({ view }) => (
                <span
                  key={view}
                  className="half-actions"
                  hidden={view !== open}
                  ref={drawn.actionRef(view)}
                />
              ))}
              <IconButton
                size="sm"
                excludeFromTabOrder
                aria-label={`Put the panel's ${side} half away`}
                tooltip={`Put the panel's ${side} half away`}
                onPress={() => {
                  if (open === undefined) return;
                  // The keyboard back on the icon that opened it (WCAG 2.4.3).
                  document.getElementById(drawn.ids.tabOf(open))?.focus();
                  drawn.onPick?.(open);
                }}
                icon={<X className="node-icon" aria-hidden="true" />}
                className="half-away"
              />
            </div>
            <RegionViews placed={placed} drawn={drawn} />
          </>
        )}
      </div>
    </Panel>
  );
}

/**
 * **Whether a part is arriving now**, and what ends it: true from the moment it opens while the
 * window is up until its animation ends, so taking the element out of the document and putting
 * it back (a project brought in front again) never replays it (R151).
 */
function useArrival(
  open: boolean,
): readonly [boolean, (e: { target: unknown; currentTarget: unknown }) => void] {
  const moved = useArrived(open) && open;
  const [ended, setEnded] = useState(false);
  const [was, setWas] = useState(open);
  if (was !== open) {
    setWas(open);
    setEnded(false);
  }
  const [end] = useState(() => (e: { target: unknown; currentTarget: unknown }) => {
    if (e.target === e.currentTarget) setEnded(true);
  });
  return [moved && !ended, end] as const;
}

/** The attribute a side view's list carries while it fades in (R122), which `App.css` draws. */
const SWAPPED = "data-swapped";

/**
 * **A side view's list fading in from 4px below** (R122), over `duration.swap`: the stylesheet's
 * animation, on the element, until it ends. The mark is taken off a view that is hidden or moved,
 * so nothing replays when it is shown again; under reduced motion the duration is 0ms.
 */
function swapIn(host: HTMLElement) {
  swapOut(host);
  host.setAttribute(SWAPPED, "");
  const ended = (e: AnimationEvent) => {
    if (e.target === host) swapOut(host);
  };
  SWAP_ENDS.set(host, ended);
  host.addEventListener("animationend", ended);
}

/** What hears each fading list's animation end, so whatever ends the fade takes it off too. */
const SWAP_ENDS = new WeakMap<HTMLElement, (e: AnimationEvent) => void>();

/** A list's fade over: its mark off, and nothing left listening for its end. */
function swapOut(host: HTMLElement) {
  host.removeAttribute(SWAPPED);
  const ended = SWAP_ENDS.get(host);
  if (ended === undefined) return;
  host.removeEventListener("animationend", ended);
  SWAP_ENDS.delete(host);
}

/** What the editor region is called: its region's name, for F6 and a screen reader (#1751). */
export const EDITOR = "Editor";

/**
 * **A region's own chrome, which F6 passes over** (DS-6, #1836): the attribute on the chat strip
 * the editor draws above its panes. F6 into the editor lands on the pane, as VS Code's lands on
 * the editor and not on its tabs; Tab still stops on the strip, in document order.
 */
export const REGION_CHROME = "data-region-chrome";

/** Where each region F6 stops in is drawn: a sidebar's open view, the editor, a half. */
const REGION_OF: Record<Stop, string> = {
  left: ".slot-left .region-view:not([hidden])",
  centre: ".region-centre",
  right: ".slot-right .region-view:not([hidden])",
  "panel-left": '.panel-half[data-side="left"]:not([hidden])',
  "panel-right": '.panel-half[data-side="right"]:not([hidden])',
};

/** The keyboard to the region after the one holding it in `cycle`, or with `back` before. */
function stepIn(root: HTMLElement, cycle: readonly Stop[], back: boolean) {
  landIn(root, stepped(cycle, stopHolding(root, document.activeElement), back));
}

/**
 * **What F6 and Shift+F6 do, run from the palette** (#1751): the keyboard to the next region of
 * the frame on screen, or with `back` the one before. One project draws its frame at a time, so
 * the frame drawn is the one in front; with none, nothing happens.
 */
export function stepRegions(back: boolean): void {
  const root = document.querySelector<HTMLElement>(".region-frame[data-cycle]");
  if (root === null) return;
  const cycle = (root.dataset.cycle ?? "").split(" ").filter(Boolean) as Stop[];
  if (cycle.length > 0) stepIn(root, cycle, back);
}

/**
 * **The order Tab walks the frame in** (#1686, D-1686-2): the DOM's, with the right column's
 * stops — its bar and its Lower icons — moved to straight after the right sidebar, before the
 * Panel's first stop. Where the Panel has none, they stay last, as the DOM has them.
 */
export function tabOrderOf(root: HTMLElement, stops: readonly HTMLElement[]): HTMLElement[] {
  const column = root.querySelector(".activity-column[data-side='right']");
  if (column === null) return [...stops];
  const upper = root.querySelector(".region-upper");
  const regions = root.querySelector(".regions");
  const inPanel = (el: HTMLElement) =>
    regions !== null && regions.contains(el) && !(upper?.contains(el) ?? false);
  const bar = stops.filter((el) => column.contains(el));
  const rest = stops.filter((el) => !column.contains(el));
  const at = rest.findIndex(inPanel);
  return at < 0 ? [...rest, ...bar] : [...rest.slice(0, at), ...bar, ...rest.slice(at)];
}

/** What WebKit would stop at in `scope`, of what is drawn. */
const drawnStops = (scope: HTMLElement): HTMLElement[] =>
  sequenceIn(scope).filter((el) => el.closest("[hidden]") === null);

/** The region F6 stops in that holds `at`, if any does. */
function stopHolding(root: HTMLElement, at: Element | null): Stop | undefined {
  if (at === null) return undefined;
  return STOPS.find((stop) =>
    [...root.querySelectorAll(REGION_OF[stop])].some((region) => region.contains(at)),
  );
}

/**
 * **The keyboard into a region**: its open view's Tab stop — a tree's current row, as Tab would
 * come in — or the view itself when it has nothing to press. In the editor, the focused pane's
 * own controls and not its terminal, where F6 is the chat's (`regionCycle.ts`).
 */
function landIn(root: HTMLElement, stop: Stop) {
  const region = root.querySelector<HTMLElement>(REGION_OF[stop]);
  if (region === null) return;
  if (stop === "centre") {
    const pane = region.querySelector(".pane.focused")?.closest<HTMLElement>(".pane-frame");
    const outside = (el: HTMLElement) => el.closest(`[${CHAT_KEYBOARD}]`) === null;
    // Past the strip (`REGION_CHROME`): with no pane in focus, or none with a control of its
    // own, the editor's first stop below its tab bar — an empty state's button — and only
    // then a terminal or the strip.
    const below = drawnStops(region).filter((el) => el.closest(`[${REGION_CHROME}]`) === null);
    const landing =
      (pane ? drawnStops(pane) : below).find(outside) ?? below[0] ?? drawnStops(region)[0];
    landing?.focus();
    return;
  }
  const view =
    stop === "left" || stop === "right"
      ? region
      : region.querySelector<HTMLElement>(".region-view:not([hidden])");
  if (view === null) return;
  (drawnStops(view)[0] ?? view).focus();
}

/** The panel each slot is drawn in. Also the id `react-resizable-panels` reports a size under,
 *  so the two have to be the same string and there is one place it is written. */
export const panelOf = (slot: SlotId): string => `region-${slot}`;

/**
 * One of the two slots beside the centre: resizable, and emptied without going away.
 *
 * **The panel stays mounted with constraints that never change; the library collapses it.**
 * That is the whole reason this component exists, and neither obvious alternative works.
 * Rendering the `Panel` and its `Separator` conditionally throws — `react-resizable-panels`
 * recalculates a separator's aria values against the group's constraint list, and taking a
 * panel out from under a live separator leaves it indexing past the end
 * (*"Panel constraints not found for index 3"*), from a document listener where no `try` of
 * ours can reach it. Clamping the panel's `minSize`/`maxSize` to zero instead throws the same
 * way, because changing a constraint re-registers the panel and a recalculation lands in the
 * gap. So the constraints are written once and `collapse()`/`expand()` — the library's own
 * way of doing this — is what moves it.
 *
 * **Its content is unmounted all the same**, which is the part that has to be true for more
 * than tidiness: a slot squeezed to zero pixels with its markup still in the document is still
 * in the tab order and still read out in full by a screen reader, so "put away" would mean
 * "invisible and in the way". Nothing in a region is state worth keeping across that — what
 * they draw is read fresh from the plane every time a workspace is focused, because the plane
 * is a directory the operator also edits by hand.
 *
 * **The flash is fixed by the first layout and not by an earlier effect, and that is measured.**
 * charter-app#141 left a window that launched with a region put away drawing it full size for
 * one frame: the panel was sized normally and a `useEffect` collapsed it, and a passive effect
 * runs after the browser has painted. The obvious repair is `useLayoutEffect`, and it **throws**
 * — *"Group &lt;id&gt; not found"*, from `getPanelConstraints`, because the group registers
 * itself in its *own* layout effect and React runs a child's layout effects before its parent's.
 * There is nowhere inside the group that runs later.
 *
 * So the first layout is made right instead: `startingSize` gives a slot that starts with
 * nothing in it `0%`, which `react-resizable-panels` snaps to `collapsedSize`, and the window
 * paints it collapsed. The effect below is then only about *changes* — a region put away or
 * brought back while the window is up — where a frame's delay is no more than the click's own.
 */
function Slot({
  side,
  placed,
  started,
  drawn,
  handle,
  children,
}: {
  side: SlotId;
  /** What is in this slot now. */
  placed: Placement[];
  /** What was in it when the window launched, which is what sized it. */
  started: Placement[];
  drawn: SlotContent;
  /** The slot's imperative handle, where the frame acts on it too (the Panel's reset). */
  handle?: ReturnType<typeof usePanelRef>;
  /** What it draws instead of its regions one after another: the Panel's halves. */
  children?: ReactNode;
}) {
  const shown = shownIn(placed);
  const open = shown.length > 0;
  // **A sidebar is a named region while it is out** (#1751), by its region's catalogue name, as
  // each half of the Panel is (P-13): F6 lands in it and a screen reader says which. The Panel's
  // own slot is not one; its halves are. Put away, it is none: an empty landmark is noise.
  const named = side === "panel" || !open ? undefined : CATALOGUE[shown[0].id].name;
  const own = usePanelRef();
  const panel = handle ?? own;
  // Brought back while the window is up, as opposed to open since launch: only the first is
  // drawn arriving, so a window does not fade its own regions in every time it starts.
  const [arrived, arrivedEnd] = useArrival(open);
  // The library's panel takes no handler of ours: its element is found by its id, and the one
  // heard is the box inside it that carries the class, and so the animation.
  useEffect(() => {
    const element = document.getElementById(panelOf(side))?.firstElementChild;
    if (!(element instanceof HTMLElement)) return;
    const ended = (e: AnimationEvent) => arrivedEnd(e);
    element.addEventListener("animationend", ended);
    return () => element.removeEventListener("animationend", ended);
  }, [side, arrivedEnd]);

  /** How big it should be when it is brought back. */
  const wanted = slotLength(side, placed);
  // **Where it starts, read once**: the length is the window's (the right panel's 300px or its
  // share of the row, the bottom row's share by the window's height), and a `defaultSize` that
  // changed when the window did would re-register the panel.
  const [starts] = useState(() => startingLength(side, started));

  const before = useRef(open);
  useEffect(() => {
    const it = panel.current;
    // Only a slot that has just been emptied or filled is acted on. `wanted` is in the
    // dependencies because it is read below and React insists, but a run that is only about a
    // size stops here — that size is one the operator's own drag has already applied, and
    // pushing it back at the panel would be charter resizing what somebody is holding.
    const changed = before.current !== open;
    before.current = open;
    if (it === null) return;
    if (!open) {
      // Idempotent, and a no-op on the launch of a window whose arrangement already started
      // this slot at nothing — the library will not collapse what is already collapsed.
      it.collapse();
      return;
    }
    // Nothing to do at the launch: `defaultSize` has already put the slot where the
    // arrangement says, and a resize here would be a second layout for no change — in the real
    // window, one that lands while the group is still measuring itself.
    if (!changed) return;
    it.expand();
    // `expand()` goes back to the size the panel had when it was collapsed, and a slot that was
    // already away when the window launched never had one — it started at nothing, so the
    // library would bring it back at its minimum. The remembered size is what it should come
    // back to, and saying so is cheaper than painting it once to find out.
    it.resize(wanted);
  }, [panel, open, wanted]);

  return (
    <Panel
      id={panelOf(side)}
      className={`region-slot slot-${side}${arrived ? " arrived" : ""}`}
      data-slot={side}
      // A side is a panel (`ui/panel`); the bottom slot only holds its halves, each one.
      {...(side === "panel" ? {} : panelRoot("side"))}
      // A region only while it has a name: an unnamed one is no landmark.
      role="region"
      aria-label={named}
      panelRef={panel}
      // **Every one of these is constant for the life of the group, and that is the point.**
      // See this component's docstring: changing a panel's constraints re-registers it, and a
      // separator that recalculates in between indexes past the end of the constraint list.
      // `started` is the arrangement the window launched with, snapshotted for this reason.
      collapsible
      collapsedSize="0%"
      defaultSize={starts}
      minSize={leastOf(side)}
      maxSize={`${SLOTS[side].most}%`}
    >
      {/* A region without views put away is unmounted (above); one with views is kept,
          hidden, because its views are where the person's folds and scroll are (#1673). */}
      {children !== undefined ? (
        // **The Panel's halves** (P-1, #1738): `Halves`.
        children
      ) : (
        <div className="slot-body">
          {side === "panel" ? null : drawn.sideTop(side)}
          {placed.map((one) =>
            drawn.withViews(one) ? (
              <RegionViews key={one.id} placed={one} drawn={drawn} swaps={side === "left"} />
            ) : one.collapsed ? null : (
              <Fragment key={one.id}>{drawn.content[one.id]}</Fragment>
            ),
          )}
        </div>
      )}
    </Panel>
  );
}

/** The handle between a slot and the centre, or between the editor and the Panel. It stays in
 *  the group when its slot is empty — see `Slot` for why — and draws nothing, so there is no
 *  line to drag. The Panel's is double-clicked back to its default height (P-12): charter's,
 *  and not the library's own, which would go back to the height the window launched at.
 *
 *  **Where a double-click lands is the library's hit area, not the handle's element.** The
 *  handle is drawn 1px thick and the library takes a press anywhere in a band 10px across it
 *  (20px for a finger), so a double-click a person or a scenario makes on it usually lands on
 *  the panel beside it: an `onDoubleClick` on the element missed it, and the Panel stayed where
 *  it was. So the window is listened to, in the capture phase, and a press in the band counts:
 *  a `dblclick`, or two presses within {@link DOUBLE_PRESS_MS}. The real window's WebDriver
 *  sends only `mousedown` and `click` for a double-click, with no pointer event and no
 *  `dblclick` (seen on CI), and the library prevents the default of a person's presses. Capture
 *  comes before the library's own listener records where its drag starts, so the second press
 *  commits the reset height. A reset said twice is the same reset.
 *
 *  **It is named for what moving it does** (DS-6, #629): the library draws it as a focusable
 *  `separator` its arrow keys move, which is a control, and a screen reader says a nameless one
 *  as "separator" alone. */
function Edge({ open, named, onReset }: { open: boolean; named: string; onReset?: () => void }) {
  const element = useHandleReset(onReset);
  return (
    <Separator
      aria-label={named}
      className={open ? undefined : "edge-gone"}
      disableDoubleClick={onReset !== undefined}
      elementRef={element}
    />
  );
}

/**
 * **A handle's own double-click reset**, heard where the library takes presses ({@link Edge}
 * says why): the element to give the `Separator` as its `elementRef`. Nothing is listened to
 * while there is no `onReset`.
 */
function useHandleReset(onReset: (() => void) | undefined) {
  const element = useRef<HTMLDivElement | null>(null);
  // The latest reset, read when a press lands: the listeners below are set once, so a press
  // and the next one are counted across the frame's renders.
  const reset = useRef(onReset);
  useEffect(() => {
    reset.current = onReset;
  });
  const resets = onReset !== undefined;
  useEffect(() => {
    if (!resets) return;
    let last: number | undefined;
    /** When the last `pointerdown` came: the `mousedown` a browser sends after it is the same
     *  press, not a second one. */
    let pointer = Number.NEGATIVE_INFINITY;
    const onIt = (e: MouseEvent) => {
      const it = element.current;
      return it !== null && inHandleBand(it.getBoundingClientRect(), e);
    };
    const pressed = (e: MouseEvent) => {
      const now = performance.now();
      if (e.type === "pointerdown") pointer = now;
      else if (now - pointer < SAME_PRESS_MS) return;
      if (e.button > 0 || !onIt(e)) return;
      if (last !== undefined && now - last <= DOUBLE_PRESS_MS) {
        last = undefined;
        reset.current?.();
      } else last = now;
    };
    const doubled = (e: MouseEvent) => {
      if (onIt(e)) reset.current?.();
    };
    // A press is a `pointerdown`, or a `mousedown` where none came: the real window's WebDriver
    // presses with mouse events alone, and sends no `dblclick` for two of them.
    window.addEventListener("pointerdown", pressed, true);
    window.addEventListener("mousedown", pressed, true);
    window.addEventListener("dblclick", doubled, true);
    return () => {
      window.removeEventListener("pointerdown", pressed, true);
      window.removeEventListener("mousedown", pressed, true);
      window.removeEventListener("dblclick", doubled, true);
    };
  }, [resets]);
  return element;
}

/** How close two presses of a handle are to be one double-click: the common desktop default. */
const DOUBLE_PRESS_MS = 500;

/** How soon after a `pointerdown` a `mousedown` is that press's own, sent after it. */
const SAME_PRESS_MS = 100;

/** Whether a press at `e` is on a handle drawn at `box`: within the band the library takes
 *  presses in (`resizeTargetMinimumSize`, 10px, 20px for a finger), across its thin side. A
 *  handle with no box at all is not drawn (its slot put away, or not laid out yet), and is
 *  never pressed: its empty box sits at the window's corner, where the title bar is. */
export function inHandleBand(
  box: { left: number; right: number; top: number; bottom: number; width: number; height: number },
  e: { clientX: number; clientY: number; pointerType?: string },
): boolean {
  if (box.width === 0 && box.height === 0) return false;
  const band = e.pointerType === "touch" || e.pointerType === "pen" ? 20 : 10;
  const across = (from: number, to: number, at: number) => {
    const grow = Math.max(0, band - (to - from)) / 2;
    return at >= from - grow && at <= to + grow;
  };
  return across(box.left, box.right, e.clientX) && across(box.top, box.bottom, e.clientY);
}
