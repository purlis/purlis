// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";
import { listen } from "./here";

/**
 * **A terminal `$VISUAL`/`$EDITOR` opens in a shell tab, at the line** (#1042).
 *
 * The core starts the editor in a tab of its own, as an argument vector and never through a
 * shell (`app/src-tauri/src/editortab.rs`), and says so on {@link EDITOR_TAB_OPENED}. The
 * window's part is to draw the tab: **in front**, since the person pressed Open in your editor
 * for it, filed on the file's workspace, and marked as a shell tab.
 */
export const EDITOR_TAB_OPENED = "editor-tab-opened";

/** What the core says of a terminal editor's tab. */
export type EditorTabOpened = {
  plane: string;
  session: number;
  /** The editor and the file's name, `vim lib.rs`. */
  name: string;
  workspace: string;
};

/** Calls `draw` for each terminal editor's tab opened in `plane`, for as long as it is mounted. */
export function useEditorTabs(plane: string, draw: (opened: EditorTabOpened) => void): void {
  useEffect(() => {
    const listening = listen<EditorTabOpened>(EDITOR_TAB_OPENED, (event) => {
      if (event.payload.plane === plane) draw(event.payload);
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane, draw]);
}
