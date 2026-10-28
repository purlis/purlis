// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useHeldStill, useHeldWhile, type HeldBy } from "./heldStill";
import { useLentMemoryStores } from "./MemoryEdits";
import { useReadOnlySaid } from "./ReadOnlyLine";
import {
  Archive,
  Circle,
  CircleDashed,
  FileText,
  FolderGit2,
  GitBranch,
  KeyRound,
  LoaderCircle,
  Plus,
  Send,
  TriangleAlert,
  UserRound,
  type LucideIcon,
} from "lucide-react";
import { catalogueMenu } from "./Menus";
import { useSessionOffers } from "./sessionOffers";
import { PanelList, type CheckOfRow } from "./PanelList";
import { doneSlugOf, todoDoneId, todoReopenId } from "./todos";
import { PersonaAtWork, usePersonasAtWork } from "./ChatRows";
import { HeadingOffer, PanelSection } from "./PanelSection";
import { Vaults, type VaultsSaid } from "./Vaults";
import { Chart, Facts, Table } from "./Views";
import { commands, type PanelView } from "./bindings";
import {
  DISPATCHES_SHOW,
  listedMemoryOffers,
  memoryKeyRun,
  toKeep,
  type Catalogued,
  type Offer,
} from "./actions";
import type { WorkspaceState } from "./workspaceState";
import { isPanelView, panelKeyOf, panelView, type OwnViewId, type ViewId } from "./sideViews";
import type { PanelTab } from "./ActivityBar";
import { Form, TextField } from "./ui/field/Field";
import { Segment, SegmentedControl } from "./ui/choice/Radio";

/**
 * One view of the right region: **what is asking for you** (ADR 0038).
 *
 * # One view at a time (#1678, ADR 0038 as amended 2026-10-10)
 *
 * The region was a stack of every panel. It is an activity bar's views now, one shown at a time:
 * Todos, Memory, Personas, Sessions and Vaults, then each approved extension's panel as a view of
 * its own, so a stranger's panel never crowds purlis's. `RegionFrame` draws the bar and keeps
 * every view mounted; this draws what one view holds — the one panel it is named for, with the
 * same heading, list and menus the stack gave it. {@link attentionPanels} says which extensions'
 * panels there are, which is what puts their tabs on the bar.
 *
 * **Alerts are not here any more, and that is a correction to ADR 0038, not an omission.** It
 * put them on this side, and this side is one project's: it follows the project in front and
 * the workspace focused in it. An alert is about a PLANE — a pin, a front door, a workspace's
 * layout, a plane root being worked in — and the plane that has one is usually not the one on
 * screen. So the window reads every project's alerts, and each project's Inbox lists its own
 * as Notices and names the other projects that have some (`InboxAlerts.tsx`, #1695).
 *
 * # What changed: this file stopped being the panels and became the thing that draws them
 *
 * The workspace's todos and the plane's personas, and **neither is written here**. They are
 * *contributions* — `purlis_core::panel` values produced in `app/src-tauri/src/panels.rs` and
 * drawn by the loop below, through the same seam an extension's declared panel arrives on. This
 * component knows what a panel is; it does not know what a todo is.
 *
 * That is the point, and it is testable rather than aspirational: an approved extension's panel
 * appears in this region with a search box, a bound, a load-more and a card on every row,
 * because `PanelList` gives every list those and this file gives every panel a `PanelList`.
 *
 * **The needs-you queue is not here any more** (charter-app#249). It sat above every panel,
 * because ADR 0038 says nothing in this region may compete with it — and then it left the
 * region altogether, for the title bar: this region is one project's and one workspace's, and
 * the queue is every project's. `NeedsYou.NeedsYouMenu` is where it is.
 */
export function Panels({
  view,
  workspace,
  state,
  offers,
  onPress,
  contributed,
  shownRow,
  onShowRow,
  vaults,
  onAddTodo,
  atRoot = false,
  rootPanels,
  personaChats,
  chatPersonasHere,
}: {
  /** The view this draws: one of the right side's own, or an extension's panel (#1678). */
  view: ViewId;
  /** The focused workspace, whose todos these are. */
  workspace: string | undefined;
  state: WorkspaceState;
  /** The catalogue by id, which is what a row's verb is looked up in. */
  offers: Catalogued;
  onPress: (offer: Offer) => void;
  /** What approved extensions contribute, asked once per window (`extension_panels`) rather
   *  than once per workspace focus — a survey re-hashes every installed extension's directory,
   *  and that is not a cost the 100 ms of a workspace switch can carry. */
  contributed: readonly PanelView[];
  /**
   * The row whose card is open, as `<panel key>/<row key>`.
   *
   * **Held by the window rather than by the row.** It was held there so the palette could open a
   * persona's card from outside this panel (charter-app#174); a persona opens a tab of its own
   * now, and so does a todo (#1214); what is left here is the popover card of a contributed
   * row that has one.
   */
  shownRow: string | undefined;
  onShowRow: (row: string | undefined) => void;
  /** The plane's vaults, drawn under the workspace's panels (`useVaults`, which the window
   *  holds). A vault is the plane's, so they are drawn with no workspace focused too. */
  vaults?: Pick<VaultsSaid, "vaults" | "trouble" | "reload">;
  /**
   * Record a todo in the named workspace, answering the core's refusal or `undefined` (SI-3).
   * The workspace is named on every call, never implied: the box says which workspace it
   * writes to, and it sends that one.
   */
  onAddTodo?: (workspace: string, text: string) => Promise<string | undefined>;
  /**
   * The plane root is focused (SI-1). It is not a workspace — no charter, memory or todos —
   * so this region says so rather than drawing a workspace's panels, and there is no Todos box
   * to type into: a todo typed here would have to land in a workspace nobody chose.
   */
  atRoot?: boolean;
  /** The plane root's own panels — its Sessions (SI-8d) — drawn under the sentence saying what
   *  the plane root is, while it is focused (`sessions.usePlaneRootPanels`); and its Personas,
   *  which are the project's, with no workspace focused too (#1686). */
  rootPanels?: readonly PanelView[];
  /** Each persona's chats, by number (#3341): a persona's row says whether one is at work or
   *  asks for the person. */
  personaChats?: ReadonlyMap<string, readonly number[]>;
  /** The personas of the focused workspace's chats, at work or at rest (#3483): with the ones
   *  its session records name, the Personas view's In this workspace. */
  chatPersonasHere?: ReadonlySet<string>;
}) {
  const { panels, trouble } = state;
  // Each view is still the "for you" side's (ADR 0038), and says whose: the tab names the view,
  // and this names the region and the workspace it is drawing, as the one region did.
  const named = workspace === undefined ? "Attention" : `Attention · ${workspace}`;

  /** Who works in the focused workspace (#3483): a persona one of its session records names,
   *  or one of its chats runs as. No persona lives in a workspace (`personas/` is the plane's),
   *  so this is where each has worked, and none with no workspace focused. */
  const records = panels?.sessions;
  const personasHere = useMemo(
    () =>
      workspace === undefined
        ? NO_PERSONAS
        : new Set([
            ...(records ?? []).flatMap((record) =>
              record.persona === null ? [] : [record.persona],
            ),
            ...(chatPersonasHere ?? NO_PERSONAS),
          ]),
    [workspace, records, chatPersonasHere],
  );

  if (view === "vaults") {
    // A vault is the plane's, so the view is drawn with no workspace focused too.
    return (
      <aside className="panels panels-view ui-text-body" data-view={view} aria-label={named}>
        {vaults !== undefined && <Vaults said={vaults} offers={offers} onPress={onPress} />}
      </aside>
    );
  }

  /** The panel this view shows, by its key: purlis's own by name, an extension's by its own. */
  const key = isPanelView(view) ? panelKeyOf(view) : OWN_PANELS[view];
  /** Every view is drawn on the panel family's view (#3340, #3341): its region is its one
   *  scroll, with no room of its own around it, and the view's top is held at the top of it. */
  const aside = "panels panels-view ui-text-body";
  const all = [...(panels?.contributed ?? []), ...contributed];
  const draw = (panel: PanelView, at: string) => (
    <Contributed
      key={panel.key}
      panel={panel}
      workspace={at}
      onAddTodo={onAddTodo}
      offers={offers}
      onPress={onPress}
      shownRow={shownRow}
      onShowRow={onShowRow}
      personaChats={personaChats}
      personasHere={personasHere}
    />
  );

  if (workspace === undefined) {
    // The plane root is not a workspace: the view says so, and draws the root's own panel where
    // it has one by this view's key — its Sessions (SI-8d). The personas are the project's, so
    // the Personas view draws them there with nothing to explain (#1686) — and with no
    // workspace focused at all, where the other views say none is.
    const own =
      atRoot || view === "personas"
        ? (rootPanels ?? []).find((panel) => panel.key === key)
        : undefined;
    if (own !== undefined && view === "personas")
      return (
        <aside className={aside} data-view={view} aria-label={named}>
          {draw(own, "")}
        </aside>
      );
    return (
      <aside className={aside} data-view={view} aria-label={named}>
        <p className="empty ui-text-secondary">
          {atRoot
            ? "The project root is not a workspace: it has no todos or memory of its own. Chats here look after the project and its workspaces; focus a workspace to see its panels."
            : "No workspace focused."}
        </p>
        {own !== undefined && draw(own, "")}
      </aside>
    );
  }

  const panel = all.find((one) => one.key === key);
  return (
    <aside className={aside} data-view={view} aria-label={named}>
      {trouble && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
      {panels === undefined ? (
        <p className="pending ui-text-secondary">
          <LoaderCircle className="node-icon spinning" />
          Reading the project…
        </p>
      ) : (
        panel !== undefined && draw(panel, workspace)
      )}
    </aside>
  );
}

/** Which of purlis's own panels each of the right side's views shows, by the panel's key. */
const OWN_PANELS: Partial<Record<OwnViewId, string>> = {
  todos: "charter/todos",
  memory: "charter/memory",
  personas: "charter/personas",
  sessions: "charter/sessions",
};

/** The key of the panel one of purlis's own views shows, as the core names it. */
export function ownPanelKey(view: OwnViewId): string | undefined {
  return OWN_PANELS[view];
}

/**
 * **The extensions' panels, each a view of the right side** (#1678): every approved extension's
 * panel the window has, in the order their `order` hints sort them, with the name and mark each
 * declared. After purlis's own views on the bar, and never among them, so an extension that is
 * approved never moves an icon a person's hand already knows — VS Code's rule for a contributed
 * view container. A panel of purlis's own is not one of these: each has its view already.
 */
export function attentionPanels(
  state: WorkspaceState,
  contributed: readonly PanelView[],
): PanelTab[] {
  const own = new Set(Object.values(OWN_PANELS));
  const seen = new Set<string>();
  return [...(state.panels?.contributed ?? []), ...contributed]
    .filter((panel) => !own.has(panel.key) && !seen.has(panel.key) && seen.add(panel.key))
    .sort((a, b) => a.order - b.order)
    .map((panel) => ({
      view: panelView(panel.key),
      name: panel.title,
      mark: MARKS[panel.mark] ?? Circle,
    }));
}

/**
 * What approved extensions contribute, asked once for the window.
 *
 * **After the first frame, never before it**, which is `Extensions.tsx`'s
 * `drawWhatIsInForce` rule for the same reason at the same cost: a survey is a disk read and a
 * fingerprint of every installed extension's whole directory, and charter's own two panels are
 * produced from the plane read the region already waits on. An operator with no extensions pays
 * one command that answers with an empty list.
 *
 * **A refusal is an empty list and not a thrown promise.** The extension record can be
 * unreadable — a missing config home, a record charter will not parse — and every one of those
 * states already means *nothing is in force* (`purlis_core::extension`). The window that
 * reports them is `Extensions.tsx`'s dialog, which is where an operator goes to find out why
 * something is not contributing; a region that refused to draw charter's own panels over a
 * stranger's unreadable record would be the registry's failure taken out on the plane.
 */
export function useContributedPanels(): PanelView[] {
  const [contributed, setContributed] = useState<PanelView[]>([]);
  useEffect(() => {
    let gone = false;
    void commands
      .extensionPanels()
      .then((said) => {
        if (!gone && said.status !== "error") setContributed(said.data ?? []);
      })
      .catch(() => {
        // Nothing: see the docstring. The list stays empty and charter's own panels draw.
      });
    return () => {
      gone = true;
    };
  }, []);
  return contributed;
}

/** Every mark in `purlis_core::panel::Mark`, as the glyph a heading draws. */
const MARKS: Record<string, LucideIcon> = {
  todo: CircleDashed,
  persona: UserRound,
  repo: FolderGit2,
  piece: GitBranch,
  note: FileText,
  trouble: TriangleAlert,
  vault: KeyRound,
  dot: Circle,
};

/**
 * One panel, whoever contributed it.
 *
 * **Nothing in here asks which panel it is.** A persona's row opens the persona's tab because
 * the row carries the catalogue row that does (`persona.show:<name>`), not because anything here
 * knows it is a persona; the context menu is the one place a panel says what its rows are about.
 */
function Contributed({
  panel,
  workspace,
  onAddTodo,
  offers,
  onPress,
  shownRow,
  onShowRow,
  personaChats,
  personasHere,
}: {
  panel: PanelView;
  /** The focused workspace, which charter's todos panel writes to. */
  workspace: string;
  onAddTodo?: (workspace: string, text: string) => Promise<string | undefined>;
  offers: Catalogued;
  onPress: (offer: Offer) => void;
  shownRow: string | undefined;
  onShowRow: (row: string | undefined) => void;
  personaChats?: ReadonlyMap<string, readonly number[]>;
  personasHere: ReadonlySet<string>;
}) {
  const Mark = MARKS[panel.mark] ?? Circle;
  /** The personas with a chat at work or asking: the Personas view's Working now (#3483), read
   *  as each row's mark reads its state, so a row's group and its mark agree. */
  const atWork = usePersonasAtWork(panel.key === PERSONAS ? personaChats : undefined);
  /** **No row moves under the pointer or the keyboard** (#3566, the Chats list's rule V100-47):
   *  while either is in the list, each row keeps the group it had, and moves once both have
   *  left. Its mark says what its chats do now meanwhile; another workspace focused is drawn as
   *  it is at once. */
  const { holding, held } = useHeldStill<HTMLDivElement>(
    [panel.blocks, atWork, personasHere],
    panel.key === PERSONAS,
  );
  /** Each row's group as the hold began. A row the plane added since is no row moving: it is
   *  drawn at once, in the group it has now. */
  const groupsHeld = useHeldWhile(
    holding,
    { atWork, personasHere, rows: rowKeysOf(panel.blocks) },
    workspace,
  );
  const personaGroupOf = (row: { key: string }): PersonaGroup => {
    const by = groupsHeld.rows.has(row.key) ? groupsHeld : { atWork, personasHere };
    return by.atWork.has(row.key)
      ? "Working now"
      : by.personasHere.has(row.key)
        ? "In this workspace"
        : "On the plane";
  };
  const open = shownRow?.startsWith(`${panel.key}/`)
    ? shownRow.slice(panel.key.length + 1)
    : undefined;
  // **A memory row's own rows** (SI-9c): Open, Edit and Delete for each memory this panel
  // lists, by the one function a persona's tab makes them with.
  // A Move row per store (#1190), from the stores the window lends.
  // And the LIVE names, for what a Move into a LIVE workspace's journal says.
  // And why the project is read-only, when it is (#826): those rows are off with its sentence.
  const { stores, live, readOnly } = useLentMemoryStores();
  const memories = useMemo(
    () => listedMemoryOffers(panel.blocks, stores, live, readOnly),
    [panel.blocks, stores, live, readOnly],
  );
  // **A record's Open and Resume** (SI-8d): built beside the panels, not in the catalogue
  // (`sessionOffers.tsx`, #1160), so a record written draws this list and not the whole view.
  const sessions = useSessionOffers();
  const lookUp = (id: string) => memories.get(id) ?? sessions.get(id) ?? offers.get(id);
  /**
   * **A todo's box toggles done** (R131, ADR 0106): ticking an open todo marks it done, and
   * unticking a done one reopens it — the catalogue's rows, so the box, the row's menu and the
   * palette are one verb. A row the catalogue offers no such row for keeps its mark.
   */
  const todoCheck: CheckOfRow | undefined =
    panel.key === TODOS
      ? (row) => {
          const done = doneSlugOf(row.key);
          const offer = offers.get(done === undefined ? todoDoneId(row.key) : todoReopenId(done));
          if (offer === undefined) return undefined;
          return {
            checked: done !== undefined,
            label: offer.title,
            off: offer.available ? undefined : offer.reason,
            onChange: () => onPress(offer),
          };
        }
      : undefined;
  /** Which of its todos the Todos view lists: the open ones first (#3358, the prototype's Q1 A). */
  const [todosShown, setTodosShown] = useState<TodosShown>("open");
  const addTodo =
    panel.key === TODOS ? (
      <>
        {onAddTodo !== undefined && <AddTodo workspace={workspace} onAdd={onAddTodo} />}
        <TodoTabs shown={todosShown} onShow={setTodosShown} />
      </>
    ) : undefined;

  return (
    <PanelSection
      testid={`panel-${named(panel)}`}
      from={panel.from ?? "charter"}
      mark={Mark}
      title={panel.title}
      provenance={panel.from ?? undefined}
      // **Every panel is a view** (#3340, #3341): one scroll, its heading and its field held at
      // the top. The top holds the box that adds one, under the heading and above the rows.
      view
      top={addTodo}
      actions={
        <>
          {/* charter's own panels are about things charter can make (SI-3): the heading's `+`
              is the catalogue's row for one more. By key, as the menus below are — the one
              place this file says what a panel is about. */}
          {panel.key === PERSONAS && (
            <HeadingOffer offer={offers.get("persona.create")} onPress={onPress} />
          )}
          {/* What the project's chats handed to other chats (#1452): the Dispatches tab. */}
          {panel.key === SESSIONS && (
            <HeadingOffer offer={offers.get(DISPATCHES_SHOW)} onPress={onPress} mark={Send} />
          )}
          {/* A new memory in the focused workspace's journal (SI-9c, ADR 0065 Q9). */}
          {panel.key === MEMORY && (
            <HeadingOffer
              offer={offers.get(`memory.new:workspace/${workspace}`)}
              onPress={onPress}
            />
          )}
          {/* And the journal's archive, to read and restore (KN-4). */}
          {panel.key === MEMORY && (
            <HeadingOffer
              offer={offers.get(`memory.archived:workspace/${workspace}`)}
              onPress={onPress}
              mark={Archive}
            />
          )}
        </>
      }
    >
      {panel.blocks.map((block, at) =>
        block.kind === "chart" ? (
          <Chart key={at} chart={block} />
        ) : block.kind === "facts" ? (
          <Facts key={at} facts={block} />
        ) : block.kind === "table" ? (
          <Table key={at} table={block} />
        ) : block.kind === "note" ? (
          <p
            /* By position, which is the one place in this file that is right: a block has no
               identity of its own in the vocabulary, and a panel's block list is fixed for as
               long as the panel is — it arrives whole from one read and is never spliced. */
            key={at}
            className={block.tone === "trouble" ? "trouble" : "note ui-text-secondary"}
            role={block.tone === "trouble" ? "alert" : undefined}
          >
            {block.text}
          </p>
        ) : (
          <HeldList key={at} held={panel.key === PERSONAS ? held : undefined}>
            <PanelList
              rows={
                panel.key === TODOS
                  ? block.rows.filter(todosOf(todosShown))
                  : panel.key === PERSONAS
                    ? byPersonaGroup(block.rows, personaGroupOf)
                    : block.rows
              }
              empty={panel.key === TODOS && todosShown === "done" ? NOTHING_DONE : block.empty}
              label={panel.title}
              testid={`list-${named(panel)}`}
              // One line a row on this side (#1674): the title, the note on hover, and a dated
              // list under its days.
              titleOnly
              // In a view the search is in its top from the first row (the prototype's Q1 A), but
              // the Todos view's top holds its box for adding one, and searches past a page.
              search={panel.key !== TODOS ? "always" : "beyond-a-page"}
              // A memory's and a session's date at the row's end, and no mark that every row
              // would share, as the prototype's Q1 A draws them (#3340).
              dateAtEnd={panel.key === MEMORY || panel.key === SESSIONS}
              markless={panel.key === MEMORY || panel.key === SESSIONS}
              // A list of things named is filtered; a store of things written is searched (Q1 A).
              searchWord={panel.key === PERSONAS ? "Filter" : "Search"}
              // **A persona's row says whether its chats are at work or ask for you** (#3341): the
              // state's own mark at the row's end, the strongest of its chats'.
              stateOf={
                panel.key === PERSONAS
                  ? (row) => <PersonaAtWork sessions={personaChats?.get(row.key) ?? NO_CHATS} />
                  : undefined
              }
              open={open}
              // The empty list's way out (#1156): the catalogue's row, as a row's is.
              offerFor={lookUp}
              onOpen={(key) => onShowRow(key === undefined ? undefined : `${panel.key}/${key}`)}
              onRun={(id, kept) => {
                // **The catalogue's row or nothing.** A row cannot invent a verb, and an id the
                // catalogue has stopped offering runs nothing rather than something else — which
                // is `Doer`'s rule for the bar's buttons, applied to a panel. A double-click keeps
                // what a single click previews (`actions.toKeep`): this list stays on screen, so
                // a second click can land on it (ADR 0065, as built in SI-9b).
                const offer = lookUp(id);
                if (offer) onPress(kept ? toKeep(offer) : offer);
              }}
              checkOf={todoCheck}
              // The open todos under their days, then the done ones (ADR 0106).
              // And the personas under who is at work, who works here, and the rest (#3483).
              groupOf={
                panel.key === TODOS
                  ? todoGroupOf
                  : panel.key === PERSONAS
                    ? personaGroupOf
                    : undefined
              }
              menuOf={(row) =>
                panel.key === TODOS
                  ? /* Open, Mark done, and Forget (SI-3, #1214): the catalogue's rows for this todo
                     in the focused workspace, which is the one this panel is about. A done one's
                     are Reopen, and Delete under the line (ADR 0106). */
                    catalogueMenu(
                      doneSlugOf(row.key) === undefined
                        ? { on: "todo", slug: row.key }
                        : { on: "doneTodo", slug: doneSlugOf(row.key) ?? "" },
                      offers,
                      onPress,
                    )
                  : panel.key === SESSIONS
                    ? /* Open and Resume (SI-8d): the place's rows for this record, whose key is
                       its plane-relative path. */
                      catalogueMenu({ on: "session", path: row.key }, sessions, onPress)
                    : panel.key === MEMORY
                      ? /* Open, Edit | Delete (ADR 0065 Q12): this memory's rows, from the lookup
                         above rather than the catalogue. */
                        catalogueMenu(
                          { on: "memory", key: memoryKeyRun(row.runs) ?? "" },
                          memories,
                          onPress,
                        )
                      : panel.key === PERSONAS && row.key !== SHARED_ROW
                        ? /* Right-click is the third reader of the catalogue (`Menus.tsx`), and on
                           a persona it has exactly one honest row: what the plane says this
                           persona is. */
                          catalogueMenu({ on: "persona", persona: row.key }, offers, onPress)
                        : undefined
              }
            />
          </HeldList>
        ),
      )}
    </PanelSection>
  );
}

/**
 * **A list, and what holds it still** (`heldStill.ts`): the element the pointer and the
 * keyboard are heard on, which draws no box of its own. A list nothing holds is drawn bare.
 * Keyed by its block's position, as the block's list is.
 */
function HeldList({
  held,
  children,
}: {
  held: HeldBy<HTMLDivElement> | undefined;
  children: ReactNode;
}) {
  if (held === undefined) return children;
  return (
    <div className="panel-list-held" {...held}>
      {children}
    </div>
  );
}

/** The keys of every row a panel's lists hold. */
function rowKeysOf(blocks: PanelView["blocks"]): ReadonlySet<string> {
  return new Set(
    blocks.flatMap((block) => (block.kind === "list" ? block.rows : [])).map((row) => row.key),
  );
}

/** No chats: what a persona with none running is handed. */
const NO_CHATS: readonly number[] = [];

/** No personas: who works in no workspace. */
const NO_PERSONAS: ReadonlySet<string> = new Set();

/**
 * **The Personas view's groups** (#3483, the prototype's Q1 A), in the order they are drawn:
 * the personas with a chat at work or asking, then the ones who work in the focused workspace,
 * then the rest of the plane's. The shared store is the plane's, so it is among the rest.
 */
const PERSONA_GROUPS = ["Working now", "In this workspace", "On the plane"] as const;
type PersonaGroup = (typeof PERSONA_GROUPS)[number];

/** `rows` in their groups' order, each group in the order the panel gave its rows. */
function byPersonaGroup<T extends { key: string }>(
  rows: readonly T[],
  groupOf: (row: T) => PersonaGroup,
): T[] {
  return PERSONA_GROUPS.flatMap((group) => rows.filter((row) => groupOf(row) === group));
}

/** charter's own panels, by the key `purlis_core::panel::Panel::key` gives them. */
const TODOS = "charter/todos";

/** Which todos the Todos view lists (#3358): its tabs, Open, Done and All. */
type TodosShown = "open" | "done" | "all";

/** The Todos view's tabs, in the order the prototype's Q1 A draws them. */
const TODO_TABS: readonly { shown: TodosShown; says: string }[] = [
  { shown: "open", says: "Open" },
  { shown: "done", says: "Done" },
  { shown: "all", says: "All" },
];

/** Whether a Todos row is one `shown` lists. */
function todosOf(shown: TodosShown): (row: { key: string }) => boolean {
  return (row) => shown === "all" || (doneSlugOf(row.key) === undefined) === (shown === "open");
}

/** What the Done tab says when no todo is done: the panel's own sentence is about open ones. */
const NOTHING_DONE = {
  headline: "Nothing done yet",
  body: "A todo marked done stays here, struck through, until it is reopened or deleted.",
  offer: null,
};

/**
 * **The Todos view's tabs** (#3358, the prototype's Q1 A): Open, Done and All, under the box that
 * adds one, in the view's top. One of a few views of the same list, so the segmented control: one
 * Tab stop, the arrows between them.
 */
function TodoTabs({ shown, onShow }: { shown: TodosShown; onShow: (to: TodosShown) => void }) {
  return (
    <SegmentedControl
      size="sm"
      className="todo-tabs"
      aria-label="Show the todos that are"
      value={shown}
      onChange={(to) => {
        const one = TODO_TABS.find((tab) => tab.shown === to);
        if (one !== undefined) onShow(one.shown);
      }}
    >
      {TODO_TABS.map((tab) => (
        <Segment key={tab.shown} value={tab.shown}>
          {tab.says}
        </Segment>
      ))}
    </SegmentedControl>
  );
}

/** The group a done todo's row is drawn under, after the days the open ones are under
 *  (ADR 0106); an open todo's stays under its day. */
function todoGroupOf(row: { key: string }): string | undefined {
  return doneSlugOf(row.key) === undefined ? undefined : "Done";
}
const PERSONAS = "charter/personas";
const SESSIONS = "charter/sessions";
const MEMORY = "charter/memory";

/** The Personas panel's row for the shared store (`panels::SHARED_ROW`): not a persona, so it
 *  gets no persona's menu. */
const SHARED_ROW = "_shared";

/**
 * The Todos panel's box: **a todo typed here goes to the focused workspace, and the box says
 * which** (SI-3). Its accessible name and its placeholder both name the workspace, because a todo
 * recorded in the wrong one is a list that lies about what is left in both.
 *
 * The core decides what is recorded (`todo_add`, which is `Workspace::record_todo`): a todo with
 * no words, or one about work already on the list, is refused in its words, drawn under the box,
 * and the box keeps what was typed so the operator can change it. The list itself is redrawn
 * from the plane — the write changes the disk, and the window reads the disk again.
 */
function AddTodo({
  workspace,
  onAdd,
}: {
  workspace: string;
  onAdd: (workspace: string, text: string) => Promise<string | undefined>;
}) {
  const [text, setText] = useState("");
  const [trouble, setTrouble] = useState<string>();
  const [busy, setBusy] = useState(false);
  /** Why the project is read-only, when it is (#826): a todo is committed, so the box is off,
   *  with the sentence as its tooltip. */
  const readOnly = useReadOnlySaid();
  const add = async () => {
    const words = text.trim();
    if (words === "" || busy || readOnly !== undefined) return;
    setBusy(true);
    const refused = await onAdd(workspace, words);
    setBusy(false);
    setTrouble(refused);
    if (refused === undefined) setText("");
  };
  return (
    <>
      <Form
        className="panel-search"
        onSubmit={(event) => {
          event.preventDefault();
          void add();
        }}
      >
        <TextField
          value={text}
          aria-label={`New todo in ${workspace}`}
          // The add field's mark, as the prototype's Q1 A draws it (#3340).
          icon={<Plus aria-hidden="true" />}
          // The refusal's line only while there is one (#3340): the prototype's top has 8px under
          // the box, so no empty line is kept for a refusal that has not come.
          {...(trouble === undefined ? {} : { error: trouble })}
          placeholder="Add a todo…"
          // Read-only rather than disabled while it is sent: a disabled box drops the keyboard,
          // and the next todo is typed into the same box.
          isReadOnly={busy}
          isDisabled={readOnly !== undefined}
          tooltip={readOnly}
          onChange={setText}
        />
      </Form>
    </>
  );
}

/**
 * What a panel is called in a `data-testid`.
 *
 * **charter's own keep the bare name they have always had** — `panel-todos`,
 * `panel-personas` — because a scenario spec is a contract with the window and renaming one to
 * advertise an internal move is a change to what the spec is about. A contributed panel is
 * `panel-ext-<extension>-<id>`, which cannot collide with a built-in's because `ext` is not a
 * legal panel id (an id starts with a letter or a digit and `charter/` is stripped, so the one
 * name this could clash with is a built-in panel called `ext-…`, and charter has none).
 */
function named(panel: PanelView): string {
  return panel.key
    .replace(/^charter\//, "")
    .split("/")
    .join("-");
}
