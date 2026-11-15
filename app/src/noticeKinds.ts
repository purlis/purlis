// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { familyOf } from "./Notice";

/**
 * **What kind of Notice a cause is, in the window's words** (Q182, R-332): the Notices view
 * filters by it and groups by it, with the count ("Clones made outside purlis · 3"), and its
 * Dismissed list names each dismissed Notice by it (Q183, R-334).
 *
 * A kind is named for one Notice (`one`, a dismissed row's) and for many (`many`, a group's head
 * and its filter). Several families can be one kind: what a relaunch did to a chat is four.
 * A family not named here is an {@link OTHER} Notice, so a new family is never left out of the
 * list, only grouped with the rest until it is named.
 */
export type NoticeKind = { one: string; many: string };

/** The kind of every family not named here. */
export const OTHER: NoticeKind = { one: "Notice", many: "Other notices" };

const REOPENED: NoticeKind = { one: "Chat reopened at launch", many: "Chats reopened at launch" };
const SANDBOX_BLOCK: NoticeKind = { one: "Sandbox block", many: "Sandbox blocks" };
const SANDBOX_SETTING: NoticeKind = { one: "Sandbox setting", many: "Sandbox settings" };

/** The named kinds, by the family of cause (`Notice.tsx`'s `familyOf`). Every family whose
 *  dismissal is kept (`dismissals.ts`'s `KEPT`) is named, so a dismissed row always says what it
 *  was (`noticeKinds.test.ts`). */
export const NOTICE_KINDS: Readonly<Record<string, NoticeKind>> = {
  "clone-vouch": { one: "Clone made outside purlis", many: "Clones made outside purlis" },
  "stale-clone": { one: "Stale repo", many: "Stale repos" },
  "pin-dormant": { one: "Dormant workspace", many: "Dormant workspaces" },
  "chat-resumed": REOPENED,
  "chat-guessed": REOPENED,
  "chat-fresh": REOPENED,
  "chat-untaken": REOPENED,
  "doctor-finding": { one: "Doctor finding", many: "Doctor findings" },
  "sandbox-changed": { one: "Sandbox change", many: "Sandbox changes" },
  "project-gone": { one: "Missing project", many: "Missing projects" },
  "sandbox-blocked": SANDBOX_BLOCK,
  "sandbox-blocked-tasks": SANDBOX_BLOCK,
  "sandbox-offer": SANDBOX_SETTING,
  "sandbox-hosts": SANDBOX_SETTING,
  "sandbox-presets": SANDBOX_SETTING,
  "sandbox-expects": SANDBOX_SETTING,
  "sandbox-policy": SANDBOX_SETTING,
  "sandbox-team-off": SANDBOX_SETTING,
  "vaults-waiting": { one: "Vault waiting", many: "Vaults waiting" },
  "report-draft": { one: "Report draft", many: "Report drafts" },
  "arrived-files": { one: "New files in a chat's folders", many: "New files in chats' folders" },
};

/** The kind of the Notice whose cause is `cause`. */
export const kindOf = (cause: string): NoticeKind => NOTICE_KINDS[familyOf(cause)] ?? OTHER;

/** What a dismissed Notice was about, after its kind: its cause past the family, as the core
 *  keyed it (`clone-vouch:repos/web|github.com/acme/web` is `repos/web`). */
export function aboutOf(cause: string): string {
  const rest = cause.slice(familyOf(cause).length + 1);
  // A clone's cause is its folder and the address its remote named then.
  return familyOf(cause) === "clone-vouch" ? rest.split("|", 1)[0] : rest;
}

/** Whether a Notice that reads `text` is one a search for `query` finds: every word typed is in
 *  it, whatever its case. An empty search finds every one. */
export function searchFinds(text: string, query: string): boolean {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const read = text.toLowerCase();
  return words.every((word) => read.includes(word));
}
