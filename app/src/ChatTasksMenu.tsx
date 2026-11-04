// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useLayoutEffect, useRef, type RefObject } from "react";
import { AnchoredMenu, Menu, MenuItem } from "./ui/menu/Menu";

/** A task of the chat, as its line says it. */
export type ChatTask = {
  /** Its chat, by number, while it runs; none for one that has finished. */
  session?: number;
  /** A key for its line: the session, or a finished task's record id. */
  key: string;
  name: string;
  /** How a finished one ended, in words (`done`, `stopped by you`). None while it runs. */
  ended?: string;
};

/**
 * **⌘⇧J: the focused chat's own tasks** (#5058, ruled on #4699): a menu of the tasks the chat
 * in the focused pane asked for, and nothing else's, under that pane's head. Picking one shows
 * it in that pane, as a press does: a task on screen already is jumped to (D1). A task that has
 * finished is listed by how it ended and cannot be picked: nothing of it runs to be shown.
 *
 * Open while its owner says so, and drawn while closed, so the close is seen. Escape, a press outside and a pick close it; a close that was
 * no pick hands the keyboard back to where it was when the menu opened.
 */
export function ChatTasksMenu({
  open,
  name,
  tasks,
  at,
  from,
  onShow,
  onClose,
}: {
  open: boolean;
  /** Whose tasks these are, as its row names it. */
  name: string;
  tasks: readonly ChatTask[];
  /** What the menu stands under: the focused pane's head. */
  at: RefObject<Element | null>;
  /** Where the keyboard was when it was asked for. */
  from: Element | null;
  /** Shows task `session` in the focused pane. */
  onShow: (session: number) => void;
  onClose: () => void;
}) {
  const picked = useRef(false);
  // Each opening starts with nothing picked.
  useLayoutEffect(() => {
    if (open) picked.current = false;
  }, [open]);
  return (
    <AnchoredMenu
      triggerRef={at}
      isOpen={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      autoFocus="first"
      onClosed={() => {
        // A pick puts the keyboard in the chat picked. Otherwise it goes back to where it was.
        if (picked.current || !(from instanceof HTMLElement) || !from.isConnected) return;
        const now = document.activeElement;
        if (now === null || now === document.body || !now.isConnected) from.focus();
      }}
    >
      <Menu aria-label={`Tasks of ${name}`} className="tasks-menu">
        {tasks.map((task) => (
          <MenuItem
            key={task.key}
            id={task.key}
            className="tasks-menu-row"
            textValue={task.name}
            // A finished task stays in the arrows' way and says how it ended.
            aria-disabled={task.session === undefined ? true : undefined}
            shouldCloseOnSelect={task.session !== undefined}
            onAction={() => {
              if (task.session === undefined) return;
              picked.current = true;
              onShow(task.session);
            }}
          >
            <span className="name">{task.name}</span>
            {task.ended !== undefined && <span className="where"> {task.ended}</span>}
          </MenuItem>
        ))}
      </Menu>
    </AnchoredMenu>
  );
}
