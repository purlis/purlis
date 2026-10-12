// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type {
  OnboardingDoor,
  OnboardingMade,
  OnboardingProgress,
  OnboardingStep,
} from "./bindings";

/**
 * **The guided steps, as a step machine** (#1817, spec #1810): welcome → sign in (when the
 * person took it) → create, get or open a project → the git check (when git is missing) → pick
 * your harness → open the project.
 *
 * Pure, so every way through is a test of its own (`guidedSteps.test.ts`); the window draws
 * what it says (`Onboarding.tsx`) and keeps it for the next launch (`useOnboarding.ts`, the
 * core's `onboarding.json`). **Nothing secret is ever part of it**: a sign-in resumed asks
 * again.
 */
export type Progress = OnboardingProgress;
export type Door = OnboardingDoor;
export type Made = OnboardingMade;

/** A step the person is counted through. `open` is the last, and is the project in front. */
export type Counted = OnboardingStep | "open";

/** Where a machine with nothing kept starts. */
export const FRESH: Progress = {
  step: "welcome",
  door: null,
  signing_in: false,
  git_missing: false,
  made: null,
  finished_once: false,
};

/** What moves the steps on. */
export type Event =
  /** A door of the welcome, or of the project step after a sign-in. */
  | { kind: "door"; door: Door }
  /** The welcome's Sign in to GitHub or GitLab. */
  | { kind: "sign-in" }
  /** Skip for now: past the sign-in to the project, or past the harness to the project open. */
  | { kind: "skip" }
  /** Back: from a door's page or the sign-in to the welcome. */
  | { kind: "back" }
  /** The act the person asked for needs git, and this machine has none. */
  | { kind: "git-missing" }
  /** Git is there now, or the person said Not now: back to the project's page. */
  | { kind: "git-settled" }
  /** A project the steps asked for opened. `harnessMissing` is what the machine said last. */
  | { kind: "arrived"; made: Made; harnessMissing: boolean }
  /** The harness step's way on: a harness installed or a chat asked for. */
  | { kind: "finish" }
  /** The project the harness step held could not be opened again. */
  | { kind: "start-over" };

/** Whether the harness step is one of this machine's: always the first time through, and after
 *  that only while no harness is installed (Q6). */
function harnessStep(progress: Progress, harnessMissing: boolean): boolean {
  return !progress.finished_once || harnessMissing;
}

/** The steps once the project is open: nothing kept but that this machine has been through
 *  them, so a finished run leaves no door, project or sign-in behind. */
const FINISHED: Progress = { ...FRESH, finished_once: true };

/** The steps after `event`. An event that does not apply where the person is changes nothing. */
export function next(progress: Progress, event: Event): Progress {
  switch (event.kind) {
    case "door":
      if (progress.step !== "welcome" && progress.step !== "project") return progress;
      return { ...progress, step: "project", door: event.door };
    case "sign-in":
      if (progress.step !== "welcome") return progress;
      return { ...progress, step: "sign-in", door: null, signing_in: true };
    case "skip":
      if (progress.step === "sign-in") return { ...progress, step: "project", door: null };
      if (progress.step === "harness") return FINISHED;
      return progress;
    case "back":
      if (progress.step === "sign-in")
        return { ...progress, step: "welcome", door: null, signing_in: false };
      if (progress.step === "project" || progress.step === "git")
        return { ...progress, step: "welcome", door: null };
      return progress;
    case "git-missing":
      if (progress.step !== "project") return progress;
      return { ...progress, step: "git", git_missing: true };
    case "git-settled":
      if (progress.step !== "git") return progress;
      return { ...progress, step: "project" };
    case "arrived":
      if (progress.step !== "project" && progress.step !== "git") return progress;
      if (!harnessStep(progress, event.harnessMissing)) return FINISHED;
      return { ...progress, step: "harness", made: event.made };
    case "finish":
      if (progress.step !== "harness") return progress;
      return FINISHED;
    case "start-over":
      return { ...FRESH, finished_once: progress.finished_once };
  }
}

/** The steps the person is counted through, and which one they are at: "step N of M" is
 *  `at + 1` of `steps.length`. A step that happened stays counted, so M never shrinks under
 *  the person; one that will not happen is never counted. */
export function stepsOf(
  progress: Progress,
  machine: { harnessMissing: boolean },
): { steps: Counted[]; at: number } {
  const steps: Counted[] = ["welcome"];
  if (progress.signing_in) steps.push("sign-in");
  steps.push("project");
  if (progress.git_missing) steps.push("git");
  if (progress.step === "harness" || harnessStep(progress, machine.harnessMissing))
    steps.push("harness");
  steps.push("open");
  return { steps, at: Math.max(0, steps.indexOf(progress.step)) };
}

/** What each step is called, in the counter and in the list of steps. */
export const STEP_NAMES: Record<Counted, string> = {
  welcome: "Welcome",
  "sign-in": "Sign in",
  project: "Create, get or open a project",
  git: "Install git",
  harness: "Pick your harness",
  open: "Open the project",
};
