// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type GitInstall } from "./bindings";

/**
 * **Git first, then the action** (#1813, spec #1810 Q5): the window's half of the git
 * presence check.
 *
 * The core never runs git while it is missing (`worktree::git::presence`, the runner's gate),
 * so nothing here is what keeps the system's install dialog away. What this adds is the
 * person's way through: an action that needs git is handed to {@link GitGate.withGit}, which
 * runs it at once when git is there, and otherwise holds it and opens the guided screen
 * (`GitMissing`). Install starts Apple's installer on macOS; from then on the gate looks again
 * every {@link WATCH_MS} by itself, and Check again looks at once. The moment git is there, the
 * held action runs: the person carries on where they were.
 *
 * Mount it once per surface whose actions need git (the welcome's doors, the guided steps),
 * and draw `<GitMissing gate={gate} />` beside them.
 */
export type GitGate = {
  /** Whether the guided screen is up: an action waits for git. */
  open: boolean;
  /** How this machine gets git, once a look found it missing. Null before that, and when the
   *  look itself failed (then `trouble` says why). */
  install: GitInstall | null;
  /** A look at the machine is in progress. */
  checking: boolean;
  /** Install was pressed and Apple's installer is up: the gate is watching for git. */
  installing: boolean;
  /** What went wrong, in the core's words: a look that failed, or an installer that did not
   *  start. */
  trouble: string | null;
  /** Runs `action` once git is there: now when it is, else when a later look finds it. A
   *  second call while one waits replaces it: the person's latest press is what resumes. */
  withGit: (action: () => unknown) => Promise<void>;
  /** Looks again; runs the held action when git is there now. */
  checkAgain: () => Promise<void>;
  /** Starts Apple's installer (macOS), then watches for git. */
  startInstall: () => Promise<void>;
  /** Not now: closes the screen and drops the held action. */
  cancel: () => void;
};

/** How often the gate looks again while Apple's installer runs. */
export const WATCH_MS = 5000;

type Look =
  { present: true } | { present: false; install: GitInstall | null; trouble: string | null };

async function look(): Promise<Look> {
  try {
    const answer = await commands.gitPresence();
    if (answer.status === "error") return { present: false, install: null, trouble: answer.error };
    if (answer.data.kind === "present") return { present: true };
    return { present: false, install: answer.data.install, trouble: null };
  } catch (err: unknown) {
    return { present: false, install: null, trouble: String(err) };
  }
}

export function useGitPresence(): GitGate {
  const [open, setOpen] = useState(false);
  const [install, setInstall] = useState<GitInstall | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [trouble, setTrouble] = useState<string | null>(null);
  /** The action waiting for git. A ref: it is run, never drawn. */
  const held = useRef<(() => unknown) | null>(null);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const close = useCallback(() => {
    held.current = null;
    setOpen(false);
    setInstalling(false);
    setTrouble(null);
  }, []);

  /** Looks, and either runs what waits or says why it still waits. */
  const settle = useCallback(async () => {
    setChecking(true);
    const found = await look();
    if (!mounted.current) return;
    setChecking(false);
    if (found.present) {
      const action = held.current;
      close();
      if (action) await action();
      return;
    }
    setInstall(found.install);
    setTrouble(found.trouble);
    setOpen(true);
  }, [close]);

  const withGit = useCallback(
    async (action: () => unknown) => {
      held.current = action;
      await settle();
    },
    [settle],
  );

  const startInstall = useCallback(async () => {
    setTrouble(null);
    try {
      const answer = await commands.installDeveloperTools();
      if (!mounted.current) return;
      if (answer.status === "error") {
        setTrouble(answer.error);
        return;
      }
      setInstalling(true);
    } catch (err: unknown) {
      if (mounted.current) setTrouble(String(err));
    }
  }, []);

  // While Apple's installer runs, look again now and then: the install ends in the system's
  // dialog, which tells purlis nothing.
  useEffect(() => {
    if (!open || !installing) return;
    const timer = setInterval(() => {
      void settle();
    }, WATCH_MS);
    return () => clearInterval(timer);
  }, [open, installing, settle]);

  return {
    open,
    install,
    checking,
    installing,
    trouble,
    withGit,
    checkAgain: settle,
    startInstall,
    cancel: close,
  };
}
