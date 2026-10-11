import { Fragment, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { Group, Panel, Separator, usePanelRef, type Layout } from "react-resizable-panels";
import { BellRing, ChevronDown, Compass, PanelBottom } from "lucide-react";
import { ActivityBar, VIEW_MARKS, type PanelTab, type Tabbed } from "./ActivityBar";
import {
  CATALOGUE,
  HALVES,
  halvesSplit,
  inSlots,
  leastOf,
  openView,
  shownIn,
  sizeForgotten,
  slotSize,
  SLOT_IDS,
  SLOTS,
  SPLIT,
  startingSize,
  VIEWS,
  type Arrangement,
  type Placement,
  type RegionId,
  type Sash,
  type Side,
  type SlotId,
  type ViewId,
} from "./regions";
import { useArrived } from "./lib/arrived";
import { HalfActions } from "./halfHead";

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
  arrangement,
  content,
  views,
  panels = [],
  badges,
  keys,
  onPick,
  centre,
  onResized,
  onReset,
  onSplit,
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
}) {
  const slots = inSlots(arrangement);
  const base = useId();
  const ids = {
    panelOf: (view: ViewId) => `${base}view-${view}`,
    tabOf: (view: ViewId) => `${base}tab-${view}`,
  };
  const withViews = (one: Placement) =>
    views !== undefined && CATALOGUE[one.id].views !== undefined;
  const present = panels.map((one) => one.view);
  /** A region's views, as its bar draws them: purlis's own, then the panels, where it takes them. */
  const tabbed = (id: RegionId): Tabbed[] => [
    ...(CATALOGUE[id].views ?? []).map((view) => ({
      view,
      name: VIEWS[view].name,
      mark: VIEW_MARKS[view],
    })),
    ...(CATALOGUE[id].panels ? panels : []),
  ];
  /** A region's bar, at its side's edge. */
  const barOf = (one: Placement, side: Side, away?: string) => (
    <ActivityBar
      key={one.id}
      side={side}
      region={one.id}
      name={CATALOGUE[one.id].name}
      away={away}
      views={tabbed(one.id)}
      open={one.collapsed ? undefined : openView(one, present)}
      keys={keys}
      badges={badges}
      {...ids}
      onPick={(view) => onPick?.(view)}
    />
  );
  /**
   * **A side's icon column** (P-1, #1737): the bars of the regions in its sidebar, then, below a
   * divider, the **Lower icons** — the bars of its halves of the Panel. One column the window's
   * whole height, so one strip of icons serves the sidebar and the half under it. A half with
   * no views has no icons, and no divider is drawn for nothing.
   */
  const column = (side: Side) => {
    const upper = slots[side].filter(withViews).map((one) => barOf(one, side));
    const lower = slots.panel
      .filter((one) => one.side === side && withViews(one) && tabbed(one.id).length > 0)
      .map((one) => barOf(one, side, `Put the panel's ${side} half away`));
    if (upper.length === 0 && lower.length === 0) return null;
    return (
      <div className="activity-column" data-side={side}>
        {upper}
        {lower.length > 0 && <div className="activity-divider" aria-hidden="true" />}
        {lower}
      </div>
    );
  };
  const drawn: SlotContent = { content, views, ids, withViews, tabbed, present, onPick };
  /** The Panel's slot, held here so a double-click of its handle can size it (P-12). */
  const panelSlot = usePanelRef();
  const resetPanel = () => {
    panelSlot.current?.resize(
      `${slotSize("panel", inSlots(sizeForgotten(arrangement, "panel")).panel)}%`,
    );
    onReset?.("panel");
  };

  // **How big each slot starts, read once.** `defaultSize` is a constraint, and a constraint
  // that changes re-registers the panel — charter-app#141 again. The arrangement the window
  // launched with is what sizes the slots; a size remembered mid-session is remembered for the
  // next launch and does not move the slot under the operator's hands.
  const [started] = useState(() => inSlots(arrangement));

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
    <div className="region-frame">
      {column("left")}
      <Group className="regions" orientation="vertical" onLayoutChanged={settled}>
        <Panel id="region-upper" className="region-upper" minSize="20%">
          <Group className="region-row" orientation="horizontal" onLayoutChanged={settled}>
            <Slot side="left" placed={slots.left} started={started.left} drawn={drawn} />
            <Edge open={shownIn(slots.left).length > 0} />
            <Panel id="region-centre" className="region-centre" minSize="20%">
              {centre}
            </Panel>
            <Edge open={shownIn(slots.right).length > 0} />
            <Slot side="right" placed={slots.right} started={started.right} drawn={drawn} />
          </Group>
        </Panel>
        <Edge open={shownIn(slots.panel).length > 0} onReset={resetPanel} />
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
};

/**
 * A region with views, in its slot: each view a tab panel, the open one shown and the rest
 * mounted and `hidden` — out of the tab order and the accessibility tree, and still holding
 * what the person did in it. Put away, every one of them is hidden.
 */
function RegionViews({
  placed,
  drawn,
  actions,
}: {
  placed: Placement;
  drawn: SlotContent;
  /** In a half of the Panel: where each view's own actions go on the half's line. */
  actions?: Partial<Record<ViewId, HTMLElement | null>>;
}) {
  const open = placed.collapsed ? undefined : openView(placed, drawn.present);
  return (
    <div className="region-views" data-region={placed.id} hidden={placed.collapsed}>
      {drawn.tabbed(placed.id).map(({ view }) => {
        const content = drawn.views?.[view];
        return (
          <div
            key={view}
            role="tabpanel"
            id={drawn.ids.panelOf(view)}
            aria-labelledby={drawn.ids.tabOf(view)}
            data-view={view}
            className="region-view"
            hidden={view !== open}
          >
            {actions === undefined ? (
              content
            ) : (
              <HalfActions.Provider value={actions[view] ?? null}>{content}</HalfActions.Provider>
            )}
          </div>
        );
      })}
    </div>
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
  // Each view's own place on the line, held as state so a view drawn before its line was mounted
  // is drawn again with somewhere to put its actions.
  const [actions, setActions] = useState<Partial<Record<ViewId, HTMLElement | null>>>({});
  // One callback per view for the half's life: a new one each render would be called again on
  // every render, and each call is a render.
  const [refOf] = useState(() => {
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
      <div className="panel-half" data-side={side} hidden={!shown}>
        {placed !== undefined && (
          <>
            <div className="half-head">
              <h2 className="sidebar-title">{title}</h2>
              {views.map(({ view }) => (
                <span
                  key={view}
                  className="half-actions"
                  hidden={view !== open}
                  ref={refOf(view)}
                />
              ))}
              <button
                type="button"
                className="panel-view half-away"
                // WebKit leaves a `<button>` out of the tab sequence without it (#190).
                tabIndex={0}
                aria-label={`Put the panel's ${side} half away`}
                title={`Put the panel's ${side} half away`}
                onClick={() => open !== undefined && drawn.onPick?.(open)}
              >
                <ChevronDown className="node-icon" aria-hidden="true" />
              </button>
            </div>
            <RegionViews placed={placed} drawn={drawn} actions={actions} />
          </>
        )}
      </div>
    </Panel>
  );
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
  const own = usePanelRef();
  const panel = handle ?? own;
  // Brought back while the window is up, as opposed to open since launch: only the first is
  // drawn arriving, so a window does not fade its own regions in every time it starts.
  const arrived = useArrived(open) && open;

  /** How big it should be when it is brought back. */
  const wanted = slotSize(side, placed);

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
    it.resize(`${wanted}%`);
  }, [panel, open, wanted]);

  return (
    <Panel
      id={panelOf(side)}
      className={`region-slot slot-${side}${arrived ? " arrived" : ""}`}
      panelRef={panel}
      // **Every one of these is constant for the life of the group, and that is the point.**
      // See this component's docstring: changing a panel's constraints re-registers it, and a
      // separator that recalculates in between indexes past the end of the constraint list.
      // `started` is the arrangement the window launched with, snapshotted for this reason.
      collapsible
      collapsedSize="0%"
      defaultSize={`${startingSize(side, started)}%`}
      minSize={leastOf(side)}
      maxSize={`${SLOTS[side].most}%`}
    >
      {/* A region without views put away is unmounted (above); one with views is kept,
          hidden, because its views are where the person's folds and scroll are (#1673). */}
      {children !== undefined
        ? // **The Panel's halves** (P-1, #1738): `Halves`.
          children
        : placed.map((one) =>
            drawn.withViews(one) ? (
              <RegionViews key={one.id} placed={one} drawn={drawn} />
            ) : one.collapsed ? null : (
              <Fragment key={one.id}>{drawn.content[one.id]}</Fragment>
            ),
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
 *  commits the reset height. A reset said twice is the same reset. */
function Edge({ open, onReset }: { open: boolean; onReset?: () => void }) {
  const element = useHandleReset(onReset);
  return (
    <Separator
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

/**
 * The button that puts a region away and brings it back (ADR 0038).
 *
 * **Not a row of the catalogue**, deliberately, and this is the one place in the bar where that
 * is true. The catalogue is what a project can DO — open a chat, split a pane, remove a
 * worktree — and every one of its rows is also a palette row, because the palette is the
 * primary input. Whether the operator has the explorer on screen is not a thing to do to the
 * plane; it is how this window is laid out, it is remembered beside the window and not on the
 * plane, and a palette full of "Navigation", "Attention" would be rows of noise in
 * front of a hundred real ones at ADR 0026's limits.
 *
 * `aria-pressed` and not a label that flips between "Show" and "Hide": the name of the thing is
 * what a person looks for, and the state is what the attribute is for.
 *
 * # Its mark, and nothing else — on the status line (charter-app#193)
 *
 * The operator asked for this twice. First *"show hide buttons can be movet to bottom status
 * bar — again like ZED"*, and then, when only half of it had been done: *"you dont moved this 3
 * hide/show bottons to bottom status bar — and let make them without labels, just small icons
 * without texts, texts only with tooltips, i already asked about this — seems you missed."*
 * `StatusLine.tsx` is where they are drawn now; this is what one of them is.
 *
 * **The words do not go away — they move from the content to `aria-label`.** That is the one
 * condition `docs/design-system.md` puts on an icon with no text beside it, and it is not a
 * formality here: `pressOnly("Explorer")` is how the palette and the scenario specs reach a
 * control, and a screen reader reads exactly the same string. An icon-only button whose
 * accessible name is an icon is a button nobody can find, by either route.
 *
 * **The `title` says what pressing it does, which the name cannot.** `aria-label` has to be the
 * name of the thing — `Explorer` — because that is what a person looks for; the tooltip is
 * where *"Put the Explorer region away"* belongs, and it already said it. A tooltip that merely
 * repeated the label would be a tooltip nobody reads twice.
 *
 * **The mark is what the region IS and never where it is**, which was already true and is now
 * the whole of what is drawn — see {@link REGION_MARKS}. A panel-left glyph would be wrong the
 * first time a region moved, and with the words gone it would be the only thing left to be
 * wrong.
 */
export function RegionToggle({
  id,
  shown,
  onToggle,
}: {
  id: RegionId;
  shown: boolean;
  onToggle: (id: RegionId) => void;
}) {
  const name = CATALOGUE[id].name;
  const Mark = REGION_MARKS[id];
  return (
    <button
      type="button"
      className="region-toggle"
      // WebKit leaves a `<button>` out of the tab sequence unless its `tabindex` is written
      // down (`docs/ui-primitives.md`, charter-app#189).
      tabIndex={0}
      aria-pressed={shown}
      aria-label={name}
      title={shown ? `Put the ${name} region away` : `Bring the ${name} region back`}
      onClick={() => onToggle(id)}
    >
      <Mark />
    </button>
  );
}

/**
 * Each region's mark, by what it IS and not by where it is.
 *
 * A panel-left / panel-right icon would be the obvious choice and would be wrong the first
 * time an operator moved a region: the layout is data (`regions.ts`), so the explorer can be
 * on the right, and a toggle drawn as "left panel" would then point at the wrong edge of the
 * window. Navigation is a compass, attention is a bell.
 *
 * A `Record` over `RegionId`, so a region added to the catalogue without a mark here is a type
 * error rather than a toggle with a hole in it.
 */
const REGION_MARKS: Record<RegionId, typeof Compass> = {
  // The region the Chats and Explorer views are in: a way to somewhere, as ADR 0038 reads it.
  // The explorer's folder tree is the Explorer view's own mark now (`ActivityBar.tsx`).
  navigation: Compass,
  aside: BellRing,
  // The Panel's halves are toggled by their Lower icons and ⌘J, not on the status line; a half
  // IS a part of the Panel, so the Panel is its mark.
  "panel-left": PanelBottom,
  "panel-right": PanelBottom,
};
