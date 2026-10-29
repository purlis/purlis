// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef, useState, type Ref } from "react";
import { Bell, FolderRoot } from "lucide-react";
import { AT_STATUS, ChipCard, mostUrgent, type ChipNotices } from "./ChipNotices";
import type { WorkspaceState } from "./workspaceState";
import { Health, type DoctorState } from "./Doctor";
import { PinItem } from "./Updates";
import type { FactBadge, PinReport } from "./bindings";
import { ago } from "./ChangesView";
import { StatusBar, type StatusContribution } from "./ui/status/StatusBar";
import { StatusCell, StatusItem } from "./ui/status/StatusItem";
import { hideStatusItem, useHiddenStatusItems } from "./statusBarPrefs";
import { NoticeDot } from "./ui/titlebar/TitleBarWorkspaces";

/**
 * **charter's status line**: one line at the very bottom of the window, under everything.
 *
 * The operator asked for it in as many words — *"project directory in right corner — lets move
 * to bottom, at most bottom of charter new one line bar, and it will be status line - and show
 * general infos"*. The project's path used to sit at the right-hand end of `header.bar`, in a
 * row with the tab strip, the `+`, the split buttons and the region toggles; it is here now,
 * and it is the only place it is drawn.
 *
 * # It is NOT a region, and that is a decision
 *
 * ADR 0038's window is regions — left, centre, right (and a bottom one until #1676) — and
 * `app/src/regions.ts` makes adding one a line in a catalogue and a line in an arrangement.
 * This is deliberately not that, and the reasons are in the shape of the thing rather than in
 * taste:
 *
 * - **Every slot in the arrangement is sized as a percentage of its group.** `SLOTS`,
 *   `CATALOGUE.size`, `slotSize` and `startingSize` are all percentages, because that is what
 *   `react-resizable-panels` takes. A status line is one line of text: its height is a font
 *   and two paddings, and the percentage that happens to equal that at 1080 px is the wrong
 *   one at 4K. There is no honest number to put in the catalogue.
 * - **A region resizes and can be put away; this does neither.** A region arrives with a way
 *   to hide it, which is the right rule for a region and the wrong one for the line that says
 *   where you are and which project you are in. A status line an operator can lose is one they
 *   will lose and then report as a bug. `FourRegions.test.tsx`'s *"cannot be put away, because
 *   it is not a region"* is the guard.
 * - **It is the frame, not a tenant of it.** the title bar sits above the
 *   regions and is not in the arrangement either. The window is chrome, regions, chrome;
 *   this is the bottom half of the chrome, and `RegionFrame` is untouched by it — which is also
 *   why this change moved no JSX inside the frame and added no fourth `Panel` to a live group
 *   (charter-app#141's throw is a thing to stay away from, not a thing to test against).
 *
 * **What that costs, said rather than hidden:** the line cannot be resized, put away, or moved
 * to another side, and nothing remembers anything about it. If it is ever wanted as a region,
 * `regions.ts` is exactly where it goes and nothing here blocks that — the content is already
 * a component taking props, which is the shape a region's content has.
 *
 * # What is on the line, and the rules it is composed by
 *
 * charter's own footer is the starting point rather than an invention:
 * `crates/purlis-core/src/footer.rs` draws `⬢ alpha · todo 3 · pieces 2 1 done · ws 4` as
 * zone 1, *"where I am"*, and it arrived at that over a long time. Three of its rules are
 * taken whole:
 *
 * - **A count lives next to the thing it counts.** The todos are the focused workspace's, so
 *   they sit beside its name; the pieces are that workspace's worktrees, said as `branches N`
 *   because the window says branch for a piece (ADR 0072, #602); `ws N` is how many
 *   others there are; and `N chats running` is the project's, which is why it came here when
 *   the title bar's breadcrumb went (ADR 0054).
 * - **Zero renders NOTHING.** A `todo 0` present every turn is furniture within a day, and a
 *   real `todo 7` in that spot then draws no more attention than the zero did. Presence is the
 *   signal.
 * - **Never a number charter cannot stand behind.** A count charter could not read is dropped,
 *   never rounded to zero — a partial total is a wrong total, and the two cannot be told apart
 *   once they are both a number on a line.
 *
 * **The one duplication, named rather than hidden**, the way ADR 0038 names its own: the
 * focused workspace is on the strip above too. They are not the same statement. The strip is
 * the *selector* — at ADR 0026's ten workspaces it scrolls, and reading it means finding the
 * selected tab among the others; this *states* the answer, in one place that never moves. That
 * is the same relationship charter's footer has to `charter ws list`, and it is the whole
 * reason zone 1 exists.
 *
 * **What is left off, and why.** The `ctx`/`cache` gauges and the usage trend are per-chat and
 * have no renderer ported — ADR 0038 names both as open, and a window-wide gauge would be one
 * conversation's number under fifty. `doctor` is one button here (`Doctor.tsx` argues why the
 * frame and not a region) and the plane's pin is another (`Updates.tsx`). Repos and
 * CI are the Changes view's (the bottom region's until #1676), and a status line that drew
 * them would be a second copy of it.
 *
 * **And the update offer is NOT here any more.** It was, beside the pin, and the two read as
 * one pair of "version facts" — but only one of them is about a project. The pin is
 * `charter version`'s verdict on THIS plane's `[charter] version` (ADR 0030), so two
 * open projects can honestly disagree about it and it belongs on the line that names the
 * project. An update offer is about the app: the same offer whichever project is in front, and
 * the line is drawn once per project, so eight projects meant eight `useUpdates` clients
 * listening for one event. It is on the title bar now — the window's own chrome, drawn once
 * (`TitleBar.tsx`). Moved rather than copied: two surfaces stating one fact is the shape
 * charter refuses, and the duplication this line already carries is named above and argued for.
 *
 * # One bar, two ends (qw357)
 *
 * The operator, 2026-10-13: *"notices and doctor buttons in bottom are not better UI integrated
 * in bottom bar, also project directory is very large and can be just compacted and showed only
 * project name, and on hovering just show directory"*. So the line reads as an editor's status
 * bar does:
 *
 * - **The left end is context, the right end is health.** Which project, which workspace and
 *   what it holds are on the left; Notices, the doctor and a drifting pin are on the right.
 *   The left end is the one that gives way in a narrow window: a count that goes off the end
 *   costs less than the Notices count doing so.
 * - **What can be pressed is a status item** (`ui/status`, #2913): no box until the pointer
 *   reaches it, 24px tall, the family's own hover and focus ring. An icon, a short word, then a
 *   count. What only says something is a status cell: its words.
 * - **It is the status bar's contribution model** (N28, `ui/status/StatusBar.tsx`): each item
 *   says its id, its end and its priority, purlis's own and an extension's badges alike, and the
 *   person can hide any item from the bar's menu and show it again there (`statusBarPrefs.ts`).
 *   The line itself stays: what can be lost is an item, never the line.
 * - **The project is its name**, the last segment of its directory, and the whole directory is
 *   its tooltip, on hover and on keyboard focus.
 *
 * **The region toggles are gone from here**, on the operator's word that they are redundant:
 * each side's activity bar puts its side away on a press of the open view and brings it back
 * on a press of any, ⌘B and ⌥⌘B do the same, and so do the palette's rows (`actions.ts`).
 */
export function StatusLine({
  plane,
  read,
  where,
  workspaces,
  running,
  state,
  alerts,
  doctor,
  onOpenSettings,
  pin,
  badges,
  factNotes,
  notices,
}: {
  /**
   * **The notices that pop from the Notices item** (#3653, spec #3647): a project's or the app's
   * results, the window's one store of them, as the title bar's chips have theirs. Its card while
   * it is up, else its dot. Absent draws neither: a project not in front shows no card.
   */
  notices?: ChipNotices;
  /** The project's root directory — the path that used to sit in the top-right corner. */
  plane: string;
  /**
   * Whether the plane has been read at all.
   *
   * **Separate from {@link where}, because "not yet" and "nowhere" are different claims** and
   * the workspace's name cannot carry both. A plane that holds no workspaces answers
   * perfectly well and leaves the window on none of them; a line that said *"reading the
   * plane…"* under it would be charter waiting for something that has already happened.
   */
  read: boolean;
  /** The workspace the window is on, as it should be read. `undefined` when it is on none. */
  where: string | undefined;
  /** How many workspaces this project has, for `ws N`. `undefined` until the plane has been
   *  read. Zero IS drawn: a plane with no workspaces is a fact, not an absence of one. */
  workspaces: number | undefined;
  /**
   * How many of this project's chats are running ({@link runningIn}), or `undefined` before
   * the project has said what it had open. It was the title bar's third clause until the
   * breadcrumb went (ADR 0054); this line is per-project, and it is where the operator reads
   * the project in front.
   */
  running?: number;
  /** What the core has said about the focused workspace. The counts are read off it and
   *  nothing extra is asked for: `workspacePanels.ts` already makes these calls once for the
   *  three regions, and a status line that asked again would be `git status` per clone a
   *  second time. */
  state: WorkspaceState;
  /**
   * The project's Inbox Notices: how many it lists, and the way to open it (#1695). What stood
   * under the tab strip and what the Alerts drawer listed are Notices in the Inbox now, and
   * this button is how they are seen while the side is put away.
   *
   * `undefined` means no Inbox was wired, which the window always does; a status line drawn on
   * its own says so rather than offering a button that answers a press with nothing.
   */
  alerts?: Alerts;
  /** What the doctor last said about this project, run inside the app (`Doctor.tsx`). Absent
   *  draws no button — a caller that has no doctor to offer offers none. */
  doctor?: DoctorState;
  /** Opens Settings at a group, for a doctor row that names a setting (SE-22). */
  onOpenSettings?: (group: string) => void;
  /** What `charter version` says about this plane's pin, why it could not be read, and a way
   *  to ask again. The item is drawn only when it drifts or was not read. */
  pin?: { pin?: PinReport; trouble?: string; again: () => void };
  /**
   * The badges the extensions on in this project and workspace show here (charter-app#340),
   * read from each one's facts file by the core — `extension_facts`, which never starts a
   * program. Absent or empty draws none.
   */
  badges?: readonly FactBadge[];
  /**
   * Why an extension that should show something here shows nothing — a facts file too big or
   * not JSON, a field its manifest did not declare, an extension that changed since it was
   * approved — in the core's words. Drawn as one quiet count with the sentences as its tooltip,
   * so a badge never just vanishes.
   */
  factNotes?: readonly string[];
}) {
  const todos = todoCount(state);
  const pieces = pieceCount(state);
  const hidden = useHiddenStatusItems();
  // **Context**, on the left: where I am, highest first. It is what the line is for, and the end
  // that gives way when the window is narrow, its last item first. **Health**, on the right: what
  // wants a look, each an item that opens it. That end never gives way.
  const items: StatusContribution[] = [
    {
      id: "purlis.project",
      name: "Project",
      side: "left",
      priority: 100,
      // The hairline that parts the project from the workspace inside it (qw357).
      separated: true,
      item: <ProjectItem plane={plane} />,
    },
    {
      id: "purlis.workspace",
      name: "Workspace",
      side: "left",
      priority: 90,
      item:
        where !== undefined ? (
          <StatusCell icon={WORKSPACE_GLYPH} tone="accent" strong data-testid="status-where">
            {where}
          </StatusCell>
        ) : (
          // The plane answered and the window is on no workspace — a plane that holds none, or
          // one whose workspaces all went away: said, because it is the answer. Or not yet read.
          <StatusCell icon={WORKSPACE_GLYPH} tone="quiet" data-testid="status-where">
            {read ? "no workspace" : "reading the project…"}
          </StatusCell>
        ),
    },
  ];
  if (todos !== undefined)
    items.push({
      id: "purlis.todos",
      name: "Todos",
      side: "left",
      priority: 80,
      item: (
        <StatusCell count={todos} data-testid="status-todos">
          todo
        </StatusCell>
      ),
    });
  if (pieces !== undefined)
    items.push({
      id: "purlis.branches",
      name: "Branches",
      side: "left",
      priority: 70,
      item: (
        <StatusCell count={pieces} data-testid="status-pieces">
          branches
        </StatusCell>
      ),
    });
  if (workspaces !== undefined)
    items.push({
      id: "purlis.workspaces",
      name: "Workspaces",
      side: "left",
      priority: 60,
      item: (
        <StatusCell count={workspaces} data-testid="status-workspaces">
          ws
        </StatusCell>
      ),
    });
  // The line's rule and not the breadcrumb's: zero is dropped, because presence is the signal,
  // and so is a count the project has not settled yet.
  if (running !== undefined && running > 0)
    items.push({
      id: "purlis.running",
      name: "Running chats",
      side: "left",
      // Last on its end: it comes and goes as chats run, and moves nothing when it does.
      priority: 5,
      item: (
        <StatusCell data-testid="status-running">
          {`${running} ${running === 1 ? "chat" : "chats"} running`}
        </StatusCell>
      ),
    });
  // An extension's badges: after purlis's own counts, because they are about where you are too
  // and they are the least of it. Given as data, in the one model purlis's own items are in. A
  // stale one is dimmed whole and says its age, so an old count never reads as a current one.
  for (const badge of badges ?? [])
    items.push({
      id: `extension:${badge.extension}/${badge.id}`,
      name: `${badge.label} (${badge.name})`,
      side: "left",
      priority: 40,
      label: badge.label,
      value: badge.stale ? `${badge.value} · ${ago(badge.age_seconds)}` : badge.value,
      stale: badge.stale,
      tooltip: `${badge.label}, from ${badge.name}'s facts file — ${ago(badge.age_seconds)}`,
      testid: `status-badge-${badge.extension}-${badge.id}`,
    });
  if (factNotes !== undefined && factNotes.length > 0)
    items.push({
      id: "purlis.extension-notes",
      name: "Extension notes",
      side: "left",
      priority: 30,
      item: (
        <StatusCell tone="quiet" data-testid="status-fact-notes" tooltip={factNotes.join("\n")}>
          {`${factNotes.length} extension ${factNotes.length === 1 ? "note" : "notes"}`}
        </StatusCell>
      ),
    });
  items.push({
    id: "purlis.notices",
    name: "Notices",
    side: "right",
    // Outermost: the one item that always stays, so nothing that comes, goes or changes its
    // words on this end ever moves it.
    priority: 10,
    // The one item that always stays: the keyboard always has an item on the bar to open the
    // bar's menu from, and so a way back to every item hidden.
    hideable: false,
    item: <NoticesItem alerts={alerts} notices={notices} project={projectName(plane)} />,
  });
  if (doctor)
    items.push({
      id: "purlis.doctor",
      name: "Doctor",
      side: "right",
      // Innermost: its verdict's words change ("1 warning", "2 warnings"), which moves only what
      // is inside it, never the pin or Notices.
      priority: 50,
      item: <Health doctor={doctor} onOpenSettings={onOpenSettings} />,
    });
  // The pin is drawn only when it drifts or could not be read (`PinItem`).
  if (pin && (pin.pin?.drift === true || pin.trouble !== undefined))
    items.push({
      id: "purlis.pin",
      name: "Pin",
      side: "right",
      // Between the doctor and Notices: a pin that starts to drift never moves Notices.
      priority: 40,
      item: <PinItem pin={pin.pin} trouble={pin.trouble} again={pin.again} />,
    });
  return (
    <StatusBar
      aria-label="Status"
      data-testid="status-line"
      items={items}
      hidden={hidden}
      onHide={hideStatusItem}
    />
  );
}

/** The mark before the workspace's name: purlis's own (`crates/purlis-core/src/footer.rs`). */
const WORKSPACE_GLYPH = <span aria-hidden="true">⬢</span>;

/**
 * A project's name: the last segment of its directory, as its tab on the title bar and a
 * terminal's prompt say it. A root (`/`, `C:\`) has no segment and is said whole.
 */
export function projectName(plane: string): string {
  const segments = plane.split(/[\\/]+/).filter((one) => one !== "");
  return segments.at(-1) ?? plane;
}

/**
 * **The project, as its name** (qw357): the operator's *"project directory is very large and
 * can be just compacted and showed only project name, and on hovering just show directory"*.
 *
 * The whole directory is the window's tooltip (`ui/tooltip`), which opens under the pointer
 * and on keyboard focus alike, so the path is one Tab away and never only a hover's. Words and
 * no control, as a table's shortened heading is (`ControlColumn`, `ui/table/ControlTable.tsx`): it presses
 * nothing, so it is no button. Two projects may share a name; the directory tells them apart,
 * and it is also what `data-directory` hands the scenario specs.
 *
 * A long name is cut with an ellipsis (`.status-project-name`), and is whole in the tooltip's
 * path.
 */
function ProjectItem({ plane }: { plane: string }) {
  return (
    <StatusCell
      tooltip={<code>{plane}</code>}
      strong
      shrinks
      data-testid="status-project"
      data-directory={plane}
      icon={
        <span role="img" aria-label="Project">
          <FolderRoot aria-hidden="true" />
        </span>
      }
    >
      {projectName(plane)}
    </StatusCell>
  );
}

/** What the project's Inbox hands the button. */
export type Alerts = {
  /**
   * How many Notices the Inbox lists (the doctor's own findings left out: its button counts
   * them) — or `undefined` when purlis cannot stand behind a number: this project's alerts are
   * not read yet, or purlis stopped looking for them. **Dropped, never zero**: a partial total
   * is a wrong total, and once it is a number on a line nobody can tell the two apart.
   */
  count: number | undefined;
  /** Opens the Inbox. */
  open: () => void;
};

/**
 * The Notices item, in the states it can be in. A status item (the set's small ghost button),
 * so it is flat on the bar: its bell, its word, then its count.
 *
 * - **A count** — drawn in the badge, because something needs the operator.
 * - **Zero** — the word `Notices` and no badge. The footer's rule: a `0` sitting there every
 *   day is furniture by the end of the week, and then a real `2` in that spot draws no more
 *   attention than the zero did. Presence is the signal. The button's NAME still says `none`,
 *   because a screen reader has no "absent badge" to notice.
 * - **No number purlis can stand behind** — a dash, and the Inbox still opens: it says what
 *   purlis could not read. A button that refused to open then would hide the one explanation
 *   there is.
 * - **No Inbox wired** — disabled and said, rather than a control that answers a press with
 *   nothing. The window always wires one; this is a status line drawn on its own.
 */
function AlertsButton({ alerts, ref }: { alerts?: Alerts; ref?: Ref<HTMLButtonElement> }) {
  if (alerts === undefined) {
    return (
      // Disabled with a hint, so still a Tab stop that does nothing (#1700, #2908): Notices is
      // the item the bar always keeps, the one its menu opens from by the keyboard.
      <StatusItem
        ref={ref}
        data-testid="status-alerts"
        isDisabled
        aria-label="Notices — nothing to open here"
        tooltip="This status line has no Inbox behind it."
        icon={<Bell aria-hidden="true" />}
        count="—"
      >
        Notices
      </StatusItem>
    );
  }
  const { count } = alerts;
  return (
    <StatusItem
      ref={ref}
      data-testid="status-alerts"
      data-count={count ?? "unknown"}
      // There are Notices: the bell takes the colour purlis's footer gives a warning, so the one
      // item on the line that is asking for something looks it. The count's own pill stays the
      // neutral one, because it is words and `state.waiting` is held to 3:1 as a mark.
      tone={count !== undefined && count > 0 ? "waiting" : undefined}
      aria-label={
        count === undefined ? "Notices: not counted" : `Notices: ${count === 0 ? "none" : count}`
      }
      tooltip={
        count === undefined
          ? "purlis could not count this project's Notices — open the Inbox to see why"
          : "Open the Inbox: this project's Notices, and what waits on you"
      }
      onPress={alerts.open}
      icon={<Bell aria-hidden="true" />}
      count={count === undefined ? "—" : count > 0 ? count : undefined}
    >
      Notices
    </StatusItem>
  );
}

/**
 * **The Notices item, and the notices that pop from it** (#3653, spec #3647): a project's or the
 * app's result (a refused pin) is the same card a workspace's notice is on its chip, hung above
 * the item and pointing down at it, over the window, so nothing in the page moves. A press
 * elsewhere hides it and leaves the same dot, on the item's leading corner, whose press brings it
 * back with the keyboard in it; × dismisses one, Esc all of them, here and in the Notices centre.
 * The slot is the item's box: the dot coming or going moves nothing on the bar.
 */
function NoticesItem({
  alerts,
  notices,
  project,
}: {
  alerts?: Alerts;
  notices?: ChipNotices;
  /** The project's name, which names the card. */
  project: string;
}) {
  const item = useRef<HTMLButtonElement>(null);
  /** The dot was pressed: the card takes the keyboard as it comes (WCAG 2.4.3). */
  const [handTo, setHandTo] = useState(false);
  const on = notices?.on.get(AT_STATUS);
  const up = notices !== undefined && on !== undefined && notices.open === AT_STATUS;
  return (
    <span className="status-notices">
      {notices !== undefined && on !== undefined && !up && (
        <NoticeDot
          scope="status-item"
          workspace="the status bar"
          count={on.length}
          kind={mostUrgent(on)}
          // No strip's arrows walk to it: a Tab stop of its own, before the item.
          stop
          onPress={() => {
            setHandTo(true);
            notices.show(AT_STATUS);
          }}
        />
      )}
      <AlertsButton alerts={alerts} ref={item} />
      {up && (
        <ChipCard
          workspace={AT_STATUS}
          named={project}
          notices={on}
          anchor={item}
          does={notices}
          side="top"
          takesKeyboard={handTo}
          onKeyboardTaken={() => setHandTo(false)}
        />
      )}
    </span>
  );
}

/**
 * The focused workspace's open todos, or nothing.
 *
 * Nothing at zero (the footer's rule), nothing before the plane has been read, and nothing
 * when the store refused — the right-hand region draws that refusal in full, and a count here
 * that quietly said `0` would contradict it.
 */
export function todoCount(state: WorkspaceState): number | undefined {
  const panels = state.panels;
  if (panels === undefined || panels.todos_refused !== null) return undefined;
  return panels.todos.length === 0 ? undefined : panels.todos.length;
}

/**
 * How many worktrees the focused workspace has, across every clone in it — or nothing.
 *
 * **Every clone has to have answered.** `worktree_list` runs per clone and comes back per
 * clone, so a total taken while two of five are still listing is a smaller number than the
 * truth, drawn with no mark on it saying so. Zero is dropped for the footer's reason; a
 * partial or refused total is dropped for a stronger one — it would be wrong.
 */
export function pieceCount(state: WorkspaceState): number | undefined {
  const clones = state.panels?.repos;
  if (clones === undefined) return undefined;
  let total = 0;
  for (const clone of clones) {
    const listed = state.pieces[clone];
    if (listed === undefined) return undefined;
    total += listed.length;
  }
  return total === 0 ? undefined : total;
}

/**
 * How many of a project's chats are running, out of the list the quit warning is given.
 *
 * **The existing answer, asked of nothing.** Every project keeps `ending` — its open chats,
 * each with what it is doing — because the quit warning has to list them; the state on each
 * one is `chatState.ts`'s, which is kept current by hooks rather than by polling. Counting
 * here is a filter over a list the project is already holding, so the line costs no command
 * of its own. A second `chat_states` call would be fifty questions to learn what it was told.
 *
 * **Running, not open**, which is the distinction charter draws and this operator lives by:
 * he keeps many chats at once and most of them are sitting still. A chat that is waiting for
 * him, one that has finished, one that failed and one that no hook has ever reported are each
 * not running — the waiting ones are counted on the project tab and in the title bar's ✋
 * menu. A number that meant "open" would say 50 all day.
 *
 * `undefined` until the project has settled, which is the core answering what it already had
 * open. Before that an empty `ending` means *not yet*, and reading it as zero would be a
 * number charter cannot stand behind, over a plane that is about to put twenty chats back.
 */
export function runningIn(report?: { ending: { state: string }[]; settled: boolean }) {
  if (report === undefined || !report.settled) return undefined;
  return report.ending.filter((chat) => chat.state === "running").length;
}
