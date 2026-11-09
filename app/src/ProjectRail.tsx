// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  type CSSProperties,
  type ReactNode,
  type RefObject,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { closestCenter, DndContext } from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { Plus } from "lucide-react";
import type { Catalogued, Offer, Project, projectRows } from "./actions";
import { Menued } from "./Menus";
import {
  DOWN_THE_RAIL,
  keepsTheFocus,
  SortableTab,
  stripAccessibility,
  useStripSensors,
} from "./sortable";
import { closeOnDelete } from "./tabKeys";
import { AsStripTab, useTabIds, useTabStrip } from "./ui/tabs/TabStrip";
import { motionReduced } from "./theme/motion";
import { inForce } from "./theme/theme";
import { railSliding, slideEnded, slideStarted } from "./railSlide";
import { useRailCrowded } from "./railRoom";
import {
  Rail,
  RailAction,
  RailBelow,
  RailList,
  RailProject,
  RailProjectWay,
  RailSlot,
  RailSpacer,
  SidebarIcon,
} from "./ui/rail/Rail";
import { useProjectTints } from "./ui/rail/projectColour";
import { watchWindowLeft } from "./ui/windowLeft";
import type { RailMode } from "./railMode";

/**
 * **The project rail** (#2915, spec #2897 R76–R89): the window's projects as a column at its far
 * left, below the title bar. It replaces the project tabs that were in the title bar (ADR 0054),
 * which are gone for good (N22): a project is its monogram here, its name in its tooltip.
 *
 * ```
 * ┌──┐
 * │CH│ ← the project in front, ringed (R78); a badge at its corner counts what waits (R79)
 * │VO│
 * │+ │ ← open a project (R80)
 * │  │
 * │▥ │ ← hide the rail: it peeks at the window's left edge after (R82)
 * └──┘
 * ```
 *
 * - **What a project's button does is the catalogue's** (`actions.projectRows`), as the tabs'
 *   was: a press runs its `Switch to project` row, which brings the project in front on the
 *   workspace it was last on (the project's view keeps it, `projectShown.ts`), or its first; its
 *   right-click menu is the same rows the palette lists (close, pin, its settings, a window of
 *   its own); Delete closes it; Shift+Space picks it up to move it up or down the rail (SI-6),
 *   and putting it among the pinned ones pins it.
 * - **Always shown or auto-hide** (R82, R83, `railMode.ts`): the rail's last button hides it,
 *   and while it is hidden the title bar's first button shows it again; the pointer at the
 *   window's left edge peeks it (R86), and the peek's last button keeps it shown (R88).
 * - **Hiding and showing slide** (R84): the column closes from 52 to 0 over `duration.slide` on
 *   the one ease while the rail slides out and fades over `duration.leave`, and the work column
 *   glides with it; showing is the reverse. Under reduced motion both are instant (R85). Pinned
 *   from the peek, it settles in its column without sliding again (R88).
 * - **It never crowds the work** (#3077, `railRoom.ts`): in a window too narrow for it, `mode` is
 *   auto-hide though the person left it shown, and it comes back as the window widens. That hide
 *   and show are the window's, so they are drawn at once, with no slide.
 */
export function ProjectRail({
  projects,
  inFront,
  rows,
  offers,
  onPress,
  waitingIn,
  unsavedIn,
  onDrop,
  lettingGo,
  mode,
  onMode,
  edge,
  nameOf,
  coming = [],
  loadOf = () => undefined,
  onChoose = () => undefined,
  onRetry = () => undefined,
  onDismiss = () => undefined,
}: {
  /** The window's projects in the rail's order: pinned first, then the rest. While a launch
   *  puts projects back, the ones still waiting for their open are among them (#5323). */
  projects: readonly Project[];
  inFront: string | undefined;
  /** The rows the rail runs (`actions.projectRows`). */
  rows: ReturnType<typeof projectRows>;
  /** The same rows by id, which a project's menu looks one up in. */
  offers: Catalogued;
  onPress: (offer: Offer) => void;
  /** How many things wait on the person in a project (R79). */
  waitingIn: (project: Project) => number;
  /** A project's unsaved work, for its dot (charter-app#302). */
  unsavedIn: (project: Project) => { blocked: boolean; says: string } | undefined;
  /** A project dragged onto another (SI-6). */
  onDrop: (moved: string, onto: string) => void;
  /** The closes this window asked that have not let go of their project yet (#1949). */
  lettingGo: number;
  mode: RailMode;
  onMode: (mode: RailMode) => void;
  /** The element below the title bar whose left edge the pointer peeks the rail at (R86). */
  edge: RefObject<HTMLElement | null>;
  /** What a screen reader calls a project being dragged. */
  nameOf: (plane: string) => string;
  /** The projects of `projects` a launch is still opening (#5323): drawn, loading, with no
   *  menu and no drag, since nothing of theirs is open to act on yet. */
  coming?: readonly string[];
  /** How a project's load is going, for its row (#5323). */
  loadOf?: (project: Project) => RowLoad | undefined;
  /** A project still waiting its turn to open, chosen: it goes next, and in front (#5323). */
  onChoose?: (plane: string) => void;
  /** Retry on a project's row (#5324). */
  onRetry?: (plane: string) => void;
  /** A project whose open failed, put away (#5324). */
  onDismiss?: (plane: string) => void;
}) {
  const hidden = mode === "auto";
  const loadingNow = projects.filter((project) => loadOf(project)?.loading).length;
  const failedNow = projects.filter((project) => loadOf(project)?.failed).length;
  const peek = useEdgePeek(hidden, edge);
  /** The slide, told to what fits itself to the work (`railSlide.ts`) so it fits once (R84). */
  const [settledAs, setSettledAs] = useState(hidden);
  /** Kept shown from the peek: the rail settles in its column with no slide (R88). */
  const [settling, setSettling] = useState(false);
  useEffect(() => {
    if (!settling) return;
    // The frame after the one it was drawn in, so the column's width is already 52.
    let second = 0;
    const first = requestAnimationFrame(() => {
      second = requestAnimationFrame(() => setSettling(false));
    });
    return () => {
      cancelAnimationFrame(first);
      cancelAnimationFrame(second);
    };
  }, [settling]);

  // Hidden or shown by the window, not the person (`railRoom.ts`, #3077): the window was resized,
  // or a sidebar opened or put away, and the rail follows that change at once rather than
  // gliding the work after it.
  const crowded = useRailCrowded();
  const [crowdedAs, setCrowdedAs] = useState(crowded);
  const byTheWindow = crowdedAs !== crowded;
  if (byTheWindow) setCrowdedAs(crowded);

  // The column starts to open or close: what fits itself to the work waits for the end (B1). Not
  // when it settles from the peek or follows the window, and not under reduced motion, which
  // slide nothing.
  if (settledAs !== hidden) {
    setSettledAs(hidden);
    if (byTheWindow) setSettling(true);
    else if (!settling && !motionReduced()) slideStarted();
  }
  // Hidden by the window with the keyboard in it: the keyboard goes where the person's own hide
  // sends it, the title bar's show, before the rail is out of reach and drops it.
  useLayoutEffect(() => {
    if (!hidden) return;
    const slot = document.querySelector('[data-scope="rail"][data-part="slot"]');
    if (slot?.contains(document.activeElement))
      document.querySelector<HTMLElement>("[data-rail-show]")?.focus({ preventScroll: true });
  }, [hidden]);
  useEffect(() => {
    if (!railSliding()) return;
    // Its length, should the column's transition never say it ended (a hidden window).
    const timer = setTimeout(slideEnded, inForce().motion.durations["duration.slide"] + 100);
    return () => clearTimeout(timer);
  }, [hidden]);
  // The rail gone mid-slide (its last project closed) ends the slide: what waits for its end
  // would otherwise wait for good, and no terminal would fit itself again.
  useEffect(() => () => slideEnded(), []);

  const hide = useCallback(() => {
    peek.close(true);
    onMode("auto");
    // The button that was pressed has gone with the rail: the keyboard goes to the one that
    // shows it again, first in the title bar.
    requestAnimationFrame(() =>
      document.querySelector<HTMLElement>("[data-rail-show]")?.focus({ preventScroll: true }),
    );
  }, [onMode, peek]);
  const keep = useCallback(() => {
    setSettling(true);
    peek.close(true);
    onMode("shown");
  }, [onMode, peek]);

  const drawn = (peeking: boolean) => (
    <ProjectColumn
      peeking={peeking}
      leaving={peeking && peek.state === "leaving"}
      onLeft={peek.settle}
      projects={projects}
      inFront={inFront}
      rows={rows}
      offers={offers}
      onPress={onPress}
      waitingIn={waitingIn}
      unsavedIn={unsavedIn}
      onDrop={onDrop}
      lettingGo={lettingGo}
      nameOf={nameOf}
      coming={coming}
      loadOf={loadOf}
      onChoose={onChoose}
      onRetry={onRetry}
      onDismiss={onDismiss}
      last={
        peeking ? (
          <RailAction
            aria-label="Keep the projects sidebar shown"
            icon={<SidebarIcon />}
            onClick={keep}
            data-rail-toggle="keep"
          />
        ) : (
          <RailAction
            aria-label="Hide the projects sidebar"
            tooltip="Shows again at the window's left edge"
            icon={<SidebarIcon />}
            onClick={hide}
            data-rail-toggle="hide"
          />
        )
      }
    />
  );

  return (
    <>
      <RailSlot hidden={hidden} settling={settling} onSlid={slideEnded}>
        {drawn(false)}
      </RailSlot>
      {hidden && peek.state !== "closed" && drawn(true)}
      {/* How many projects are still loading, said as it changes (#5323): each row's mark is
          its description too, which a screen reader hears only on reaching it. */}
      <span className="sr-only" role="status">
        {loadingNow === 0
          ? ""
          : `Loading ${loadingNow} ${loadingNow === 1 ? "project" : "projects"}…`}
        {failedNow === 0
          ? ""
          : ` ${failedNow} ${failedNow === 1 ? "project" : "projects"} could not open.`}
      </span>
    </>
  );
}

/** How a project's load is going, as its row draws it (`App.tsx`, `projectLoads.ts`). */
export type RowLoad = { loading: boolean; failed: boolean; says?: string; retry: boolean };

/** What a row offers while its load fell short (#5324): Retry, and Dismiss if it asks for it. */
function RowWays({
  project,
  load,
  onRetry,
  onDismiss,
  dismisses = false,
}: {
  project: Project;
  load: RowLoad | undefined;
  onRetry: (plane: string) => void;
  onDismiss: (plane: string) => void;
  /** Its open failed, so no menu can close it. */
  dismisses?: boolean;
}) {
  return (
    <>
      {load?.retry && (
        <RailProjectWay doing="retry" name={project.name} onPress={() => onRetry(project.plane)} />
      )}
      {dismisses && load?.failed && (
        <RailProjectWay
          doing="dismiss"
          name={project.name}
          onPress={() => onDismiss(project.plane)}
        />
      )}
    </>
  );
}

/** The rail's own column of projects and its two actions, in its slot or peeking. */
function ProjectColumn({
  peeking,
  leaving,
  onLeft,
  projects,
  inFront,
  rows,
  offers,
  onPress,
  waitingIn,
  unsavedIn,
  onDrop,
  lettingGo,
  nameOf,
  coming,
  loadOf,
  onChoose,
  onRetry,
  onDismiss,
  last,
}: {
  peeking: boolean;
  leaving: boolean;
  onLeft: () => void;
  projects: readonly Project[];
  inFront: string | undefined;
  rows: ReturnType<typeof projectRows>;
  offers: Catalogued;
  onPress: (offer: Offer) => void;
  waitingIn: (project: Project) => number;
  unsavedIn: (project: Project) => { blocked: boolean; says: string } | undefined;
  onDrop: (moved: string, onto: string) => void;
  lettingGo: number;
  nameOf: (plane: string) => string;
  coming: readonly string[];
  loadOf: (project: Project) => RowLoad | undefined;
  onRetry: (plane: string) => void;
  onDismiss: (plane: string) => void;
  onChoose: (plane: string) => void;
  last: ReactNode;
}) {
  // A project's rows (`rows`) are the open projects' alone, in their order: a project still
  // coming has none, and is skipped in counting the place of each that has (#5323).
  const rowAt = useMemo(() => {
    const at = new Map<string, number>();
    for (const project of projects)
      if (!coming.includes(project.plane)) at.set(project.plane, at.size);
    return at;
  }, [coming, projects]);
  const tabId = useTabIds();
  const tintOf = useProjectTints();
  const sensors = useStripSensors();
  const words = useMemo(() => stripAccessibility("project", (id) => nameOf(String(id))), [nameOf]);
  const ids = projects.map((project) => project.plane);
  const strip = useTabStrip({ keys: ids, selected: inFront });
  const {
    list: belowList,
    count: belowCount,
    show: showBelow,
  } = useWaitingBelow(projects.map(waitingIn));
  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      modifiers={DOWN_THE_RAIL}
      accessibility={words}
      onDragEnd={({ active, over }) => {
        if (over) onDrop(String(active.id), String(over.id));
      }}
    >
      <SortableContext items={ids} strategy={verticalListSortingStrategy}>
        <Rail
          name="Projects"
          strip={strip}
          tabs={projects.map((_, place) => tabId(place))}
          peek={peeking}
          data-strip="Projects"
          data-leaving={leaving ? "" : undefined}
          // The closes this window asked that have not let go of their project yet (#1949):
          // a scenario spec waits for none before the next one runs.
          data-letting-go={lettingGo}
          onAnimationEnd={(event) => {
            if (leaving && event.target === event.currentTarget) onLeft();
          }}
        >
          <RailList ref={belowList}>
            {projects.map((project, place) =>
              coming.includes(project.plane) ? (
                <SortableTab key={project.plane} id={project.plane} fixed>
                  {({ sortable, style }) => (
                    <>
                      <AsStripTab
                        strip={strip}
                        tabKey={project.plane}
                        selected={false}
                        ref={sortable.setNodeRef}
                      >
                        {(tab) => (
                          <RailProject
                            {...tab}
                            style={style as CSSProperties}
                            id={tabId(place)}
                            name={project.name}
                            current={false}
                            colour={tintOf(project.name)}
                            load={loadOf(project)}
                            data-directory={project.plane}
                            data-coming=""
                            onClick={() => onChoose(project.plane)}
                          />
                        )}
                      </AsStripTab>
                      <RowWays
                        project={project}
                        load={loadOf(project)}
                        onRetry={onRetry}
                        onDismiss={onDismiss}
                        dismisses
                      />
                    </>
                  )}
                </SortableTab>
              ) : (
                <SortableTab key={project.plane} id={project.plane}>
                  {({ sortable, style }) => (
                    // Right-click is the catalogue's third reader (`Menus.tsx`), on the button,
                    // which takes the keyboard's context-menu key too.
                    <>
                      <Menued
                        on={{ on: "project", plane: project.plane }}
                        offers={offers}
                        onPress={onPress}
                      >
                        <AsStripTab
                          strip={strip}
                          tabKey={project.plane}
                          selected={project.plane === inFront}
                          ref={sortable.setNodeRef}
                        >
                          {(tab) => (
                            <RailProject
                              {...tab}
                              style={style as CSSProperties}
                              id={tabId(place)}
                              name={project.name}
                              current={project.plane === inFront}
                              waiting={waitingIn(project)}
                              unsaved={unsavedIn(project)}
                              colour={tintOf(project.name)}
                              load={loadOf(project)}
                              // Its directory: how a scenario learns which project a button is.
                              data-directory={project.plane}
                              data-dragging={sortable.isDragging || undefined}
                              aria-describedby={sortable.attributes["aria-describedby"]}
                              {...sortable.listeners}
                              onKeyDown={(event) => {
                                // A button that is up is being carried: its keys are the drag's.
                                keepsTheFocus(event, sortable.isDragging);
                                sortable.listeners?.onKeyDown?.(event);
                                if (!sortable.isDragging)
                                  closeOnDelete(
                                    event,
                                    rows.close[rowAt.get(project.plane) ?? place],
                                    onPress,
                                  );
                              }}
                              onClick={() => {
                                const offer = rows.switchTo[rowAt.get(project.plane) ?? place];
                                if (offer.available) onPress(offer);
                              }}
                            />
                          )}
                        </AsStripTab>
                      </Menued>
                      <RowWays
                        project={project}
                        load={loadOf(project)}
                        onRetry={onRetry}
                        onDismiss={onDismiss}
                      />
                    </>
                  )}
                </SortableTab>
              ),
            )}
            <span data-strip-doing="">
              <RailAction
                aria-label={rows.open.title}
                icon={<Plus aria-hidden="true" />}
                onClick={() => onPress(rows.open)}
              />
            </span>
          </RailList>
          {belowCount > 0 && <RailBelow waiting={belowCount} onPress={showBelow} />}
          <RailSpacer />
          <span data-strip-doing="">{last}</span>
        </Rail>
      </SortableContext>
    </DndContext>
  );
}

/**
 * **How many things wait in the projects scrolled out of the rail's list**, followed as it
 * scrolls and changes size (N5), and the way to bring the first of them into view. None where
 * nothing is laid out.
 */
function useWaitingBelow(waiting: readonly number[]) {
  const list = useRef<HTMLDivElement>(null);
  const [count, setCount] = useState(0);
  const key = waiting.join(",");
  useEffect(() => {
    const element = list.current;
    if (!element) return;
    const counted = () => {
      const fold = element.scrollTop + element.clientHeight;
      const tabs = [...element.querySelectorAll<HTMLElement>('[role="tab"]')];
      const sum = tabs.reduce(
        (total, tab, at) =>
          tab.offsetTop + tab.offsetHeight > fold ? total + (waiting[at] ?? 0) : total,
        0,
      );
      setCount(element.clientHeight > 0 ? sum : 0);
    };
    counted();
    element.addEventListener("scroll", counted, { passive: true });
    const sized = new ResizeObserver(counted);
    sized.observe(element);
    return () => {
      element.removeEventListener("scroll", counted);
      sized.disconnect();
    };
    // `key` stands for `waiting`, which is a new array each render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  const show = useCallback(() => {
    const element = list.current;
    if (!element) return;
    const fold = element.scrollTop + element.clientHeight;
    const tabs = [...element.querySelectorAll<HTMLElement>('[role="tab"]')];
    tabs
      .find((tab, at) => (waiting[at] ?? 0) > 0 && tab.offsetTop + tab.offsetHeight > fold)
      ?.scrollIntoView({ block: "nearest" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return { list, count, show };
}

/** Where the pointer peeks the rail: within this many px of the window's left edge (R86). */
export const PEEK_AT = 6;

/** Where the peek leaves: once the pointer is further than this from the edge (R87). */
export const PEEK_UNTIL = 70;

/**
 * **How long the pointer rests at the edge before the rail peeks**, in ms: a pointer thrown at
 * the window's edge to reach the activity bar there passes through, and its click is the bar's,
 * never the peek's.
 */
export const PEEK_DWELL = 180;

/** The peek's state: not drawn, in, or sliding out. */
export type Peek = "closed" | "open" | "leaving";

/** The peek, and since when the pointer has rested at the edge without it. */
export type Peeking = { peek: Peek; since: number | null };

/** No peek, and the pointer not resting at the edge. */
export const NO_PEEK: Peeking = { peek: "closed", since: null };

/** One position of the pointer, as {@link peekAfter} reads it. */
export type PointerAt = {
  /** From the left edge of the window below the title bar. */
  x: number;
  /** Below the title bar, inside the window. */
  under: boolean;
  /** When, in ms. */
  time: number;
  /** A button is held: a selection or a drag that brushes the edge peeks nothing. */
  pressed: boolean;
  /** The first move since the pointer came into the window, which has not rested anywhere. */
  fresh: boolean;
};

/**
 * **What the pointer does to the peek** (R86, R87): resting at the left edge, below the title
 * bar, for {@link PEEK_DWELL} with no button held, it comes in (or back in while it goes); further
 * than {@link PEEK_UNTIL} from the edge it goes. A pointer passing through, pressing, or only just
 * come into the window peeks nothing.
 */
export function peekAfter(was: Peeking, at: PointerAt): Peeking {
  if (was.peek === "open") return at.x > PEEK_UNTIL ? { peek: "leaving", since: null } : was;
  const resting = at.under && at.x >= 0 && at.x < PEEK_AT && !at.pressed && !at.fresh;
  if (!resting) return was.since === null ? was : { ...was, since: null };
  const since = was.since ?? at.time;
  return at.time - since >= PEEK_DWELL ? { peek: "open", since: null } : { ...was, since };
}

/** The peek, followed from the pointer while the rail is hidden. */
function useEdgePeek(active: boolean, edge: RefObject<HTMLElement | null>) {
  const [held, setHeld] = useState<Peeking>(NO_PEEK);
  // Shown, the rail has no peek, and none is left behind for its next hide (S3).
  const [heldFor, setHeldFor] = useState(active);
  if (heldFor !== active) {
    setHeldFor(active);
    setHeld(NO_PEEK);
  }
  const state: Peek = active ? held.peek : "closed";
  const now = useRef(held);
  useLayoutEffect(() => {
    now.current = held;
  });
  useEffect(() => {
    if (!active) return;
    const element = edge.current;
    // The window below the title bar's box, kept as it changes rather than asked on each move.
    let box = element?.getBoundingClientRect();
    const sized = new ResizeObserver(() => {
      box = element?.getBoundingClientRect();
    });
    if (element) sized.observe(element);
    let outside = false;
    let last: PointerAt | undefined;
    let rest: ReturnType<typeof setTimeout> | undefined;
    const step = (at: PointerAt) => {
      last = at;
      const next = peekAfter(now.current, at);
      if (next !== now.current) {
        now.current = next;
        setHeld(next);
      }
      clearTimeout(rest);
      // Resting without moving: asked again once the dwell has passed.
      if (next.since !== null)
        rest = setTimeout(() => {
          if (last) step({ ...last, time: last.time + PEEK_DWELL });
        }, PEEK_DWELL);
    };
    const move = (event: PointerEvent) => {
      if (!box) return;
      step({
        x: event.clientX - box.left,
        under: event.clientY >= box.top && event.clientY <= box.bottom,
        time: event.timeStamp,
        pressed: event.buttons !== 0,
        fresh: outside,
      });
      outside = false;
    };
    // The pointer gone from the window, or the window from the front: the peek goes (S4).
    const away = () => {
      outside = true;
      clearTimeout(rest);
      const was = now.current;
      if (was.peek !== "open" && was.since === null) return;
      const next: Peeking =
        was.peek === "open" ? { peek: "leaving", since: null } : { ...was, since: null };
      now.current = next;
      setHeld(next);
    };
    document.addEventListener("pointermove", move);
    document.addEventListener("pointerdown", move);
    const unwatch = watchWindowLeft(away);
    return () => {
      clearTimeout(rest);
      sized.disconnect();
      document.removeEventListener("pointermove", move);
      document.removeEventListener("pointerdown", move);
      unwatch();
    };
  }, [active, edge]);
  // The peek's slide out ends it; where no animation runs to say so (reduced motion plays none
  // that lasts, and a window that draws no stylesheet plays none at all), its length does.
  useEffect(() => {
    if (state !== "leaving") return;
    const lasts = motionReduced() ? 0 : inForce().motion.durations["duration.leave"];
    const timer = setTimeout(
      () => setHeld((was) => (was.peek === "leaving" ? NO_PEEK : was)),
      lasts + 50,
    );
    return () => clearTimeout(timer);
  }, [state]);
  const settle = useCallback(() => setHeld((was) => (was.peek === "leaving" ? NO_PEEK : was)), []);
  const close = useCallback(
    (at: boolean) => setHeld(at ? NO_PEEK : { peek: "leaving", since: null }),
    [],
  );
  return { state, settle, close };
}
