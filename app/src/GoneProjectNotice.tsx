// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState, useSyncExternalStore } from "react";
import { commands, type GoneProject } from "./bindings";
import { Notice } from "./Notice";

/**
 * **The gone projects dismissed in this window, by path** (#1291): one list, which the window's
 * line about a project the last quit had open and the opener's line about a recent both read,
 * so a project dismissed in one is not drawn by the other, nor by an opener drawn again later
 * in the run (the opener's own state went with it each time it was put away).
 *
 * A dismissal lasts until its cause changes: until the core's answer of what this machine
 * remembers ({@link settleGone}) no longer lists the path as gone, because it is there again, or
 * was located or forgotten (here, on another line, or in Settings › You › This machine).
 *
 * **For this run only, for now.** The kept dismissals (`dismissals.ts`, NO-2) are written per
 * project the window holds, and a gone project is never one it holds, so the core will not take
 * one keyed by its path; keeping these past a relaunch waits on the core taking a remembered
 * gone path there.
 */
let goneDismissed: ReadonlySet<string> = new Set();
const goneListeners = new Set<() => void>();

function holdGone(next: ReadonlySet<string>) {
  goneDismissed = next;
  for (const listener of goneListeners) listener();
}

/** The gone project at `path` is dismissed for this run. */
export function dismissGone(path: string): void {
  if (!goneDismissed.has(path)) holdGone(new Set(goneDismissed).add(path));
}

/**
 * **The core answered which remembered projects are gone** (`recent_planes`): a dismissal for a
 * path that answer lacks is let go, because its cause changed. Called only with an answer, never
 * with a list not read yet.
 */
export function settleGone(stillGone: readonly string[]): void {
  const next = [...goneDismissed].filter((path) => stillGone.includes(path));
  if (next.length !== goneDismissed.size) holdGone(new Set(next));
}

/** The gone projects dismissed in this run, for a component that redraws when they change. */
export function useGoneDismissed(): ReadonlySet<string> {
  return useSyncExternalStore(onGoneDismissed, () => goneDismissed);
}

/** Calls `listener` whenever a gone project is dismissed or let go. Answers the way to stop. */
function onGoneDismissed(listener: () => void): () => void {
  goneListeners.add(listener);
  return () => void goneListeners.delete(listener);
}

/** Forgets this run's dismissals, as a new launch would. For tests. */
export function forgetGoneDismissals(): void {
  goneDismissed = new Set();
}

/**
 * **A remembered project that is gone, with Locate… and Forget** (NO-5, #1237): drawn by the
 * window for a project the last quit had open, and by the opener for a recent.
 *
 * - **Forget** is the core's (ST-2): the project's recent, approval, pins and tabs go, so it no
 *   longer comes back at every launch.
 * - **Locate…** asks for a folder, and the core re-points the entry once it has checked a
 *   project is there (`locate_project`). What it found is handed to `onLocated`, which opens it
 *   through the trust gate like any open: the approval does not travel with the path.
 *
 * Neither is done without a press, because a disk that is unplugged may come back. A refusal
 * is said inside the Notice, which stays, so the operator can pick again; so is a folder picker
 * that could not finish, which a cancel is not.
 */
export function GoneProjectNotice({
  gone,
  cause,
  onLocated,
  onForgotten,
  onDismiss,
}: {
  gone: GoneProject;
  /** What the line is about, stably (`project-gone:<path>`). */
  cause: string;
  /** The core re-pointed the entry at this project. */
  onLocated: (found: string) => void;
  /** The core forgot the entry. */
  onForgotten: () => void;
  onDismiss: () => void;
}) {
  const [refused, setRefused] = useState<string>();

  const locate = async () => {
    const picked = await commands
      .pickProject()
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    // A picker that could not finish is said, like a refusal: otherwise it reads as a cancel.
    if (picked.status === "error") {
      setRefused(picked.error);
      return;
    }
    // A cancelled dialog is null and is not a failure: nothing is said and nothing moves.
    if (!picked.data) return;
    const located = await commands
      .locateProject(gone.path, picked.data)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    if (located.status === "ok") onLocated(located.data);
    else setRefused(located.error);
  };

  const forget = async () => {
    const forgot = await commands
      .forgetProject(gone.path)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    if (forgot.status === "ok") onForgotten();
    else setRefused(forgot.error);
  };

  return (
    <Notice
      cause={cause}
      tone={refused === undefined ? "news" : "trouble"}
      fixes={[
        { label: "Locate…", onPress: () => void locate() },
        { label: "Forget", onPress: () => void forget() },
      ]}
      onDismiss={onDismiss}
    >
      {gone.said}
      {refused !== undefined && (
        <>
          {" "}
          <span>{refused}</span>
        </>
      )}
    </Notice>
  );
}
