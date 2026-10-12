// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { commands } from "./bindings";
import {
  type Counted,
  type Event,
  FRESH,
  guides,
  type Made,
  next,
  type Progress,
  stepsOf,
} from "./guidedSteps";

/**
 * **The guided steps, held for the window and kept for the next launch** (#1817).
 *
 * The window holds one of these (`App.tsx`), because two of the steps reach past the welcome:
 * a project the steps asked for is held behind the harness step rather than drawn, and the
 * window's own ways of opening a project are where it arrives.
 *
 * - **Read each time the steps appear** (`reread`, which `Onboarding.tsx` asks as it mounts),
 *   from the core's `onboarding.json` (Machine tier, no secrets), so a quit at any step, or a
 *   person who left the welcome for a project and came back, is at the same step.
 * - **Kept on every change**, the whole of it. A machine with no config home keeps nothing and
 *   says so once, in `trouble`; the steps still work for this launch.
 * - **The git check is the git gate's** (`gitPresence.ts`): while an act waits for git at the
 *   project step, the steps are at the git check, and when git is there or the person said
 *   Not now, they are back at the project.
 */
export type Onboarding = {
  progress: Progress;
  /** The steps counted, and where the person is among them. */
  counted: { steps: Counted[]; at: number };
  /** Whether the machine said, at its last look, that no harness is installed. */
  harnessMissing: boolean;
  /** Why the steps' place could not be kept, in the core's words. */
  trouble: string | undefined;
  /** Whether the steps guide this window now: a window that holds no project, a machine that
   *  has never been through them, or a project held at their harness step. A window full of
   *  projects on a machine that finished them adds one with the plain welcome, and nothing it
   *  opens is taken for the steps' project. */
  guiding: boolean;
  /** The project the harness step holds behind it, while it does. */
  holding: string | undefined;
  /** Whether a project opening now is the one the steps asked for. */
  awaiting: boolean;
  dispatch: (event: Event) => void;
  /** Reads the kept place again, and looks at which harnesses this machine has. */
  reread: () => void;
  /** A project the steps asked for opened. */
  arrived: (made: Made) => void;
};

export function useOnboarding({
  gateOpen,
  adding,
}: {
  /** Whether the git gate holds an act until git is there. */
  gateOpen: boolean;
  /** Whether this window already holds a project: "Add another project", not a first run. */
  adding: boolean;
}): Onboarding {
  const [progress, setProgress] = useState<Progress>(FRESH);
  /** Whether the kept place has been read at least once, so nothing is kept before it is. */
  const [read, setRead] = useState(false);
  const [harnessMissing, setHarnessMissing] = useState(false);
  const [trouble, setTrouble] = useState<string>();

  // **The git check follows the gate**, adjusted as the gate changes rather than after it
  // (React's "adjusting state when a prop changes"): an act waiting at the project step is the
  // git check, and the gate closing — git there, or Not now — is the project again.
  const [gateWas, setGateWas] = useState(gateOpen);
  if (gateOpen !== gateWas) {
    setGateWas(gateOpen);
    setProgress((was) => next(was, { kind: gateOpen ? "git-missing" : "git-settled" }));
  }

  /** Bumped by every press, so a read that answers after the person moved on does not take
   *  them back: it only adds what the machine kept about having been through the steps. */
  const pressed = useRef(0);
  const reread = useCallback(() => {
    const at = pressed.current;
    void commands
      .onboardingProgress()
      .then((answer) => {
        // A place that could not be read is no place: the welcome, which is where a person
        // with nothing kept starts anyway.
        const kept = answer?.status === "ok" ? answer.data : null;
        if (kept)
          setProgress((was) =>
            pressed.current === at
              ? kept
              : { ...was, finished_once: was.finished_once || kept.finished_once },
          );
        setRead(true);
      })
      .catch(() => setRead(true));
    void commands
      .harnessSetupFound()
      .then((answer) => {
        if (answer?.status === "ok")
          setHarnessMissing(!answer.data.harnesses.some((row) => row.installed));
      })
      .catch(() => undefined);
  }, []);

  // Kept whole on every change, once the kept place has been read.
  const kept = JSON.stringify(progress);
  useEffect(() => {
    if (!read) return;
    void commands
      .keepOnboardingProgress(JSON.parse(kept) as Progress)
      .then((answer) => setTrouble(answer?.status === "error" ? answer.error : undefined))
      .catch((err: unknown) => setTrouble(String(err)));
  }, [kept, read]);

  const dispatch = useCallback((event: Event) => {
    pressed.current += 1;
    setProgress((was) => next(was, event));
  }, []);
  const arrived = useCallback(
    (made: Made) => dispatch({ kind: "arrived", made, harnessMissing }),
    [dispatch, harnessMissing],
  );

  const guiding = guides(progress, adding);
  return {
    progress,
    counted: stepsOf(progress, { harnessMissing }),
    harnessMissing,
    trouble,
    guiding,
    holding: progress.step === "harness" ? progress.made?.plane : undefined,
    awaiting: guiding && (progress.step === "project" || progress.step === "git"),
    dispatch,
    reread,
    arrived,
  };
}
