// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type CSSProperties, type ReactNode, useEffect, useMemo, useRef } from "react";
import { QueueRead } from "./QueueRead";
import { closestCenter, DndContext } from "@dnd-kit/core";
import { horizontalListSortingStrategy, SortableContext } from "@dnd-kit/sortable";
import {
  tabRenameId,
  tileAllId,
  workItemSaid,
  RENAMES_ON_F2,
  type Offer,
  type Catalogued,
} from "./actions";
import { Menued } from "./Menus";
import {
  ALONG_THE_STRIP,
  keepsTheFocus,
  SortableTab,
  stripAccessibility,
  useStripSensors,
} from "./sortable";
import { droppedReference, overChat, type ChatsForReferences } from "./references";
import { FreshMark, freshMarkShown } from "./PlaneUpdated";
import { REGION_CHROME } from "./RegionFrame";
import { movedAt, sameList, useChatsSelect, type Chats } from "./chatState";
import {
  byLastActivity,
  chatOf,
  contentsOf,
  panesOf,
  taskShownIn,
  type Pinned,
  type Tabs,
} from "./tabs";
import { TabNeeds } from "./TabChip";
import { type Ended, type Needing } from "./tabTasks";
import { TabCell, tabsPart, type TabVariant } from "./ui/tabs/Tabs";
import { StripTab, TabStripRow, useTabStrip } from "./ui/tabs/TabStrip";
import { isFileView } from "./pieceViews";
import { tornOff } from "./tabWindows";
import { WRAPPING_UP } from "./NeedsYou";
import { NONE_RESTORING, RESTORING } from "./restoringChats";
import { closeOnDelete, renameOnF2 } from "./tabKeys";
import { type ChatRow } from "./chatsTree";
import { STRIP, TAB } from "./paneDrag";
import { showsStopping, TabMarks, type TabFacts } from "./ChatRows";
import { faceOf, type Naming, type TabFace } from "./tabFace";
import type { Ref } from "./fits";
import type { RefObject } from "react";
import { Doer, ShowMore, Pin, Closer } from "./StripMarks";
import { NewTabButton } from "./NewTabButton";
import type { NewTabRow } from "./newTabList";
import { TIPS_BELOW } from "./ui/tooltip/Tooltip";
import { numberedName } from "./chatNumber";

/** What a tab with no tasks hands its chip, the same lists every time. */
export const NO_ROWS: readonly ChatRow[] = [];
export const NO_ENDED: readonly Ended[] = [];

/**
 * **The chat tab `id` shows** (#1486): the task its first pane is switched to, and otherwise
 * the tab's own chat. Nothing for a tab that opened on a view. What a thing dropped on the tab
 * is handed to, since the tab reads as the chat it shows.
 */
function chatShownBy(tabs: Tabs, id: number): number | undefined {
  return taskShownIn(tabs, id) ?? chatOf(tabs, id);
}

/**
 * **What a tab holds, as the tabs family draws it** (#2914): a chat, a view, or one file in an
 * editor, whose tab joins the editor's toolbar under it.
 */
function variantOf(tabs: Tabs, id: number): TabVariant {
  const lead = contentsOf(tabs, id)[0]?.content;
  if (lead?.kind !== "view") return "chat";
  return isFileView(lead.view) ? "file" : "view";
}

/**
 * **A chat tab's tooltip**: the chat it shows by its number and the tab's name, `#641 · steward 4`
 * (#6212), then where a handed-off chat came from and the work item it works on. The number is
 * the chat's current one, read off the pane, so a chat started again says its new number.
 */
function tabTip(
  named: { number: number; name: string } | undefined,
  from: string | undefined,
  workItem: string | undefined,
): string | undefined {
  return tipLines(
    named === undefined ? undefined : numberedName(named.number, named.name),
    from,
    workItem === undefined ? undefined : workItemSaid(workItem),
  );
}

/**
 * **The chat strip** (ADR 0036, DS-6 #1836): the focused workspace's tabs above the panes, each
 * with its fresh mark and `×`, the `+` that opens a chat, and the show-more menu of
 * what the strip is not drawing. Split out of `PlaneView.tsx` (#1035), which owns every value
 * it is handed.
 */
export function ChatStrip({
  by,
  newTabRows,
  found,
  press,
  tabs,
  ofWorkspace,
  colourOf,
  tintOf,
  shown,
  hidden,
  chats,
  chatTabId,
  strip,
  chatControls,
  chatLeast,
  chipWidth,
  dragSensors,
  chatDragWords,
  dragTab,
  tearOff,
  draggingRef,
  draggedAtRef,
  notGoingBack,
  isPinned,
  pressTab,
  referenceChats,
  wrapping,
  stopping,
  shells,
  personaOf,
  factsOf,
  naming,
  pinsIconOnly,
  taskNameOf,
  taskTabOf,
  hiddenIn,
  tasksRunningOf,
  waitingOn,
  showFromChip,
  freshOf,
  freshMarkOf,
  planeUpdates,
  handedFrom,
  workItems,
  restoring = NONE_RESTORING,
}: ChatStripProps) {
  /** Where the pointer is while a tab is carried, for a tear-off (#4768). */
  const pointer = useFollowedPointer();
  /** Tile all on the tab in front, as the catalogue holds it (#4765). */
  const tileAll = tabs.inFront === undefined ? undefined : by(tileAllId(tabs.inFront));
  /** The tab's own Rename… row (#5343): a double-click on it and F2 on it press it, as its menu
   *  does, and its dialog says what it renames. A view's tab has none. */
  const renameRowOf = (id: number) => by(tabRenameId(id));
  // The tabs drawn, as React Aria's tablist knows them (`ui/tabs/TabStrip`).
  const drawn = shown.map(String);
  // What each tab wears (#4762), read once a draw for every tab it is drawn or listed by.
  const faces = useMemo(
    () => new Map([...shown, ...hidden].map((id) => [id, faceOf(tabs, id, naming)])),
    [hidden, naming, shown, tabs],
  );
  const faceAt = (id: number) => faces.get(id) ?? faceOf(tabs, id, naming);
  // The chat a tab shows, by its number and the tab's name: what its hover leads with (#6212).
  const namedChatOf = (id: number) => {
    const number = chatShownBy(tabs, id);
    return number === undefined ? undefined : { number, name: faceAt(id).name };
  };
  const along = useTabStrip({
    keys: drawn,
    selected: tabs.inFront === undefined ? undefined : String(tabs.inFront),
  });
  return (
    <header
      className="bar"
      // The strip the tabs sit in (`ui/tabs`): docked on the content panel under it, or a strip
      // of its own, as the theme says.
      {...tabsPart("strip")}
      // Every tooltip on the strip stands below it, clear of its row (#3361).
      {...TIPS_BELOW}
      {...{ [REGION_CHROME]: "" }}
      data-colour={colourOf(ofWorkspace) ?? undefined}
      style={tintOf(ofWorkspace)}
    >
      {/* The chats of the FOCUSED WORKSPACE (ADR 0036), which is what the tmux frame's
        sessions-under-a-workspace was. Named, because the projects and the workspaces
        above are tablists too and a query for `role="tab"` across the whole window
        would mix all three. */}
      {/* Draggable along the strip (SI-6, `sortable.tsx`): a drop moves a tab within its
        group, and one carried across the pinned boundary pins or unpins it. */}
      <DndContext
        sensors={dragSensors}
        collisionDetection={closestCenter}
        modifiers={ALONG_THE_STRIP}
        accessibility={chatDragWords}
        onDragStart={() => {
          draggingRef.current = true;
          notGoingBack();
          pointer.follow();
        }}
        onDragCancel={() => {
          draggingRef.current = false;
          draggedAtRef.current = performance.now();
          pointer.stop();
        }}
        onDragEnd={({ active, over }) => {
          // The click that ends a drag is the drag's (#1486, `pressTab`).
          draggingRef.current = false;
          draggedAtRef.current = performance.now();
          notGoingBack();
          // **Let go past the window's edge, it is torn off** (#4768) into a tab window.
          const at = pointer.stop();
          if (tornOff(at, { width: window.innerWidth, height: window.innerHeight }))
            tearOff(Number(active.id));
          else if (over) dragTab(Number(active.id), Number(over.id));
        }}
      >
        <SortableContext items={shown.map(String)} strategy={horizontalListSortingStrategy}>
          {/* The arrows, Home and End along the tabs, and the one Tab stop, are React Aria's
            (`ui/tabs/TabStrip`), heard on this row for keys pressed on a tab. Its tablist owns
            the tabs and not the `×`, fresh mark and task chip beside each (#1204). */}
          <TabStripRow
            strip={along}
            name="Tabs"
            ids={shown.map((_, place) => chatTabId(place))}
            className="tabs"
            data-strip="Tabs"
            // A pane or a chat dropped here makes a tab of its own (#4763).
            {...{ [STRIP]: "" }}
            ref={strip}
            style={{ "--least": `${chatLeast}px`, "--chip": `${chipWidth}px` } as CSSProperties}
          >
            {shown.map((id, place) => (
              <SortableTab key={id} id={String(id)}>
                {({ sortable, style }) => (
                  /* Right-click is the third reader of the catalogue (`Menus.tsx`).
                     `asChild`, so the strip gains no wrapper element: the trigger IS the
                     tab.

                     No `data-tab` and no scroll-into-view ref any more: #171 deleted
                     `offscreen.ts` and the strip collapses rather than scrolls, so there is
                     nothing to scroll a tab into and nothing measuring tabs through the
                     markup. */
                  <Menued
                    // The tab's own rows, never a chat's (#4753): a chat's are its pane's ⋯ and
                    // its sidebar row's, so a split tab never acts on the wrong chat.
                    on={{ on: "tab", tab: id }}
                    offers={found}
                    onPress={press}
                  >
                    <TabCell
                      className="tab"
                      ref={sortable.setNodeRef}
                      style={style}
                      variant={variantOf(tabs, id)}
                      // Chosen changes its colour and fill, never its size: no tab moves.
                      selected={id === tabs.inFront}
                      // The first tab starts at the panel's edge, and flares on its right only.
                      first={place === 0}
                      data-dragging={sortable.isDragging || undefined}
                      // Pulled down off the strip, it splits in beside a pane (#4763).
                      {...{ [TAB]: String(id) }}
                      // A pinned tab at its icon only, where pins alone would take over half
                      // the strip; its name is in its tooltip (#4762).
                      data-icon-only={(pinsIconOnly && isPinned(id)) || undefined}
                      // A task's own tab (#1489): what its look is hung on.
                      data-task-tab={taskTabOf(id) === undefined ? undefined : ""}
                      // A file, a folder, lines or a search hit dropped on a chat's tab is
                      // typed into that chat as a reference, unsent (FM-9).
                      // **Into the chat the tab shows** (#1486): the tab reads
                      // `steward 4 › talk`, so what is dropped on it goes to talk.
                      onDragOver={(event) => {
                        if (chatShownBy(tabs, id) !== undefined) overChat(event);
                      }}
                      onDrop={(event) => {
                        const session = chatShownBy(tabs, id);
                        const r = droppedReference(event);
                        if (session === undefined || r === undefined) return;
                        event.preventDefault();
                        referenceChats.hand(r, session, "add");
                      }}
                      // A chat being smart-closed wears the closing look (ADR 0064).
                      data-wrapping-up={
                        panesOf(tabs, id).some((one) => wrapping.has(one.session)) || undefined
                      }
                      // A chat being stopped wears the stop's look (#1459).
                      data-stopping={showsStopping(tabs, id, stopping) || undefined}
                      // A chat the launch has still to start (`restoring.rs`).
                      data-restoring={
                        panesOf(tabs, id).some((one) => restoring.has(one.session)) || undefined
                      }
                    >
                      <StripTab
                        strip={along}
                        tabKey={String(id)}
                        selected={id === tabs.inFront}
                        tip={
                          panesOf(tabs, id).some((one) => wrapping.has(one.session))
                            ? WRAPPING_UP
                            : panesOf(tabs, id).some((one) => restoring.has(one.session))
                              ? RESTORING
                              : // A split lists its panes on hover, in reading order (#4762),
                                // and a tab says how many of its tasks run (#5341).
                                tipLines(
                                  splitTip(faceAt(id)) ??
                                    tabTip(
                                      namedChatOf(id),
                                      handedFrom[chatOf(tabs, id) ?? -1],
                                      workItems[chatOf(tabs, id) ?? -1],
                                    ),
                                  runningSaid(tasksRunningOf(id)),
                                )
                        }
                        id={chatTabId(place)}
                        // What a press does that the tab's name does not say (#1486):
                        // in front and showing a task, it goes back to the session.
                        aria-description={
                          id === tabs.inFront && taskNameOf(id) !== undefined
                            ? `Press to go back to ${tabs.byId[id].name}`
                            : undefined
                        }
                        // The fresh mark beside it as well (#1246): it is outside the tab.
                        aria-describedby={
                          [
                            sortable.attributes["aria-describedby"],
                            freshMarkShown(freshOf(id), planeUpdates[chatOf(tabs, id) ?? -1])
                              ? freshMarkOf(id)
                              : undefined,
                          ]
                            .filter(Boolean)
                            .join(" ") || undefined
                        }
                        // F2 renames here rather than opening the palette (`RENAMES_ON_F2`), on a
                        // tab that has a rename row, and never a view's.
                        {...(renameRowOf(id) ? { [RENAMES_ON_F2]: "" } : {})}
                        {...sortable.listeners}
                        onKeyDown={(event) => {
                          // A tab that is up is being carried: its keys are the drag's.
                          keepsTheFocus(event, sortable.isDragging);
                          sortable.listeners?.onKeyDown?.(event);
                          if (sortable.isDragging) return;
                          closeOnDelete(event, by(`tab.close:${id}`), press);
                          renameOnF2(event, renameRowOf(id), press);
                        }}
                        // The catalogue's row, not a second copy of it. The tab already in front
                        // has a row that says so and cannot run — a tab is never disabled, because
                        // the selected tab is the one a keyboard has to be able to land on.
                        //
                        // **Pressed while it is in front, a tab showing a task goes
                        // back to its session's own chat** (#1486, `pressTab`).
                        onClick={(event) => pressTab(id, by(`tab.select:${id}`), event.detail > 0)}
                        // A double-click on the tab opens its Rename — the same row again.
                        onDoubleClick={() => {
                          // Not a press to go back to the main chat (#1486).
                          notGoingBack();
                          // …and keeps a preview tab, VS Code's double-click (SI-9b).
                          const offer = renameRowOf(id) ?? by(`tab.keep:${id}`);
                          if (offer?.available) press(offer);
                        }}
                      >
                        {/* No `updates` here: on the strip the plane-updated mark
                                  is a button of its own beside the tab (`FreshMark`). */}
                        <TabMarks
                          tabs={tabs}
                          id={id}
                          face={faceAt(id)}
                          personaOf={personaOf}
                          factsOf={factsOf}
                          shells={shells}
                          wrapping={wrapping}
                          stopping={stopping}
                          task={taskNameOf(id)}
                          taskOf={taskTabOf(id)}
                          pin={<Pin held={isPinned(id)} what="tab" />}
                        />
                      </StripTab>
                      {/* **A chat of the tab waits for you off screen** (V100-37): the hand,
                            and a press goes to the one that has waited longest. All a tab draws
                            of its tasks (#5341): their counts and their menu are on the chat's
                            pane head. Beside the tab's button: a button holds no button. */}
                      <QueueRead>
                        {(queue) => <TabNeeds needs={hiddenIn(queue, id)} onShow={showFromChip} />}
                      </QueueRead>
                      {/* **No skill run and no browsers on a tab** (#5524): a tab says which
                            chat this is and whether it needs you (#5339). What its chat uses now
                            is on its pane head (`usingOn`), and its browsers are rows under it in
                            the Chats list too. */}
                      {/* **The fresh mark stays on the tab** (#5524, recorded): the ruling on
                            charter#369 put it here, so the person, who alone can start the chat
                            fresh, is told where they look for whether a chat needs them. Moving
                            it is a change to that ruling, not a cleanup. */}
                      <FreshMark
                        id={freshMarkOf(id)}
                        // **Never on a task's own tab** (#1489): nothing drawn on
                        // that tab ends the task's program, and a fresh start does.
                        offer={freshOf(id)}
                        files={planeUpdates[chatOf(tabs, id) ?? -1]}
                        onPress={press}
                      />
                      <Closer
                        offer={by(`tab.close:${id}`)}
                        onPress={press}
                        reachable={id === tabs.inFront}
                      />
                    </TabCell>
                  </Menued>
                )}
              </SortableTab>
            ))}
            {/* **The `+`, right after the last tab** — Chrome's, and the operator's
                (D6, DS-7 #1839): *"the `+` is an inline tab after the last tab"*. Its
                accessible name is the catalogue's `New tab`, which is what a screen
                reader reads and what `pressOnly("New tab")` finds; it is no `role="tab"`
                and the tablist does not own it (#1204), the project strip's shape.

                **Inside the strip and still never collapsed** (charter-app#130/#131):
                `useRoom` takes this element's width off the room before `fitting`
                shares the rest among the tabs, so at fifty chats it is drawn after the
                last tab that fits, and the strip does not scroll. Not a roving item, so
                the arrows pass over it and Tab stops on it after the selected tab.

                **`New tab` stays on the strip and did not move onto a pane**, where the
                splits and the close went: it acts on the strip, and there is one. **It
                opens a tab with one empty pane** (#4758; spec story 13), which New chat
                then fills, and **a rest on it or Down shows the list of what else opens in
                a new tab** (#5292), from the catalogue (`NewTabButton`). */}
            <span className="strip-doing" ref={chatControls}>
              <NewTabButton
                offer={by("tab.new")}
                rows={newTabRows}
                onPress={press}
                dragging={() => draggingRef.current}
              />
            </span>
          </TabStripRow>
        </SortableContext>
      </DndContext>
      {/* The affordance that says the strip is not showing everything (ADR 0039). It is
        the first thing on the strip that says how many tabs there are past the edge —
        a scroller never did, which is the premise ADR 0036 was missing. It is absent
        when nothing is hidden, because then there is nothing for it to say.

        **Outside the strip, at the right end of the bar** (D6): a control that appears
        exactly when the strip is full must not live inside the thing that is full
        (charter-app#130/#131). It appears only once the tabs fill the strip, so it
        then sits beside the `+`. */}
      <div className="more">
        <HiddenTabs hidden={hidden} tabs={tabs} chats={chats}>
          {(notShowing) => (
            <QueueRead>
              {(queue) => (
                <ShowMore
                  noun="tab"
                  hidden={notShowing.map((id) => ({
                    key: String(id),
                    offer: by(`tab.select:${id}`),
                    needs: waitingOn(queue, id),
                    children: (
                      <TabMarks
                        tabs={tabs}
                        id={id}
                        face={faceAt(id)}
                        personaOf={personaOf}
                        factsOf={factsOf}
                        updates={planeUpdates}
                        shells={shells}
                        wrapping={wrapping}
                        stopping={stopping}
                        task={taskNameOf(id)}
                        taskOf={taskTabOf(id)}
                        needs={hiddenIn(queue, id).map((chat) => chat.name)}
                      />
                    ),
                  }))}
                  onPress={press}
                />
              )}
            </QueueRead>
          )}
        </HiddenTabs>
      </div>
      {/* **Tile all, at the strip's right end** (#4765): the tab in front's panes evened into a
        grid. **Drawn only while it can run**, as the show-more is only while something is
        hidden: at one pane it has nothing to say, and a button there would take room from the
        tabs on every strip that is not split. The tab menu and the palette keep its row,
        greyed with its reason. */}
      {tileAll?.available === true && (
        <Doer offer={tileAll} onPress={press} words="Tile all" iconOnly />
      )}
      {/* **`Split right`, `Split down` and `End this pane's chat` were here and are on the
        panes now** — the operator: *"harnesses panes should each have close button and
        spliting buttons in pane right top corner … so this will fully replace separate
        buttons Split right, Split left, Exit this pane's chat buttons, and this will be
        clear for spliting — user will know what pane is spliting."*

        The argument is the one he gives. All three act on THE FOCUSED PANE, and with a
        window split four ways the bar gives no sign of which that is: the operator reads
        the layout, works out where the keyboard went last, and presses a button somewhere
        else entirely. A control on the pane names its own target.

        They are still catalogue rows and still in the palette, which is what a keyboard
        without a pointer uses: the palette acts on the focused pane, and a pane's own
        button focuses that pane before it runs the same row. */}
      {/* **No region toggles here, and none on the status line either** (qw357): a side is put
        away and brought back from its activity bar, its key and the palette.

        The project's path is not here either, for the same reason and since #172. */}
    </header>
  );
}

/** A tab's hover, line by line: what it has to say, nothing where it has nothing. */
function tipLines(...lines: (string | undefined)[]): string | undefined {
  const said = lines.filter((line): line is string => line !== undefined);
  return said.length === 0 ? undefined : said.join("\n");
}

/** **How many of a tab's tasks run, on its hover** (#5341): the count it no longer wears. */
export function runningSaid(running: number): string | undefined {
  return running === 0 ? undefined : `${running} ${running === 1 ? "task" : "tasks"} running`;
}

/** What a split tab says on hover: every pane, in reading order. Nothing for one pane. */
function splitTip(face: TabFace): string | undefined {
  // How many panes, then which (#5342): the picture after the name says the shape, and hovering
  // says the rest.
  return face.picture !== undefined
    ? `${face.picture.panes} panes: ${face.panes.join(" · ")}`
    : undefined;
}

/**
 * **Where the pointer is while a tab is carried** (#4768): `dnd-kit`'s own delta is held to the
 * strip's axis (`ALONG_THE_STRIP`), so it cannot say whether a tab was let go past the window's
 * edge. The pointer is followed from the pick-up to the drop, and let go of with it.
 */
function useFollowedPointer() {
  const at = useRef<{ x: number; y: number }>(undefined);
  const heard = useRef<((event: PointerEvent) => void) | undefined>(undefined);
  const followed = useMemo(
    () => ({
      follow: () => {
        followed.stop();
        const seen = (event: PointerEvent) => {
          at.current = { x: event.clientX, y: event.clientY };
        };
        heard.current = seen;
        window.addEventListener("pointermove", seen, true);
      },
      stop: () => {
        if (heard.current) window.removeEventListener("pointermove", heard.current, true);
        heard.current = undefined;
        const was = at.current;
        at.current = undefined;
        return was;
      },
    }),
    [],
  );
  useEffect(() => () => void followed.stop(), [followed]);
  return followed;
}

/** Each is `PlaneView`'s own value of the same name: the strip reads them and holds none. */
type ChatStripProps = {
  by: (id: string) => Offer | undefined;
  /** The `+` list's rows, from the catalogue (#5292). */
  newTabRows: readonly NewTabRow[];
  found: Catalogued;
  press: (offer: Offer) => void;
  tabs: Tabs;
  ofWorkspace: string | undefined;
  colourOf: (workspace: string | undefined) => string | null;
  tintOf: (workspace: string | undefined) => CSSProperties;
  shown: readonly number[];
  /** The tabs the strip has no room for, in the strip's order. The show-more menu orders them. */
  hidden: readonly number[];
  /** What the plane's chats are doing, which `HiddenTabs` reads the menu's order from. */
  chats: Chats;
  chatTabId: (at: number) => string;
  strip: Ref;
  chatControls: Ref;
  chatLeast: number;
  chipWidth: number;
  dragSensors: ReturnType<typeof useStripSensors>;
  chatDragWords: ReturnType<typeof stripAccessibility>;
  dragTab: (moved: number, onto: number) => void;
  /** **A tab let go past the window's edge** (#4768): torn off into a window of its own. */
  tearOff: (tab: number) => void;
  /** Whether a tab is being carried, for `pressTab` to tell a drag's click from a press. */
  draggingRef: RefObject<boolean>;
  /** When the last drag ended, for the same. */
  draggedAtRef: RefObject<number>;
  notGoingBack: () => void;
  isPinned: Pinned;
  pressTab: (id: number, select: Offer | undefined, byPointer: boolean) => void;
  referenceChats: ChatsForReferences;
  wrapping: ReadonlySet<number>;
  stopping: ReadonlySet<number>;
  shells: ReadonlySet<number>;
  personaOf: (session: number | undefined) => string | null;
  /** What each pane is called, for what a tab wears (`tabFace.ts`, #4762). */
  naming: Naming;
  /** Whether the pins go icon-only: alone at their floor they would take over half the strip
   *  (`fits.pinnedFirst`, #4762). */
  pinsIconOnly: boolean;
  /** What chat `session`'s row reads its state from beside the board, so its tab says the
   *  row's word (#4935). */
  factsOf?: (session: number) => TabFacts | undefined;
  taskNameOf: (id: number) => string | undefined;
  taskTabOf: (id: number) => { asker: string | undefined } | undefined;
  hiddenIn: (queue: readonly number[], id: number) => readonly Needing[];
  /** How many of tab `id`'s tasks are open: what its hover says (#5341). */
  tasksRunningOf: (id: number) => number;
  waitingOn: (queue: readonly number[], id: number) => number;
  showFromChip: (session: number) => void;
  freshOf: (id: number) => Offer | undefined;
  freshMarkOf: (tab: number) => string;
  planeUpdates: Readonly<Record<number, readonly string[]>>;
  handedFrom: Record<number, string>;
  workItems: Record<number, string>;
  /** The chats the launch has still to start (`restoring.rs`): each one's tab says so. */
  restoring?: ReadonlySet<number>;
};

/**
 * What the show-more menu lists: the tabs the strip has no room for, **most recently moved
 * first** — and nothing else.
 *
 * **The menu is not a find surface** (ADR 0039). It lists what the strip is hiding, not every
 * chat: the palette lists every chat with a search and a ranking over it, it is better at
 * finding than any menu will be, and a menu built as a second one of those is a menu that should
 * not have been built.
 *
 * **And now it is the only pointer route to a hidden tab, which is what the amendment to ADR
 * 0039 had to argue for.** Its rows bring a tab forward, and the tab it brings forward is drawn
 * on the strip with its own `×` (`fits.ts`, the selected tab is always drawn). So ending a chat
 * is still two presses and never one from a menu under the cursor.
 *
 * **The order is read here, where the menu is drawn, and nowhere above it** (#4726). When each
 * chat last moved is the CORE's count (ADR 0039), which comes down on `chat-moved` and in the
 * first snapshot, so two windows on one plane agree. A busy chat behind the edge changes it on
 * nearly every move: read at the top of the project view, each of those drew the whole view
 * again. Read here, a move draws this menu and its button, and the strip keeps only how many it
 * hides, which a move does not change. And redrawn only when the order changes: a move by a
 * chat the strip is drawing changes nothing in a menu of the ones it is not.
 */
function HiddenTabs({
  hidden,
  tabs,
  chats,
  children,
}: {
  hidden: readonly number[];
  tabs: Tabs;
  chats: Chats;
  children: (notShowing: readonly number[]) => ReactNode;
}) {
  const notShowing = useChatsSelect(
    chats,
    (states) => byLastActivity(hidden, tabs, (session) => movedAt(states, session)),
    sameList,
  );
  return children(notShowing);
}
