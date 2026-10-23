// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type CSSProperties, type ReactNode } from "react";
import { QueueRead } from "./QueueRead";
import * as RovingFocusGroup from "@radix-ui/react-roving-focus";
import { closestCenter, DndContext } from "@dnd-kit/core";
import { horizontalListSortingStrategy, SortableContext } from "@dnd-kit/sortable";
import { OUTSIDE, OUTSIDE_TITLE, ROOT_TIP, type Offer, type Catalogued } from "./actions";
import { Menued } from "./Menus";
import {
  ALONG_THE_STRIP,
  keepsTheFocus,
  SortableTab,
  stripAccessibility,
  useStripSensors,
} from "./sortable";
import { StripTablist } from "./StripTablist";
import { useTabStop } from "./roving";
import type { Ref } from "./fits";
import { Doer, ShowMore, Gear } from "./StripMarks";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * **The workspace strip** (ADR 0036), the second of the three strips: one tab per workspace
 * shown, draggable among the pins, the focused one's gear, the `+` that makes one, and the
 * show-more menu of what the strip is not drawing. Split out of `PlaneView.tsx` (#1035), which
 * owns every value it is handed.
 */
export function WorkspaceStrip({
  by,
  found,
  press,
  focused,
  colourOf,
  tintOf,
  workspaceMarks,
  workspacesShown,
  workspacesNotShowing,
  workspaceTabId,
  workspaceStop,
  workspaceStrip,
  workspaceControls,
  workspaceLeast,
  rootWidth,
  workspaceRoot,
  dragSensors,
  workspaceDragWords,
  dragWorkspace,
  waitingIn,
}: WorkspaceStripProps) {
  return (
    <div className="workspaces">
      {/* Draggable along the strip (SI-6, `sortable.tsx`): a drop moves a pinned workspace
          among the pins, and one carried across the boundary pins or unpins it. */}
      <DndContext
        sensors={dragSensors}
        collisionDetection={closestCenter}
        modifiers={ALONG_THE_STRIP}
        accessibility={workspaceDragWords}
        onDragEnd={({ active, over }) => {
          if (over) dragWorkspace(String(active.id), String(over.id));
        }}
      >
        <SortableContext items={workspacesShown.shown} strategy={horizontalListSortingStrategy}>
          <RovingFocusGroup.Root asChild orientation="horizontal" {...workspaceStop}>
            <div
              className="workspaces-strip"
              data-strip="Workspaces"
              ref={workspaceStrip}
              style={
                {
                  "--least": `${workspaceLeast}px`,
                  "--root": `${rootWidth}px`,
                } as CSSProperties
              }
            >
              {/* The tablist, which owns the tabs and not the gear beside one (#1204,
                  `StripTablist.tsx`). */}
              <StripTablist
                name="Workspaces"
                ids={workspacesShown.shown.map((_, place) => workspaceTabId(place))}
              />
              {workspacesShown.shown.map((workspace, place) => {
                const offer = by(`workspace.focus:${workspace}`);
                // **The plane root** (SI-1): first, fixed, an icon with the operator's
                // tooltip, and a menu of its own — it is not a workspace. Its tab has a
                // tab's own padding and is as wide as its icon and its count pill (qw337).
                const root = workspace === OUTSIDE;
                return (
                  <SortableTab key={workspace} id={workspace} fixed={workspace === OUTSIDE}>
                    {({ sortable, style }) => (
                      /* The cell: the tab and, on the focused workspace's, its gear
                         (SE-23). A wrapper, as a project's tab and its `×` have, because a
                         button cannot hold a button. It is what is dragged; it carries the
                         tab's tint too, so the gear sits on the tab's own shade. */
                      <span
                        className="workspace"
                        ref={sortable.setNodeRef}
                        data-dragging={sortable.isDragging || undefined}
                        data-colour={colourOf(workspace) ?? undefined}
                        style={{ ...tintOf(workspace), ...style }}
                      >
                        {/* Right-click is the third reader of the catalogue (`Menus.tsx`):
                            focus, pin, make one, and — under the line — delete this one.
                            `asChild`, and on the tab and not the cell: the context-menu
                            key opens a menu only on the trigger that has the keyboard
                            (`ContextMenu`'s keys, `ui/menu/ContextMenu.tsx`), and that is the tab. */}
                        <Menued
                          on={root ? { on: "root" } : { on: "workspace", workspace }}
                          offers={found}
                          onPress={press}
                        >
                          <RovingFocusGroup.Item
                            asChild
                            tabStopId={workspace}
                            active={workspace === focused}
                          >
                            <Tooltip tip={root ? ROOT_TIP : offer?.title}>
                              <button
                                role="tab"
                                id={workspaceTabId(place)}
                                // The root's own width is measured, and comes off the room the
                                // named workspaces share (`fits.useWidth`, qw337).
                                ref={root ? workspaceRoot : undefined}
                                className={root ? "plane-root" : undefined}
                                aria-label={root ? OUTSIDE_TITLE : undefined}
                                aria-selected={workspace === focused}
                                aria-describedby={
                                  root ? undefined : sortable.attributes["aria-describedby"]
                                }
                                // Its own colour, in front or not (charter-app#281): its shade and its
                                // mark are its tint, set on the tab and the cell around it.
                                data-colour={colourOf(workspace) ?? undefined}
                                style={tintOf(workspace)}
                                {...sortable.listeners}
                                onKeyDown={(event) => {
                                  keepsTheFocus(event, sortable.isDragging);
                                  sortable.listeners?.onKeyDown?.(event);
                                }}
                                onClick={() => {
                                  if (offer?.available) press(offer);
                                }}
                              >
                                {workspaceMarks(workspace)}
                              </button>
                            </Tooltip>
                          </RovingFocusGroup.Item>
                        </Menued>
                        {/* Its settings (SE-23, V89i): on the focused workspace only, and
                            quiet until its tab is under the pointer or the keyboard. The
                            row its menu's Workspace settings… runs. The plane root is not
                            a workspace and has none. */}
                        {workspace === focused && !root && (
                          <Gear offer={by(`workspace.settings:${workspace}`)} onPress={press} />
                        )}
                      </span>
                    )}
                  </SortableTab>
                );
              })}
              {/* **The `+`, right after the last tab**, inside the strip as the project
                  strip's is and as Chrome draws it (D6, DS-7 #1839). No `role="tab"` and
                  not owned by the tablist (#1204); `useRoom` takes its width off the room
                  first (`controls`), so the collapse never hides it. The arrows pass over
                  it; Tab stops on it after the focused workspace's tab. Why a `+` at all,
                  and why this glyph, is written at the show-more below. */}
              <span className="strip-doing" ref={workspaceControls}>
                <Doer offer={by("workspace.create")} onPress={press} iconOnly />
              </span>
            </div>
          </RovingFocusGroup.Root>
        </SortableContext>
      </DndContext>
      {/* What the strip is not drawing, **at the right end of the row** (D6): it appears
          only once the strip is full, so it then sits beside the `+`. `.strip-doing` and
          not a `.more` of its own, because the strips' own controls share one look and a
          second class name would be a second place to dress them.

          **The `+` is the operator's, and it is drawing a control over a row that was
          already there** — *"also no new workspace button in workspaces tab — it should
          be like projects tabs buttons"* (charter-app#193). `workspace.create` has been in
          the catalogue since #172, with the dialog behind it; the palette runs it and the
          tab's own menu lists it, and the one strip that is entirely about workspaces had
          no way to make one. Nothing here knows what it does: it is one `Doer` over that
          row, so its words, its availability and its refusal are the catalogue's, exactly
          as they are in the other two surfaces that offer it.

          **`Plus`, which is the chat strip's glyph and not the project strip's pair.**
          charter-app#178 split `project.open` and `project.create` into `FolderOpen` and
          `FolderPlus` because that strip draws two of them an inch apart and one glyph on
          both is a strip aimed at by memory. There is one control here, and the rule it
          falls under is the older one: the `+` at the end of a strip makes one more of
          what the strip lists, which an operator learns once for all three. */}
      <div className="strip-doing">
        <QueueRead>
          {(queue) => (
            <ShowMore
              noun="workspace"
              hidden={workspacesNotShowing.map((workspace) => ({
                key: workspace,
                offer: by(`workspace.focus:${workspace}`),
                needs: waitingIn(queue, workspace),
                children: workspaceMarks(workspace),
              }))}
              onPress={press}
            />
          )}
        </QueueRead>
      </div>
    </div>
  );
}

/** Each is `PlaneView`'s own value of the same name: the strip reads them and holds none. */
type WorkspaceStripProps = {
  by: (id: string) => Offer | undefined;
  found: Catalogued;
  press: (offer: Offer) => void;
  focused: string | undefined;
  colourOf: (workspace: string | undefined) => string | null;
  tintOf: (workspace: string | undefined) => CSSProperties;
  workspaceMarks: (workspace: string) => ReactNode;
  workspacesShown: { shown: string[]; hidden: string[] };
  workspacesNotShowing: readonly string[];
  workspaceTabId: (at: number) => string;
  workspaceStop: ReturnType<typeof useTabStop>;
  workspaceStrip: Ref;
  workspaceControls: Ref;
  workspaceLeast: number;
  rootWidth: number;
  workspaceRoot: Ref;
  dragSensors: ReturnType<typeof useStripSensors>;
  workspaceDragWords: ReturnType<typeof stripAccessibility>;
  dragWorkspace: (moved: string, onto: string) => void;
  waitingIn: (queue: readonly number[], workspace: string) => number;
};
