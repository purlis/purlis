// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { ShieldCheck, ShieldHalf, ShieldOff } from "lucide-react";

/** How much a chat's sandbox holds, as the core says it (`OpenChat.sandbox`, #2071). */
export type SandboxLevel = "full" | "floor" | "none";

/** What each level is called, on its tab and to a screen reader. */
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
export function SandboxMark({ level }: { level?: string | null }) {
  if (level == null || !isLevel(level)) return null;
  const Icon = ICONS[level];
  // Named by `aria-label`, not by words in the tab: the tab's text stays the chat's name, which
  // is what the window and its close button call the chat.
  return (
    <span
      className="tab-sandbox"
      data-sandbox={level}
      role="img"
      aria-label={SANDBOX_SAID[level]}
      title={SANDBOX_SAID[level]}
    >
      <Icon className="tab-mark" aria-hidden="true" />
    </span>
  );
}
