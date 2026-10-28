// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  Fragment,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";
import { House, Plus } from "lucide-react";
import { OUTSIDE, OUTSIDE_TITLE } from "./actions";
import { pressed } from "./keys/registry";
import { onAMac } from "./tabKeys";
import { type ReportedWorkspaces } from "./planeReport";
import { moves } from "./workspaceSlide";
import { HeldPopover, PopoverFooter, PopoverRule, type KeyHint } from "./ui/popover/Popover";
import { monogramOf } from "./ui/rail/Rail";
import { useProjectTints } from "./ui/rail/projectColour";
import {
  PinToggle,
  SwitcherColumns,
  SwitcherFilter,
  SwitcherIcon,
  SwitcherList,
  SwitcherMark,
  SwitcherRow,
} from "./ui/titlebar/Switcher";
import {
  AllWorkspaces,
  PinnedStrip,
  ProjectCrumb,
  RootChip,
  TitleBarProject,
  TitleBarSlash,
  WorkspaceChip,
} from "./ui/titlebar/TitleBarWorkspaces";
import {
  pinKeySaid,
  pinOfKey,
  projectsFound,
  sideways,
  stepTarget,
  Swipe,
  workspacesFound,
  workspacesKeyCap,
  workspacesKeySaid,
  type Step,
} from "./workspaceSwitch";

/** No pins, one value for every render. */
const NO_PINS: readonly string[] = [];

/** A project as the title bar's switchers list it. */
export type SwitchProject = {
  plane: string;
  /** What the person calls it. */
  name: string;
  /** How many things wait on the person in it, across its workspaces. */
  waiting: number;
  /** Its workspaces, once its plane is read. */
  workspaces?: ReportedWorkspaces;
  /** Why a new workspace cannot be made in it now (a project opened read-only), if it cannot. */
  cannotCreate?: string;
};

/** What the title bar asks the window to do. */
export type SwitchDoes = {
  /** Brings `plane` to the front, on `workspace` (the root's name for the root), sliding the
   *  way `step` went when a step asked for it. */
  go: (plane: string, workspace: string, step?: Step) => void;
  /** Brings `plane` to the front on the workspace it was last on (R81). */
  choose: (plane: string) => void;
  /** Pins `workspace` of `plane`, or lets its pin go. */
  pin: (plane: string, workspace: string, pinned: boolean) => void;
  /** Asks for a new workspace in `plane` (R108). */
  create: (plane: string) => void;
};

/** Which of the two switchers is open, if one is. */
type Open = "workspaces" | "projects" | undefined;

/** The keys the two-column switcher answers to (R100): it steps across to the workspaces too. */
const PROJECT_KEYS: readonly KeyHint[] = [
  { keys: ["↑", "↓"], does: "move" },
  { keys: ["→"], does: "workspaces" },
  { keys: ["⏎"], does: "go" },
  { keys: ["esc"], does: "close" },
];

/** A row's mark: the plane root's house, or a project's monogram. */
const rootMark = (
  <SwitcherIcon>
    <House />
  </SwitcherIcon>
);

/**
 * **The project in front and its workspaces, in the title bar** (#2916, spec #2897 R90–R115,
 * N22): after the window controls, and after the rail's show while the rail hides itself, the
 * project's name, a faint slash, and its pinned workspaces as chips (no limit, each with what
 * waits in it, the current one lit), the dashed chip that pins the current workspace while it is
 * not pinned, and the ⌄ that opens every workspace.
 *
 * - **With the rail shown** the name is words (R92); with it hidden the name is a crumb — its
 *   mark, its name and ⌄ — that opens the two-column switcher of every project and workspace
 *   (R93–R100), since the rail that names the projects is out of sight.
 * - **⌥⌘P** (Ctrl+Alt+Shift+P elsewhere, `workspaceSwitch.opensTheWorkspaces`) opens and closes
 *   the workspace popover, or the two-column switcher while the rail is hidden (R106 as the
 *   operator's ruling of 2026-10-24 amends it; ⌘P stays the palette's). **⌘1–⌘9** (Ctrl+1–9
 *   elsewhere) go to the first nine pins (R111). None of them acts while a dialog is up.
 * - **A sideways swipe over the title bar** steps one pin per gesture, round the ends (R112);
 *   over a strip too long to show whole, it scrolls the strip instead (R103). Anywhere else a
 *   sideways scroll scrolls what is under it.
 * - **A step leaves the current chip in sight** (R104): scrolled to, smoothly, or at once under
 *   reduced motion.
 */
export function TitleBarSwitch({
  projects,
  inFront,
  railShown,
  held,
  does,
  menuOf,
}: {
  projects: readonly SwitchProject[];
  inFront: string;
  /** The rail is shown, so the bar names the project in words; hidden, the name switches. */
  railShown: boolean;
  /** The palette is up: the keys are not this bar's (a modal is the key registry's to find). */
  held?: boolean;
  does: SwitchDoes;
  /** A chip's context menu, around the chip: the workspace's rows of the catalogue, or the
   *  plane root's for its chip (`OUTSIDE`). */
  menuOf?: (workspace: string, chip: ReactNode) => ReactNode;
}) {
  const mac = onAMac();
  const front = projects.find((one) => one.plane === inFront);
  const workspaces = front?.workspaces;
  const pins = workspaces?.pins ?? NO_PINS;
  const focused = workspaces?.focused;
  const [open, setOpen] = useState<Open>();
  const crumb = useRef<HTMLButtonElement>(null);
  const all = useRef<HTMLButtonElement>(null);
  const strip = useRef<HTMLDivElement>(null);
  const house = useRef<HTMLButtonElement>(null);
  const tintOf = useProjectTints();
  /** Where the keyboard was when a switcher opened, to give it back as the switcher closes. */
  const openedFrom = useRef<Element | null>(null);
  // **The keyboard back where it was** once a switcher has closed, by Escape or by a switch:
  // where it was, while that is still drawn, else the chip of the workspace in front: the
  // plane root's house while the root is (#3654), which stands outside the strip.
  const wasOpen = useRef(open);
  useEffect(() => {
    const before = wasOpen.current;
    wasOpen.current = open;
    if (before === undefined || open !== undefined) return;
    const from = openedFrom.current;
    openedFrom.current = null;
    queueMicrotask(() => {
      const at = document.activeElement;
      if (at !== null && at !== document.body && at.isConnected) return;
      if (from instanceof HTMLElement && from.isConnected && from.closest("[hidden]") === null) {
        from.focus({ preventScroll: true });
        return;
      }
      const chosen =
        strip.current?.querySelector<HTMLElement>('[data-part="workspace"][data-selected]') ??
        (house.current?.hasAttribute("data-selected") ? house.current : null) ??
        strip.current?.querySelector<HTMLElement>("[data-titlebar-all]");
      chosen?.focus({ preventScroll: true });
    });
  }, [open]);

  // **Hiding the rail closes the switcher** (R84), and showing it closes the other.
  const [railWas, setRailWas] = useState(railShown);
  if (railWas !== railShown) {
    setRailWas(railShown);
    setOpen(undefined);
  }

  const goTo = (workspace: string, step?: Step) => {
    setOpen(undefined);
    // Choosing the current chip does nothing (R113).
    if (workspace === focused) return;
    does.go(inFront, workspace, step);
  };

  // The keys, claimed wherever the keyboard is, before a terminal hears them; never behind
  // something modal (the palette, a dialog), whose keys they are then.
  const read = workspaces !== undefined;
  const latest = useRef({ goTo, pins, held, railShown, mac, read, open });
  useLayoutEffect(() => {
    latest.current = { goTo, pins, held, railShown, mac, read, open };
  });
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const now = latest.current;
      // The keys and their context rules are the registry's (#2921): a modal that is up and the
      // palette keep them; off a Mac a pin's Ctrl chord is a chat's while it has the keyboard; the
      // popover's key is taken from a chat on purpose.
      const hit = pressed(e, ["workspaces.open", "workspace.pin"], {
        mac: now.mac,
        palette: now.held === true,
      });
      if (hit === undefined) return;
      if (hit.id === "workspaces.open") {
        e.preventDefault();
        e.stopPropagation();
        if (e.repeat) return;
        // The workspace popover stands under the ⌄, which is drawn once the plane is read.
        if (now.railShown && !now.read) return;
        if (now.open !== undefined) {
          setOpen(undefined);
          return;
        }
        openedFrom.current = document.activeElement;
        setOpen(now.railShown ? "workspaces" : "projects");
        return;
      }
      const place = pinOfKey(e, now.mac);
      if (place === undefined) return;
      const to = now.pins[place - 1];
      if (to === undefined) return;
      e.preventDefault();
      e.stopPropagation();
      now.goTo(to);
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, []);

  const focusedNow = useRef(focused);
  useLayoutEffect(() => {
    focusedNow.current = focused;
  });
  // **The swipe** (R112): one step per gesture, over the title bar only.
  useEffect(() => {
    const swipe = new Swipe();
    const wheel = (e: WheelEvent) => {
      const on = e.target;
      if (!(on instanceof Element) || on.closest('[data-testid="title-bar"]') === null) return;
      const dx = sideways(e);
      if (dx === undefined) return;
      // A strip too long to show whole scrolls itself instead (R103).
      const long = strip.current;
      if (long !== null && long.contains(on) && long.scrollWidth > long.clientWidth + 2) return;
      e.preventDefault();
      const step = swipe.feed(dx, e.timeStamp || performance.now());
      if (step === undefined) return;
      const now = latest.current;
      const to = stepTarget(now.pins, focusedNow.current, step);
      if (to !== undefined) now.goTo(to, step);
    };
    document.addEventListener("wheel", wheel, { passive: false });
    return () => document.removeEventListener("wheel", wheel);
  }, []);

  // **The current chip stays in sight after a step** (R104): smoothly, or at once where nothing
  // moves. Not as the bar is first drawn, where nothing has stepped.
  const steppedFrom = useRef(focused);
  useEffect(() => {
    const from = steppedFrom.current;
    steppedFrom.current = focused;
    if (from === undefined || from === focused) return;
    const chip = strip.current?.querySelector<HTMLElement>(
      '[data-part="workspace"][data-selected]',
    );
    if (chip === null || chip === undefined || typeof chip.scrollIntoView !== "function") return;
    chip.scrollIntoView({
      inline: "nearest",
      block: "nearest",
      behavior: moves() ? "smooth" : "auto",
    });
  }, [focused]);

  if (front === undefined) return null;
  /** Opens or closes `which` from its own opener, minding where the keyboard was. */
  const toggle = (which: Exclude<Open, undefined>) => {
    const closing = open === which;
    if (!closing) openedFrom.current = document.activeElement;
    setOpen(closing ? undefined : which);
  };
  /** A chip with the workspace's own menu around it, where the window gives one. */
  const withMenu = (workspace: string, drawn: ReactNode) =>
    menuOf === undefined ? drawn : menuOf(workspace, drawn);
  const unpinned =
    focused !== undefined && focused !== OUTSIDE && !pins.includes(focused) ? focused : undefined;
  /** The strip's one Tab stop: the chip of the workspace in front, else the first pin, else the
   *  ⌄ (charter-app#189). */
  const stop = unpinned ?? (focused !== undefined && pins.includes(focused) ? focused : pins[0]);
  const chip = (workspace: string, at: number) => {
    const drawn = (
      <WorkspaceChip
        key={workspace}
        name={workspace}
        current={workspace === focused}
        tabIndex={workspace === stop ? 0 : -1}
        waiting={workspaces?.waiting[workspace] ?? 0}
        tooltip={at < 9 ? `${workspace} · ${pinKeySaid(at + 1, mac)}` : workspace}
        onClick={() => goTo(workspace)}
      />
    );
    return <Fragment key={workspace}>{withMenu(workspace, drawn)}</Fragment>;
  };

  return (
    <>
      {railShown ? (
        <TitleBarProject name={front.name} />
      ) : (
        <ProjectCrumb
          name={front.name}
          mark={<SwitcherMark letters={monogramOf(front.name)} colour={tintOf(front.name)} small />}
          ref={crumb}
          open={open === "projects"}
          onPress={() => toggle("projects")}
        />
      )}
      <TitleBarSlash />
      {/* The plane root's chip (#3654): always here, outside the strip so it never scrolls
          away, and with no pin to let go. Its menu is the root's, which has none either. */}
      <Fragment key="root">
        {withMenu(
          OUTSIDE,
          <RootChip
            ref={house}
            name={OUTSIDE_TITLE}
            current={focused === OUTSIDE}
            onPress={() => goTo(OUTSIDE)}
          />,
        )}
      </Fragment>
      {/* Drawn once its plane is read: which workspaces it has is the plane's answer. */}
      {workspaces !== undefined && (
        <PinnedStrip name="Pinned workspaces" ref={strip}>
          {pins.map(chip)}
          {unpinned !== undefined && (
            <Fragment key={`${unpinned}+`}>
              {withMenu(
                unpinned,
                <WorkspaceChip
                  name={unpinned}
                  variant="unpinned"
                  current
                  tabIndex={0}
                  waiting={workspaces?.waiting[unpinned] ?? 0}
                  tooltip={`Pin ${unpinned}`}
                  onClick={() => does.pin(inFront, unpinned, true)}
                />,
              )}
            </Fragment>
          )}
          <AllWorkspaces
            ref={all}
            keys={workspacesKeySaid(mac)}
            open={open === "workspaces"}
            stop={stop === undefined}
            onPress={() => toggle("workspaces")}
          />
        </PinnedStrip>
      )}
      {open === "workspaces" && (
        <HeldPopover
          triggerRef={all}
          isOpen
          onOpenChange={(next) => !next && setOpen(undefined)}
          aria-label="Switch workspace"
          layout="list"
          offset={6}
          data-switcher="workspaces"
        >
          <WorkspacePopover
            project={front}
            keys={workspacesKeyCap(mac)}
            onGo={(workspace) => goTo(workspace)}
            onPin={(workspace, pinned) => does.pin(inFront, workspace, pinned)}
            onCreate={() => {
              setOpen(undefined);
              does.create(inFront);
            }}
            onClose={() => setOpen(undefined)}
          />
        </HeldPopover>
      )}
      {open === "projects" && (
        <HeldPopover
          triggerRef={railShown ? all : crumb}
          isOpen
          onOpenChange={(next) => !next && setOpen(undefined)}
          aria-label="Switch project and workspace"
          layout="list"
          offset={6}
          data-switcher="projects"
        >
          <ProjectSwitcher
            projects={projects}
            inFront={inFront}
            keys={workspacesKeyCap(mac)}
            onGo={(plane, workspace) => {
              setOpen(undefined);
              if (plane === inFront) goTo(workspace);
              else does.go(plane, workspace);
            }}
            onChoose={(plane) => {
              setOpen(undefined);
              if (plane !== inFront) does.choose(plane);
            }}
            onPin={does.pin}
            onClose={() => setOpen(undefined)}
          />
        </HeldPopover>
      )}
    </>
  );
}

/** What a list's keys do: ↑ and ↓ move the chosen row, stopping at the ends (R109). */
function moved(at: number, count: number, key: string): number | undefined {
  if (count === 0) return undefined;
  if (key === "ArrowDown") return Math.min(count - 1, at + 1);
  if (key === "ArrowUp") return Math.max(0, at - 1);
  return undefined;
}

/**
 * **The workspace popover** (R107–R109): 320px, its filter "Go to a workspace in ‹project›…",
 * a row per workspace with its count and its pin, the plane root first, then "+ New
 * workspace…", and the keys in its footer. Typing chooses the first row it finds; ↑ and ↓ stop at
 * the ends; Enter goes; Esc and a press outside close it; a pin's toggle keeps it open.
 */
function WorkspacePopover({
  project,
  keys,
  onGo,
  onPin,
  onCreate,
  onClose,
}: {
  project: SwitchProject;
  keys: string;
  onGo: (workspace: string) => void;
  onPin: (workspace: string, pinned: boolean) => void;
  onCreate: () => void;
  onClose: () => void;
}) {
  const id = useId();
  const [typed, setTyped] = useState("");
  const listed = project.workspaces;
  const names = useMemo(() => {
    const q = typed.trim().toLowerCase();
    const named = (listed?.names ?? []).filter((name) => name.toLowerCase().includes(q));
    const root = q === "" || OUTSIDE_TITLE.toLowerCase().includes(q) ? [OUTSIDE] : [];
    return [...root, ...named];
  }, [listed, typed]);
  const rows = [...names, NEW];
  const [at, setAt] = useState(() => Math.max(0, names.indexOf(listed?.focused ?? "")));
  const chosen = Math.min(at, rows.length - 1);
  const rowId = (row: string) => `${id}-${rows.indexOf(row)}`;
  const go = (row: string) => (row === NEW ? onCreate() : onGo(row));
  const key = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const row = rows[chosen];
      if (row !== undefined && !(row === NEW && project.cannotCreate !== undefined)) go(row);
      return;
    }
    const next = moved(chosen, rows.length, event.key);
    if (next === undefined) return;
    event.preventDefault();
    setAt(next);
  };
  return (
    <>
      <SwitcherFilter
        label={`Go to a workspace in ${project.name}…`}
        value={typed}
        onChange={(value) => {
          setTyped(value);
          // Typing chooses the first row it finds (R109).
          setAt(0);
        }}
        listbox={id}
        active={rows[chosen] === undefined ? undefined : rowId(rows[chosen])}
        keys={keys}
        onKeyDown={key}
      />
      <SwitcherList
        id={id}
        label={`Workspaces in ${project.name}`}
        empty="No workspace matches."
        nothing={names.length === 0}
        pins={rows.map((row) =>
          row === OUTSIDE || row === NEW ? (
            <span key={row} aria-hidden="true" />
          ) : (
            <PinToggle
              key={row}
              name={row}
              pinned={listed?.pins.includes(row) ?? false}
              onChange={(pinned) => onPin(row, pinned)}
            />
          ),
        )}
      >
        {rows.map((row, place) =>
          row === NEW ? (
            <SwitcherRow
              key={row}
              id={rowId(row)}
              name="New workspace…"
              unavailable={project.cannotCreate}
              mark={
                <SwitcherIcon>
                  <Plus />
                </SwitcherIcon>
              }
              active={place === chosen}
              onPress={onCreate}
            />
          ) : (
            <SwitcherRow
              key={row}
              id={rowId(row)}
              name={row === OUTSIDE ? OUTSIDE_TITLE : row}
              mark={row === OUTSIDE ? rootMark : undefined}
              waiting={listed?.waiting[row] ?? 0}
              active={place === chosen}
              onPress={() => onGo(row)}
            />
          ),
        )}
      </SwitcherList>
      <PopoverRule />
      <PopoverFooter />
    </>
  );
}

/** The last row of the workspace popover, which makes one (R108). */
const NEW = "\u0000new";

/**
 * **The two-column switcher** (R94–R100), while the rail is hidden: 560px, its filter "Go to a
 * project or workspace…", the projects on the left with their marks and counts, and the
 * workspaces of the one chosen on the right with their counts and pins.
 *
 * A press on a project only shows its workspaces (R97). The arrows move in the column the
 * keyboard is in, which starts in the projects; → steps across to the workspaces and ← back
 * (audit §7.8: the hint is given its behaviour). Enter on a workspace goes there; on a project,
 * to the workspace it was last on (R81). Typing chooses the first project it finds (R98).
 */
function ProjectSwitcher({
  projects,
  inFront,
  keys,
  onGo,
  onChoose,
  onPin,
  onClose,
}: {
  projects: readonly SwitchProject[];
  inFront: string;
  keys: string;
  onGo: (plane: string, workspace: string) => void;
  onChoose: (plane: string) => void;
  onPin: (plane: string, workspace: string, pinned: boolean) => void;
  onClose: () => void;
}) {
  const id = useId();
  const tintOf = useProjectTints();
  const [typed, setTyped] = useState("");
  const listed = useMemo(
    () => projects.map((one) => ({ name: one.name, workspaces: one.workspaces?.names ?? [] })),
    [projects],
  );
  const found = projectsFound(listed, typed);
  const [chosenPlane, setChosenPlane] = useState(inFront);
  const at = found.findIndex((index) => projects[index].plane === chosenPlane);
  const projectAt = at < 0 ? 0 : at;
  const project = found.length > 0 ? projects[found[projectAt]] : undefined;
  const wsNames = project === undefined ? [] : workspacesFound(listed[found[projectAt]], typed);
  const wsRows =
    project === undefined
      ? []
      : typed.trim() === "" || OUTSIDE_TITLE.toLowerCase().includes(typed.trim().toLowerCase())
        ? [OUTSIDE, ...wsNames]
        : wsNames;
  const [column, setColumn] = useState<"projects" | "workspaces">("projects");
  const [wsAt, setWsAt] = useState(() =>
    Math.max(0, wsRows.indexOf(project?.workspaces?.focused ?? "")),
  );
  const wsChosen = Math.min(wsAt, Math.max(0, wsRows.length - 1));
  const projectRowId = (index: number) => `${id}-p-${index}`;
  const wsRowId = (index: number) => `${id}-w-${index}`;
  const show = (plane: string) => {
    setChosenPlane(plane);
    const next = projects.find((one) => one.plane === plane);
    setWsAt(
      Math.max(
        0,
        [OUTSIDE, ...(next?.workspaces?.names ?? [])].indexOf(next?.workspaces?.focused ?? ""),
      ),
    );
  };
  const key = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    const input = event.currentTarget;
    const atEnd = input.selectionStart === input.value.length;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      if (project === undefined) return;
      if (column === "workspaces" && wsRows[wsChosen] !== undefined)
        onGo(project.plane, wsRows[wsChosen]);
      else onChoose(project.plane);
      return;
    }
    if (event.key === "ArrowRight" && atEnd && column === "projects" && wsRows.length > 0) {
      event.preventDefault();
      setColumn("workspaces");
      return;
    }
    if (event.key === "ArrowLeft" && atEnd && column === "workspaces") {
      event.preventDefault();
      setColumn("projects");
      return;
    }
    if (column === "projects") {
      const next = moved(projectAt, found.length, event.key);
      if (next === undefined) return;
      event.preventDefault();
      show(projects[found[next]].plane);
    } else {
      const next = moved(wsChosen, wsRows.length, event.key);
      if (next === undefined) return;
      event.preventDefault();
      setWsAt(next);
    }
  };
  const active =
    column === "projects"
      ? project === undefined
        ? undefined
        : projectRowId(projectAt)
      : wsRows.length === 0
        ? undefined
        : wsRowId(wsChosen);
  return (
    <>
      <SwitcherFilter
        label="Go to a project or workspace…"
        value={typed}
        onChange={(value) => {
          setTyped(value);
          // Typing chooses the first project it finds (R98), and its first workspace.
          const first = projectsFound(listed, value)[0];
          if (first !== undefined) setChosenPlane(projects[first].plane);
          setWsAt(0);
        }}
        listbox={column === "projects" ? `${id}-p` : `${id}-w`}
        active={active}
        keys={keys}
        onKeyDown={key}
      />
      <SwitcherColumns>
        <SwitcherList id={`${id}-p`} label="Projects" header="Projects" empty="No project matches.">
          {found.map((index, place) => {
            const one = projects[index];
            return (
              <SwitcherRow
                key={one.plane}
                id={projectRowId(place)}
                name={one.name}
                waiting={one.waiting}
                mark={<SwitcherMark letters={monogramOf(one.name)} colour={tintOf(one.name)} />}
                active={place === projectAt}
                // A press only shows its workspaces (R97).
                onPress={() => show(one.plane)}
              />
            );
          })}
        </SwitcherList>
        <SwitcherList
          id={`${id}-w`}
          label={project === undefined ? "Workspaces" : `Workspaces in ${project.name}`}
          header={project === undefined ? "Workspaces" : `Workspaces in ${project.name}`}
          empty="No workspace matches."
          pins={
            project === undefined
              ? undefined
              : wsRows.map((row) =>
                  row === OUTSIDE ? (
                    <span key={row} aria-hidden="true" />
                  ) : (
                    <PinToggle
                      key={row}
                      name={row}
                      pinned={project.workspaces?.pins.includes(row) ?? false}
                      onChange={(pinned) => onPin(project.plane, row, pinned)}
                    />
                  ),
                )
          }
        >
          {project === undefined
            ? []
            : wsRows.map((row, place) => (
                <SwitcherRow
                  key={row}
                  id={wsRowId(place)}
                  name={row === OUTSIDE ? OUTSIDE_TITLE : row}
                  mark={row === OUTSIDE ? rootMark : undefined}
                  waiting={project.workspaces?.waiting[row] ?? 0}
                  // The workspace Enter would go to, while the keyboard is in this column, or
                  // the one the project is on while it is in the other.
                  active={place === wsChosen && column === "workspaces"}
                  onPress={() => onGo(project.plane, row)}
                />
              ))}
        </SwitcherList>
      </SwitcherColumns>
      <PopoverRule />
      <PopoverFooter hints={PROJECT_KEYS} />
    </>
  );
}
