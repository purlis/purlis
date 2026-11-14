// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { ModeDiffers } from "./bindings";
import { harnessSaid } from "./chatLog";
import { modeDiffersOf, useChatsHere, useChatsSelect } from "./chatState";
import { Notice } from "./Notice";

/**
 * **Each harness's own permission modes, as a sentence says them**, by the harness as the core
 * names it: only a harness whose adapter reports a mode purlis can check has any (Claude Code's,
 * measured in its hooks, Q270). Another harness's chat is never told one harness's words.
 */
const MODES_SAID: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  claude: {
    default: "default",
    acceptEdits: "accept edits",
    plan: "plan",
    auto: "auto",
    bypassPermissions: "bypass permissions",
  },
};

/** What the Notice says of `differs`: the harness that reported it, by its adapter's display
 *  name, the two modes in its own words, then what purlis does about it (R-005, Q280). A mode
 *  nobody measured is said as one purlis doesn't know, never as a name it guessed. */
export function modeDiffersSaid(differs: ModeDiffers): string {
  const harness = harnessSaid(differs.harness);
  const words = MODES_SAID[differs.harness ?? ""] ?? {};
  const set = words[differs.set];
  const reported = words[differs.reported];
  const started = set === undefined ? "" : ` purlis started it in ${set} mode.`;
  const what =
    reported === undefined
      ? `${harness} reports a permission mode for this chat that purlis doesn't know.${started}`
      : set === undefined
        ? `${harness} reports this chat runs in ${reported} mode, which purlis didn't start it in.`
        : `${harness} reports this chat runs in ${reported} mode, but purlis started it in ${set} mode.`;
  return `${what} purlis counts this chat as unattended from now on.`;
}

/**
 * **A mode purlis didn't set, on its chat's pane** (R-243, T1-44, #6435): a standing fault, so a
 * Notice and never an ask. The core tells it once the chat's attendance fact flipped on a mode
 * its armed hook reported (`stuck.rs`' `mode`), with the harness that reported it; only a
 * harness whose adapter reports a mode purlis can check is read for one, so on any other this
 * draws nothing. The fact moves one way, so nothing here can undo it:
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
