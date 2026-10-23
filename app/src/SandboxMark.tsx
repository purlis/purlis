// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { ShieldCheck, ShieldHalf, ShieldOff } from "lucide-react";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** How much a chat's sandbox holds, as the core says it (`OpenChat.sandbox`, #2071). */
export type SandboxLevel = "full" | "floor" | "none";

/** What each level is called, on its tab and to a screen reader. */
/** A chat's level as the plane's chats say it, and why where it runs with no sandbox. */
export type SandboxSaid = { level: string; why?: string | null; sites?: number | null };

export const SANDBOX_SAID: Readonly<Record<SandboxLevel, string>> = {
  full: "full sandbox",
  floor: "floor",
  none: "no sandbox",
};

const ICONS = { full: ShieldCheck, floor: ShieldHalf, none: ShieldOff } as const;

/** Whether `word` is a level the core says. */
function isLevel(word: string): word is SandboxLevel {
  return word in ICONS;
}

/**
 * **A chat's sandbox level, on its tab** (#2071): the full sandbox, the floor (the default:
 * no network or folder limits, but no vault read and nothing a later program runs written), or
 * no sandbox. Always drawn for a chat, so its level is never out of sight; nothing for a shell
 * or a chat still to start.
 */
export function SandboxMark({
  level,
  why,
  sites,
  of,
}: {
  level?: string | null;
  why?: string | null;
  /** How many sites only the chat's browser reaches (#2364): said after its level. */
  sites?: number | null;
  /** Whose level it is, where that is not plain from where it is drawn: the title bar's. */
  of?: string;
}) {
  if (level == null || !isLevel(level)) return null;
  const Icon = ICONS[level];
  // Where the floor could not be applied here, the mark says why (#2071, Q2), on its title and
  // to a screen reader: not as a Notice on the pane, which every such chat would carry.
  const levelSaid = why ? `${SANDBOX_SAID[level]}: ${why}` : SANDBOX_SAID[level];
  // Its browser's own sites, apart from the level (#2364): the Network view lists them.
  const withSites =
    sites != null && sites > 0
      ? `${levelSaid}; its browser reaches ${sites} ${sites === 1 ? "site" : "sites"} of its own`
      : levelSaid;
  const said = of ? `${of}: ${withSites}` : withSites;
  // Named by `aria-label`, not by words in the tab: the tab's text stays the chat's name, which
  // is what the window and its close button call the chat.
  return (
    <Tooltip tip={said}>
      <span className="tab-sandbox" data-sandbox={level} role="img" aria-label={said}>
        <Icon className="tab-mark" aria-hidden="true" />
      </span>
    </Tooltip>
  );
}
