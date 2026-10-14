// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { commands } from "./bindings";

/**
 * **A command the core has not answered for a while is said, not waited on in silence** (#1949).
 *
 * The core answers most of the window's commands off the window's thread now, so a dialog that
 * waits on one waits as long as the core takes. A wait the person cannot see ends in nothing
 * happening: a question that never comes up, a press that seems lost. Every command is timed
 * from its ask, and one still unanswered after {@link LATE_MS} is written down here; the window
 * says so on its line (`App.tsx`), and a scenario test that fails reads the list
 * (`window.__purlisLateAnswers`, `e2e/failures.ts`), so the failure names what never came.
 *
 * Commands that wait on a person or on the network by design are not timed (`WAITS_BY_DESIGN`).
 */

/** How long a command may take before the window says it is late. */
export const LATE_MS = 20_000;

/** How many late answers are kept, newest last. */
export const KEPT = 20;

/**
 * The commands that may take longer than {@link LATE_MS} on purpose: they wait for a person (a
 * system dialog, a password, a sign-in in the browser), for the network (a clone, a push, a
 * forge's list), or they never answer until the window goes (a quit, a restart).
 */
export const WAITS_BY_DESIGN: ReadonlySet<string> = new Set([
  // A person: the system's own dialogs, and an administrator's password.
  "pickProject",
  "pickExtension",
  "installCliOnPath",
  "installDeveloperTools",
  // A sign-in, finished in the browser or on another device.
  "forgeDeviceSignInWait",
  "forgePkceSignIn",
  "forgeSignInWithToken",
  "forgeImportCliLogin",
  "forgeCliLogins",
  "vaultSetupAccounts",
  "vaultSetupBegin",
  "vaultSetupTest",
  "vaultSetupCreate",
  "vaultSetupChange",
  "forgeDeviceSignInStart",
  // A vault's provider: its own program, which may ask the person (a fingerprint, a password)
  // before it answers.
  "vaultOpen",
  "vaultRefresh",
  "vaultSecretReveal",
  "vaultSecretCopy",
  "vaultIdentityPut",
  // The network.
  "getProject",
  "openRepo",
  "cloneRepo",
  "takeRepos",
  "reachableRepos",
  "publishProject",
  "createProjectOnForge",
  "forgeOwners",
  "forgeRepoList",
  "forgeAccounts",
  "planeFetch",
  "savePlane",
  "saveRepo",
  "updateProject",
  "changePush",
  "changeLand",
  "mergeSaveRequest",
  "fixSaveRequest",
  "taskBranchMerge",
  "worktreeMerge",
  "lookAtSaveRequest",
  "changePushQuestion",
  "taskBranchMergeQuestion",
  // A program of the person's or of a project's: an extension's action, a doctor's fix, a chat's
  // harness starting with what it brings (curation, the first task).
  "runAction",
  "planeDoctorFix",
  "curate",
  "firstTaskRun",
  "checkForUpdate",
  "installUpdate",
  "installExtension",
  // The window goes before an answer would come.
  "quit",
  "askToQuit",
  "relaunch",
  "restartToUpdate",
  "restartToUpdateChoosing",
  "restartOnTheSessionBus",
]);

/** One command answered late, or not yet. */
export type Late = {
  /** The command, by the window's name for it. */
  command: string;
  /** When it was asked, by the clock on the wall. */
  at: number;
  /** How long its answer took, or `undefined` while none has come. */
  ms: number | undefined;
};

let late: readonly Late[] = [];
const listeners = new Set<() => void>();

function changed(): void {
  late = [...late];
  for (const listener of listeners) listener();
}

type Ask = (...args: unknown[]) => Promise<unknown>;

/** `ask`, timed: written down here once it is {@link LATE_MS} late, and again when it answers. */
export function timed(command: string, ask: Ask, lateMs = LATE_MS): Ask {
  return (...args: unknown[]) => {
    const from = performance.now();
    let mine: Late | undefined;
    const timer = setTimeout(() => {
      mine = { command, at: Date.now() - lateMs, ms: undefined };
      late = [...late, mine].slice(-KEPT);
      changed();
    }, lateMs);
    return ask(...args).finally(() => {
      clearTimeout(timer);
      if (mine === undefined) return;
      // A new entry in its place, so whoever drew the old one sees it changed.
      const was = mine;
      const answered = { ...was, ms: performance.now() - from };
      late = late.map((one) => (one === was ? answered : one));
      changed();
    });
  };
}

let watching = false;

/**
 * Times every command the window asks, once, for the life of the window, but the ones that wait
 * on purpose. After the lanes and the switch clock, so what is timed is what the person waits.
 */
export function watchTheAnswers(): void {
  if (watching) return;
  watching = true;
  const asks = commands as unknown as Record<string, Ask>;
  for (const [name, ask] of Object.entries(asks)) {
    if (typeof ask !== "function" || WAITS_BY_DESIGN.has(name)) continue;
    asks[name] = timed(name, ask.bind(commands));
  }
  (globalThis as { __purlisLateAnswers?: () => readonly Late[] }).__purlisLateAnswers = () => late;
}

/** The late answers kept, oldest first. */
export function useLateAnswers(): readonly Late[] {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => late,
  );
}

/** Forgets every late answer: a test's window is a new one. */
export function forgetLateAnswers(): void {
  late = [];
  changed();
}

/** What the window says of one late answer still not come, in its own line. */
export function saidOfLate(one: Late): string {
  return (
    `purlis has not answered the window for ${Math.round(LATE_MS / 1000)} s (${one.command}). ` +
    "It may still be working; what you asked for appears when it answers."
  );
}
