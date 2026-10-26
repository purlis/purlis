// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type CSSProperties } from "react";
import { QueueRead } from "./QueueRead";
import { closestCenter, DndContext } from "@dnd-kit/core";
import { horizontalListSortingStrategy, SortableContext } from "@dnd-kit/sortable";
import { SquareTerminal } from "lucide-react";
import { workItemSaid, RENAMES_ON_F2, type Offer, type Catalogued } from "./actions";
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
import { TabBrowsers, type BrowserShown } from "./ChatBrowsersShown";
import { REGION_CHROME } from "./RegionFrame";
import {
  chatOf,
  contentsOf,
  panesOf,
  shownIn,
  taskShownIn,
  type Backgrounded,
  type Pinned,
  type Tabs,
} from "./tabs";
import { TabTasks, type Place as TaskPlace } from "./TabChip";
import { SkillRunChip } from "./SkillRunChip";
import { type Ended, type Needing } from "./tabTasks";
import { TabCell, tabsPart, type TabVariant } from "./ui/tabs/Tabs";
import { StripTab, TabStripRow, useTabStrip } from "./ui/tabs/TabStrip";
import { isFileView } from "./pieceViews";
import { chipSays, WRAPPING_UP, WrappingUp } from "./NeedsYou";
import { NONE_RESTORING, RESTORING } from "./restoringChats";
import { closeOnDelete, renameOnF2 } from "./tabKeys";
import { TabRename } from "./TabRename";
import { type ChatRow } from "./chatsTree";
import { showsStopping, TabMarks } from "./ChatRows";
import type { Ref } from "./fits";
import type { StateClock } from "./stateClock";
import type { UsedReader } from "./tasksUsed";
import type { RefObject } from "react";
import { Doer, ShowMore, Pin, Closer } from "./StripMarks";
import { TIPS_BELOW } from "./ui/tooltip/Tooltip";
import type { SandboxSaid } from "./SandboxMark";

/** What a tab with no tasks hands its chip, the same lists every time. */
export const NO_ROWS: readonly ChatRow[] = [];
export const NO_ENDED: readonly Ended[] = [];

/**
 * **The chat tab `id` shows** (#1486): the task its session's tab is switched to, and otherwise
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

/** A chat tab's tooltip: where a handed-off chat came from, and the work item it works on. */
function tabTip(from: string | undefined, workItem: string | undefined): string | undefined {
  const lines = [from, workItem === undefined ? undefined : workItemSaid(workItem)].filter(
    (line): line is string => line !== undefined,
  );
  return lines.length === 0 ? undefined : lines.join("\n");
}

/**
 * **The chat strip** (ADR 0036, DS-6 #1836): the focused workspace's tabs above the panes, each
 * with its task chip, fresh mark and `×`, the `+` that opens a chat, and the show-more menu of
 * what the strip is not drawing. Split out of `PlaneView.tsx` (#1035), which owns every value
 * it is handed.
 */
export function ChatStrip({
  by,
  found,
  press,
  tabs,
  ofWorkspace,
  colourOf,
  tintOf,
  shown,
  notShowing,
  chatTabId,
  strip,
  chatControls,
  chatLeast,
  chipWidth,
  dragSensors,
  chatDragWords,
  dragTab,
  draggingRef,
  draggedAtRef,
  notGoingBack,
  isBackground,
  isPinned,
  isDragging,
  nameBoxFor,
  saveName,
  endRename,
  pressTab,
  referenceChats,
  wrapping,
  stopping,
  shells,
  personaOf,
  sandboxOf,
  taskNameOf,
  taskTabOf,
  tabHasTasks,
  showTabTasks,
  tasksIdOf,
  tasksAsked,
  chatsByTab,
  endedByTab,
  runningOfTab,
  chatClock,
  hiddenIn,
  waitingOn,
  showFromChip,
  taskEndsOf,
  pressTaskEnd,
  placeFromChip,
  usedFromChip,
  stopAllOf,
  activityFromChip,
  freshOf,
  freshMarkOf,
  planeUpdates,
  handedFrom,
  workItems,
  restoring = NONE_RESTORING,
  browsersOfTab,
  openBrowser,
  openSkillRun,
}: ChatStripProps) {
  // The tabs drawn, as React Aria's tablist knows them (`ui/tabs/TabStrip`): not a tab being
  // renamed, whose name box is drawn in its place and is not a tab.
  const drawn = shown.filter((id) => !nameBoxFor(id)).map(String);
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
        }}
        onDragCancel={() => {
          draggingRef.current = false;
          draggedAtRef.current = performance.now();
        }}
        onDragEnd={({ active, over }) => {
          // The click that ends a drag is the drag's (#1486, `pressTab`).
          draggingRef.current = false;
          draggedAtRef.current = performance.now();
          notGoingBack();
          if (over) dragTab(Number(active.id), Number(over.id));
        }}
      >
        <SortableContext items={shown.map(String)} strategy={horizontalListSortingStrategy}>
          {/* The arrows, Home and End along the tabs, and the one Tab stop, are React Aria's
            (`ui/tabs/TabStrip`), heard on this row for keys pressed on a tab. Its tablist owns
            the tabs and not the `×`, fresh mark and task chip beside each (#1204). Not a tab
            being renamed: its name box is drawn in its place, and is not a tab. */}
          <TabStripRow
            strip={along}
            name="Tabs"
            ids={shown.flatMap((id, place) => (nameBoxFor(id) ? [] : [chatTabId(place)]))}
            className="tabs"
            data-strip="Tabs"
            ref={strip}
            style={{ "--least": `${chatLeast}px`, "--chip": `${chipWidth}px` } as CSSProperties}
          >
            {shown.map((id, place) => (
              <SortableTab key={id} id={String(id)} fixed={isBackground(id)}>
                {({ sortable, style }) => (
                  /* Right-click is the third reader of the catalogue (`Menus.tsx`).
                     `asChild`, so the strip gains no wrapper element: the trigger IS the
                     tab.

                     No `data-tab` and no scroll-into-view ref any more: #171 deleted
                     `offscreen.ts` and the strip collapses rather than scrolls, so there is
                     nothing to scroll a tab into and nothing measuring tabs through the
                     markup. */
                  <Menued
                    on={{ on: "chat", tab: id, session: chatOf(tabs, id) }}
                    offers={found}
                    onPress={press}
                  >
                    {isBackground(id) ? (
                      /* **In the background** (SI-8f): the chat is wrapping up, so its tab is a
                         chip — the chat's icon and the breathing mark, its name in the tooltip —
                         at the strip's left edge, and fixed: it is not dragged, and nothing is
                         dropped on it. Pressed, it brings the chat forward to watch it work;
                         it stays a chip while it wraps up. Its menu still has Cancel smart close. */
                      <TabCell
                        className="tab chip"
                        ref={sortable.setNodeRef}
                        style={style}
                        variant="background"
                        selected={id === tabs.inFront}
                        first={place === 0}
                        data-chip=""
                        data-wrapping-up=""
                      >
                        <StripTab
                          strip={along}
                          tabKey={String(id)}
                          selected={id === tabs.inFront}
                          tip={chipSays(tabs.byId[id].name)}
                          id={chatTabId(place)}
                          aria-label={chipSays(tabs.byId[id].name)}
                          onKeyDown={(event) => closeOnDelete(event, by(`tab.close:${id}`), press)}
                          onClick={() => {
                            const offer = by(`tab.select:${id}`);
                            if (offer?.available) press(offer);
                          }}
                        >
                          <SquareTerminal
                            className="tab-mark"
                            data-mark="chat"
                            aria-hidden="true"
                          />
                          <WrappingUp held />
                        </StripTab>
                      </TabCell>
                    ) : (
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
                        {nameBoxFor(id) ? (
                          // The name, open for editing in the tab's place (charter-app#254). Not
                          // inside the tab's button: an input inside a button is two controls in one.
                          <TabRename
                            name={tabs.byId[id].name}
                            onSave={(typed) => saveName(id, typed)}
                            onDone={endRename}
                          />
                        ) : (
                          <StripTab
                            strip={along}
                            tabKey={String(id)}
                            selected={id === tabs.inFront}
                            tip={
                              panesOf(tabs, id).some((one) => wrapping.has(one.session))
                                ? WRAPPING_UP
                                : panesOf(tabs, id).some((one) => restoring.has(one.session))
                                  ? RESTORING
                                  : tabTip(
                                      handedFrom[chatOf(tabs, id) ?? -1],
                                      workItems[chatOf(tabs, id) ?? -1],
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
                            // Down opens the menu of its tasks, where it has any (#1487).
                            aria-haspopup={tabHasTasks(id) ? "menu" : undefined}
                            // The fresh mark beside it as well (#1246): it is outside the tab.
                            aria-describedby={
                              [
                                sortable.attributes["aria-describedby"],
                                freshMarkShown(freshOf(id), planeUpdates[chatOf(tabs, id) ?? -1])
                                  ? freshMarkOf(id)
                                  : undefined,
                                // And its tasks' counts, which are beside it too
                                // (#1487): "Tasks of steward 4: 2 working".
                                tabHasTasks(id) ? tasksIdOf(id) : undefined,
                              ]
                                .filter(Boolean)
                                .join(" ") || undefined
                            }
                            // F2 renames here rather than opening the palette (`RENAMES_ON_F2`), on a
                            // tab that has a rename row — a chat's, and never a view's.
                            {...(by(`tab.rename:${id}`) ? { [RENAMES_ON_F2]: "" } : {})}
                            {...sortable.listeners}
                            onKeyDown={(event) => {
                              // A tab that is up is being carried: its keys are the drag's.
                              keepsTheFocus(event, sortable.isDragging);
                              sortable.listeners?.onKeyDown?.(event);
                              if (sortable.isDragging) return;
                              closeOnDelete(event, by(`tab.close:${id}`), press);
                              renameOnF2(event, by(`tab.rename:${id}`), press);
                              // **Down on a tab with tasks opens their menu** (#1487),
                              // as Down opens the menu under an item of a menu bar. The
                              // strip runs across, so Down was nothing's.
                              if (
                                event.key === "ArrowDown" &&
                                !(event.altKey || event.ctrlKey || event.metaKey) &&
                                !event.shiftKey &&
                                tabHasTasks(id)
                              ) {
                                event.preventDefault();
                                showTabTasks(id);
                              }
                            }}
                            // The catalogue's row, not a second copy of it. The tab already in front
                            // has a row that says so and cannot run — a tab is never disabled, because
                            // the selected tab is the one a keyboard has to be able to land on.
                            //
                            // **Pressed while it is in front, a tab showing a task goes
                            // back to its session's own chat** (#1486, `pressTab`).
                            onClick={(event) =>
                              pressTab(id, by(`tab.select:${id}`), event.detail > 0)
                            }
                            // A double-click on the name renames it — the same row again.
                            onDoubleClick={() => {
                              // Not a press to go back to the main chat (#1486).
                              notGoingBack();
                              // …and keeps a preview tab, VS Code's double-click (SI-9b).
                              const offer = by(`tab.rename:${id}`) ?? by(`tab.keep:${id}`);
                              if (offer?.available) press(offer);
                            }}
                          >
                            {/* No `updates` here: on the strip the plane-updated mark
                                  is a button of its own beside the tab (`FreshMark`). */}
                            <TabMarks
                              tabs={tabs}
                              id={id}
                              persona={personaOf(chatOf(tabs, id))}
                              sandbox={sandboxOf?.(chatOf(tabs, id))}
                              shells={shells}
                              wrapping={wrapping}
                              stopping={stopping}
                              task={taskNameOf(id)}
                              taskOf={taskTabOf(id)}
                              pin={
                                <Pin
                                  held={isPinned(id)}
                                  what={chatOf(tabs, id) === undefined ? "tab" : "chat"}
                                />
                              }
                            />
                          </StripTab>
                        )}
                        {/* **The chip of the session's tasks** (#1487): their counts, the
                            hand where one waits off screen, and the menu to switch
                            between them. Beside the tab's button: a button holds no
                            button. Nothing for a tab with no tasks. */}
                        <QueueRead>
                          {(queue) => (
                            <TabTasks
                              id={tasksIdOf(id)}
                              name={tabs.byId[id].name}
                              rows={chatsByTab.get(id) ?? NO_ROWS}
                              ended={endedByTab.get(id) ?? NO_ENDED}
                              current={
                                shownIn(tabs, id).find((one) => one.pane === tabs.byId[id].focused)
                                  ?.session ?? chatShownBy(tabs, id)
                              }
                              needs={hiddenIn(queue, id)}
                              asked={tasksAsked?.tab === id ? tasksAsked.count : 0}
                              clock={chatClock}
                              dragging={isDragging}
                              onShow={showFromChip}
                              ends={taskEndsOf}
                              onPress={pressTaskEnd}
                              onPlace={placeFromChip}
                              used={usedFromChip}
                              limits={runningOfTab.get(id)}
                              stopAll={stopAllOf}
                              onActivity={activityFromChip}
                            />
                          )}
                        </QueueRead>
                        {/* **The skill run open in the tab's chat** (SK-3): pressed, it
                            opens the skill's tab on that run. Beside the tab's button, as
                            the tasks' chip is. */}
                        {openSkillRun !== undefined && (
                          <SkillRunChip session={chatOf(tabs, id)} onOpen={openSkillRun} />
                        )}
                        {/* **The tab's chats' browsers, live** (#2024): a thumbnail each,
                            pressed to open its view. Beside the tab's button, as the fresh
                            mark is. */}
                        {browsersOfTab !== undefined && openBrowser !== undefined && (
                          <TabBrowsers browsers={browsersOfTab(id)} onOpen={openBrowser} />
                        )}
                        <FreshMark
                          id={freshMarkOf(id)}
                          // **Never on a task's own tab** (#1489): nothing drawn on
                          // that tab ends the task's program, and a fresh start does.
                          offer={freshOf(id)}
                          files={planeUpdates[chatOf(tabs, id) ?? -1]}
                          onPress={press}
                        />
                        <Closer offer={by(`tab.close:${id}`)} onPress={press} />
                      </TabCell>
                    )}
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
                splits and the close went: it acts on the strip, and there is one. */}
            <span className="strip-doing" ref={chatControls}>
              <Doer offer={by("chat.new")} onPress={press} iconOnly />
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
                    persona={personaOf(chatOf(tabs, id))}
                    sandbox={sandboxOf?.(chatOf(tabs, id))}
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
      </div>
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

/** Each is `PlaneView`'s own value of the same name: the strip reads them and holds none. */
type ChatStripProps = {
  by: (id: string) => Offer | undefined;
  found: Catalogued;
  press: (offer: Offer) => void;
  tabs: Tabs;
  ofWorkspace: string | undefined;
  colourOf: (workspace: string | undefined) => string | null;
  tintOf: (workspace: string | undefined) => CSSProperties;
  shown: readonly number[];
  notShowing: readonly number[];
  chatTabId: (at: number) => string;
  strip: Ref;
  chatControls: Ref;
  chatLeast: number;
  chipWidth: number;
  dragSensors: ReturnType<typeof useStripSensors>;
  chatDragWords: ReturnType<typeof stripAccessibility>;
  dragTab: (moved: number, onto: number) => void;
  /** Whether a tab is being carried, for `pressTab` to tell a drag's click from a press. */
  draggingRef: RefObject<boolean>;
  /** When the last drag ended, for the same. */
  draggedAtRef: RefObject<number>;
  notGoingBack: () => void;
  isBackground: Backgrounded;
  isPinned: Pinned;
  isDragging: () => boolean;
  nameBoxFor: (id: number) => boolean;
  saveName: (id: number, typed: string) => Promise<string | undefined>;
  endRename: (back: boolean) => void;
  pressTab: (id: number, select: Offer | undefined, byPointer: boolean) => void;
  referenceChats: ChatsForReferences;
  wrapping: ReadonlySet<number>;
  stopping: ReadonlySet<number>;
  shells: ReadonlySet<number>;
  personaOf: (session: number | undefined) => string | null;
  /** How much chat `session`'s sandbox holds (#2071), as the plane's chats say it. */
  sandboxOf?: (session: number | undefined) => SandboxSaid | null;
  taskNameOf: (id: number) => string | undefined;
  taskTabOf: (id: number) => { asker: string | undefined } | undefined;
  tabHasTasks: (id: number) => boolean;
  showTabTasks: (tab: number) => void;
  tasksIdOf: (tab: number) => string;
  tasksAsked: { tab: number; count: number } | undefined;
  chatsByTab: ReadonlyMap<number, ChatRow[]>;
  endedByTab: ReadonlyMap<number, Ended[]>;
  runningOfTab: ReadonlyMap<number, string>;
  chatClock: StateClock;
  hiddenIn: (queue: readonly number[], id: number) => readonly Needing[];
  waitingOn: (queue: readonly number[], id: number) => number;
  showFromChip: (session: number) => void;
  taskEndsOf: (session: number) => Offer[];
  pressTaskEnd: (offer: Offer) => void;
  placeFromChip: (session: number, where: TaskPlace) => void;
  usedFromChip: UsedReader;
  stopAllOf: (session: number) => Offer | undefined;
  activityFromChip: (session: number) => void;
  freshOf: (id: number) => Offer | undefined;
  freshMarkOf: (tab: number) => string;
  planeUpdates: Readonly<Record<number, readonly string[]>>;
  handedFrom: Record<number, string>;
  workItems: Record<number, string>;
  /** The chats the launch has still to start (`restoring.rs`): each one's tab says so. */
  restoring?: ReadonlySet<number>;
  /** The browsers of tab `id`'s chats (#2024), drawn live beside it. */
  browsersOfTab?: (id: number) => readonly BrowserShown[];
  /** Opens a browser's view, from its thumbnail on a tab. */
  openBrowser?: (shown: BrowserShown) => void;
  /** Opens a skill's tab on one of its runs, from the chip of the run open in a tab's chat. */
  openSkillRun?: (skill: string, run: string) => void;
};
