// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { ModeDiffers } from "./bindings";
import { modeDiffersOf, useChatsHere, useChatsSelect } from "./chatState";
import { Notice } from "./Notice";

/** Each permission mode Claude Code names in its hooks (Q270), as a sentence says it. */
const MODE_SAID: Readonly<Record<string, string>> = {
  default: "default",
  acceptEdits: "accept edits",
  plan: "plan",
  auto: "auto",
  bypassPermissions: "bypass permissions",
};

/** What the Notice says of `differs`: the two modes, then what purlis does about it. A mode
 *  nobody measured is said as one purlis doesn't know, never as a name it guessed. */
export function modeDiffersSaid(differs: ModeDiffers): string {
  const set = MODE_SAID[differs.set] ?? differs.set;
  const reported = MODE_SAID[differs.reported];
  const what =
    reported === undefined
      ? `Claude Code reports a permission mode for this chat that purlis doesn't know. purlis started it in ${set} mode.`
      : `Claude Code reports this chat runs in ${reported} mode, but purlis started it in ${set} mode.`;
  return `${what} purlis counts this chat as unattended from now on.`;
}

/**
 * **A mode purlis didn't set, on its chat's pane** (R-243, T1-44, #6435): a standing fault, so a
 * Notice and never an ask. The core tells it once the chat's attendance fact flipped on a mode
 * its armed hook reported (`stuck.rs`' `mode`); only Claude Code's hook is read for one, so on
 * any other harness this draws nothing. The fact moves one way, so nothing here can undo it:
 * **Dismiss** puts the Notice away, and another mode reported is a new cause that brings it back.
 */
export function ModeDiffersNotice({ session }: { session: number }) {
  const differs = useChatsSelect(useChatsHere(), (states) => modeDiffersOf(states, session));
  const [dismissed, setDismissed] = useState<string | null>(null);
  if (differs === undefined) return null;
  const cause = `mode-differs:${session}:${differs.reported}`;
  if (dismissed === cause) return null;
  return (
    <Notice
      cause={`mode-differs:${session}:${differs.reported}`}
      at="pane"
      label="Mode"
      onDismiss={() => setDismissed(cause)}
    >
      {modeDiffersSaid(differs)}
    </Notice>
  );
}
