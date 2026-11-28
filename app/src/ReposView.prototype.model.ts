// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE (#7997, map #7990; never merged). The model behind the one Repos view's three
 * variants: what a branch folder's row is, which group it falls in, the proposed "needs you"
 * states that drive the Repos icon's badge, and the words of each state's one status icon.
 * Nothing here is wired to the core; every action is a stub that says what it would do.
 */
import {
  CircleCheck,
  CircleDot,
  CircleX,
  GitMerge,
  GitPullRequest,
  Moon,
  Pause,
  Swords,
  Unplug,
  FileDiff,
  type LucideIcon,
} from "lucide-react";

/**
 * **A folder's one state.** The first five are the proposed **needs you** set: the only states
 * the Repos icon's badge counts.
 */
export type RowState =
  | "conflict"
  | "check-failed"
  | "merge-waiting"
  | "behind-dirty"
  | "unwired"
  | "live"
  | "pr-open"
  | "resting"
  | "landed"
  | "dormant";

export interface FolderRow {
  id: string;
  repo: string;
  /** The folder's name under `.worktrees/<repo>`. */
  folder: string;
  branch: string;
  /** The owning task's or chat's title, when one is known. */
  title?: string;
  owner: { kind: "chat" | "task" | "pr" | "none"; label: string };
  state: RowState;
  pr?: number;
  /** A live chat that holds a landed folder, so it cannot leave yet. */
  held?: string;
  ahead: number;
  behind: number;
  /** Uncommitted files. */
  dirty: number;
  sizeGb: number;
  /** Of `sizeGb`, what `target/` and `node_modules` hold. */
  buildGb: number;
  ageMin: number;
}

export interface RemovedLine {
  branch: string;
  repo: string;
  /** Local time it went, HH:MM. */
  at: string;
  gb: number;
  why: string;
  pr: number;
}

export interface RepoHead {
  name: string;
  branch: string;
  ahead: number;
  behind: number;
  ci: "passing" | "failing" | "running" | "none";
  uncommitted: number;
  /** Clone trouble: not cloned, or its clone failed. */
  clone?: { kind: "not-cloned" } | { kind: "failed"; why: string } | { kind: "cloning" };
}

export interface ReposData {
  workspace: string;
  repos: RepoHead[];
  rows: FolderRow[];
  removed: RemovedLine[];
  /** Free disk in GB, for the pressure rule (#7993: below 50 GB or 10% free). */
  freeGb: number;
  diskGb: number;
  /** Folders whose `target/` the idle cleanup cleared under pressure. */
  idleCleared?: { folders: number; gb: number };
  /** The workspace itself: its own folder, drawn as the first row. */
  workspaceLive: boolean;
}

export type Group = "needs" | "working" | "finished" | "dormant";

export const NEEDS_YOU: ReadonlySet<RowState> = new Set([
  "conflict",
  "check-failed",
  "merge-waiting",
  "behind-dirty",
  "unwired",
]);

export function groupOf(state: RowState): Group {
  if (NEEDS_YOU.has(state)) return "needs";
  if (state === "landed") return "finished";
  if (state === "dormant") return "dormant";
  return "working";
}

export const GROUPS: { id: Group; title: string }[] = [
  { id: "needs", title: "Needs you" },
  { id: "working", title: "Working" },
  { id: "finished", title: "Finished" },
  { id: "dormant", title: "Dormant" },
];

/** What each state's one icon is, its tone and what its tooltip says first. */
export const STATES: Record<
  RowState,
  { icon: LucideIcon; tone: "failed" | "waiting" | "success" | "accent" | "quiet"; word: string }
> = {
  conflict: { icon: Swords, tone: "failed", word: "Conflicts with main" },
  "check-failed": { icon: CircleX, tone: "failed", word: "A check failed" },
  "merge-waiting": { icon: GitMerge, tone: "waiting", word: "Ready to merge: waits for you" },
  "behind-dirty": { icon: FileDiff, tone: "waiting", word: "Behind main, with uncommitted work" },
  unwired: { icon: Unplug, tone: "waiting", word: "Missing a layer the plane carries" },
  live: { icon: CircleDot, tone: "accent", word: "A chat is working in it" },
  "pr-open": { icon: GitPullRequest, tone: "quiet", word: "Pull request open" },
  resting: { icon: Pause, tone: "quiet", word: "No chat in it" },
  landed: { icon: CircleCheck, tone: "success", word: "Landed" },
  dormant: { icon: Moon, tone: "quiet", word: "Dormant" },
};

/** The sentence after the state's word in its tooltip: why, and what happens next. */
export function stateSaid(row: FolderRow): string {
  switch (row.state) {
    case "conflict":
      return `#${row.pr} conflicts with main. Open it to resolve, or ask its chat to.`;
    case "check-failed":
      return `#${row.pr}: “app (macOS)” failed 14 min ago.`;
    case "merge-waiting":
      return `#${row.pr} passed every check. Merge waits for your press.`;
    case "behind-dirty":
      return `${row.behind} behind main, with ${row.dirty} uncommitted files and no chat in it.`;
    case "unwired":
      return "The plane carries its ask rules layer, which this folder lacks. Its chats run unguarded until it is wired again.";
    case "live":
      return `${row.owner.label} is working here${row.dirty ? `, ${row.dirty} files uncommitted` : ""}.`;
    case "pr-open":
      return `#${row.pr} is open, ${row.ahead} ahead, ${row.behind} behind main.`;
    case "resting":
      return `Last touched ${ageSaid(row.ageMin)} ago. Dormant after 7 days silent.`;
    case "landed":
      return row.held
        ? `Merged as #${row.pr}. ${row.held} still holds it, so it leaves when that chat closes.`
        : `Merged as #${row.pr}. It leaves on its own within 30 min.`;
    case "dormant":
      return `Silent ${ageSaid(row.ageMin)}, no pull request, not landed. Put away keeps the branch.`;
  }
}

/** The branch as a row names it when no title is known: without its 8-character id tail. */
export const shortBranch = (branch: string) => branch.replace(/-[a-z0-9]{8}$/, "");

export const nameOf = (row: FolderRow) => row.title ?? shortBranch(row.branch);

export function ageSaid(min: number): string {
  if (min < 60) return `${min}m`;
  if (min < 60 * 24) return `${Math.round(min / 60)}h`;
  return `${Math.round(min / 60 / 24)}d`;
}

export function gbSaid(gb: number): string {
  if (gb < 1) return `${Math.round(gb * 1000)} MB`;
  return `${gb < 10 ? gb.toFixed(1) : Math.round(gb)} GB`;
}

export const sum = (rows: readonly FolderRow[]) => rows.reduce((a, r) => a + r.sizeGb, 0);

/** What could go now: every finished folder and every dormant one (its branch kept). */
export const canGo = (rows: readonly FolderRow[]) =>
  rows.filter((r) => r.state === "landed" || r.state === "dormant");

/** Pressure starts below 50 GB or 10% free, whichever is larger (#7993). */
export const underPressure = (d: Pick<ReposData, "freeGb" | "diskGb">) =>
  d.freeGb < Math.max(50, d.diskGb * 0.1);

/** The header's disk sentence: "253 GB in 168 folders · 61 GB can go". */
export function diskSaid(rows: readonly FolderRow[]): string {
  const goes = sum(canGo(rows));
  return `${gbSaid(sum(rows))} in ${rows.length} folders${goes > 0 ? ` · ${gbSaid(goes)} can go` : ""}`;
}

/** The path the tooltip names. */
export const pathOf = (row: FolderRow, workspace: string) =>
  `~/IdeaProjects/charter/workspaces/${workspace}/.worktrees/${row.repo}/${row.folder}`;

/** A row's order inside its group: needs you by urgency, the rest newest first. */
export function byAge(a: FolderRow, b: FolderRow) {
  return a.ageMin - b.ageMin;
}

/** What a stub says it would do: logged, and shown in the frame's log. */
export type Did = (what: string) => void;
