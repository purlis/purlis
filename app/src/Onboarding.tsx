// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState, type ComponentProps } from "react";
import { DoorPage, Launcher } from "./DoorPage";
import { GitMissing } from "./GitMissing";
import type { GitGate } from "./gitPresence";
import { HarnessSetupTab } from "./HarnessSetupTab";
import { HARNESS_SETUP, type HarnessSetupAsk } from "./harnessSetup";
import { STEP_NAMES } from "./guidedSteps";
import type { Onboarding as Steps } from "./useOnboarding";
import { type Chose, type Door, Welcome } from "./Welcome";
import { Button } from "./ui/button/Button";

type WelcomeProps = ComponentProps<typeof Welcome>;

/** How long a window that already holds a project waits for the kept place before drawing its
 *  welcome anyway (#2400): far past one local read, and short of a wait anyone would notice. */
export const PLACE_READ_DEADLINE_MS = 400;

/**
 * **The guided steps** (#1817, spec #1810): what a window with no project in front draws, one
 * step at a time, each saying where it is — "Step 2 of 5" — over the list of them.
 *
 * 1. **Welcome**: the doors, the sign-in, the recents (`Welcome.tsx`).
 * 2. **Sign in**, when the person took it from the welcome, or when the act they asked for needs
 *    it (#1819: Create on GitHub or GitLab with no account there): skippable (Q2). The page is
 *    the welcome's own (`ForgeSignIn.tsx`, which #1814's token sign-in fills in); after one an
 *    act asked for, Continue and Back come back to that act's page.
 * 3. **Create, get or open a project**: the page behind the door (`FirstRun.tsx`,
 *    `GetProject.tsx`), or the doors again after a sign-in.
 * 4. **The git check**, only when the act asked for needs git and this machine has none
 *    (#1813's gate): Install, Check again, Not now; the act carries on once git is there.
 * 5. **Pick your harness**: the Set up a harness tab's own rows and installers (#622/#963),
 *    with the project the steps made held behind it — what is installed, Install for what is
 *    not, and the harness's own sign-in pointed to. **Skip for now** opens the project. Shown
 *    the first time through, and after that only while no harness is installed (Q6).
 * 6. **Open the project**: the project comes to the front, and the steps are done.
 *
 * Every step is a page whose heading takes the keyboard as it appears, and every act on it is a
 * button in the tab order. A quit at any step resumes there: the place is kept on this machine
 * (`useOnboarding.ts`).
 */
export function Onboarding({
  steps,
  gate,
  holdsMade,
  reopen,
  ...welcome
}: Omit<WelcomeProps, "door" | "onChose" | "withGit" | "onSignedIn"> & {
  steps: Steps;
  /** The window's git gate (#1813). */
  gate: GitGate;
  /** Whether the project the harness step holds is open in this window. */
  holdsMade: boolean;
  /** Opens a project again, for a harness step resumed at a launch that did not bring it back. */
  reopen: (path: string) => void;
}) {
  const { progress, counted, dispatch, reread } = steps;
  const { step } = progress;

  // Each time the steps appear, at the place this machine kept: a quit, or a project the person
  // went to and came back from, picks up where it was. The read is normally one IPC round trip;
  // one that has not answered by the deadline no longer holds the window (#2400).
  const [placeRead, setPlaceRead] = useState<"answered" | "late">();
  useEffect(() => {
    let gone = false;
    const late = setTimeout(() => {
      if (!gone) setPlaceRead((was) => was ?? "late");
    }, PLACE_READ_DEADLINE_MS);
    void reread().then(() => {
      if (!gone) setPlaceRead((was) => was ?? "answered");
    });
    return () => {
      gone = true;
      clearTimeout(late);
    };
  }, [reread]);
  // **Past the deadline, whether the steps guide this window is decided from what is known, and
  // held for this appearance**: a read that answers later still updates the place, but it does
  // not swap one welcome for the other under a door press.
  const [guidingHeld, setGuidingHeld] = useState<boolean>();
  if (placeRead === "late" && guidingHeld === undefined) setGuidingHeld(steps.guiding);
  const guiding = guidingHeld ?? steps.guiding;

  const chose = useCallback(
    (to: Chose) => {
      if (to === undefined) dispatch({ kind: "back" });
      else if (to === "sign-in") dispatch({ kind: "sign-in" });
      else if (to === "sign-in-first") dispatch({ kind: "needs-sign-in" });
      else dispatch({ kind: "door", door: to });
    },
    [dispatch],
  );
  const withGit = useCallback((act: () => unknown) => void gate.withGit(act), [gate]);
  const { onOpenRepo } = welcome;
  const openRepoWithGit = useCallback<WelcomeProps["onOpenRepo"]>(
    (path, template) => withGit(() => onOpenRepo(path, template)),
    [onOpenRepo, withGit],
  );

  /** The page the welcome draws: the sign-in, a door's, or its own. Kept under the git check,
   *  so the act that waits there carries on in the page it was asked from. */
  const door: Door | undefined =
    step === "sign-in"
      ? "sign-in"
      : (step === "project" || step === "git") &&
          (progress.door === "create" || progress.door === "get")
        ? progress.door
        : undefined;

  /** Whether a token signed in on the sign-in step, so its way on is Continue, not Skip. */
  const [signedIn, setSignedIn] = useState(false);

  // Back from the git check, the keyboard goes to the page the act was asked from: that page
  // stayed drawn, hidden, under the check, so its heading takes it again.
  const page = useRef<HTMLDivElement>(null);
  const [stepWas, setStepWas] = useState(step);
  const [fromGit, setFromGit] = useState(false);
  if (step !== stepWas) {
    setStepWas(step);
    setFromGit(stepWas === "git");
  }
  useEffect(() => {
    if (fromGit && step === "project")
      page.current?.querySelector<HTMLElement>("h1, button")?.focus();
  }, [fromGit, step]);

  // A window that already holds a project, before the kept place has answered (#2400): whether
  // the steps guide it is not known yet. A welcome drawn now could be swapped for the other one
  // under a door press, which drops the page the press opened. It is one small local read, waited on
  // for `PLACE_READ_DEADLINE_MS` at most.
  if (placeRead === undefined && welcome.adding) return null;

  // Not guiding (a window full of projects, on a machine that has been through the steps): the
  // plain welcome, still behind the git gate, with no step drawn and no door kept.
  if (!guiding) return <Welcome {...welcome} onOpenRepo={openRepoWithGit} withGit={withGit} />;

  // Where the person has got to: one line, on a first run only. Someone back on a machine that has
  // been through the steps has nothing to be counted through (the harness step still counts).
  const stepSaid = (!progress.finished_once || step === "harness") && (
    <nav className="onboarding-steps" aria-label="Setting up">
      <p className="onboarding-count">
        Step {counted.at + 1} of {counted.steps.length}: {STEP_NAMES[counted.steps[counted.at]]}
      </p>
    </nav>
  );

  return (
    <div className="onboarding">
      {steps.trouble !== undefined && (
        <p className="came-back">{steps.trouble}. The steps still work until purlis quits.</p>
      )}

      {step === "harness" && progress.made ? (
        <Launcher side={stepSaid}>
          <HarnessStep
            made={progress.made}
            here={holdsMade}
            trouble={welcome.trouble}
            reopen={reopen}
            onOn={() => dispatch({ kind: "finish" })}
            onSkip={() => dispatch({ kind: "skip" })}
            onStartOver={() => dispatch({ kind: "start-over" })}
          />
        </Launcher>
      ) : (
        <>
          {step === "git" && (
            <Launcher side={stepSaid}>
              <GitStep gate={gate} onSettled={() => dispatch({ kind: "git-settled" })} />
            </Launcher>
          )}
          <div ref={page} className="onboarding-page" hidden={step === "git"}>
            <Welcome
              {...welcome}
              onOpenRepo={openRepoWithGit}
              door={door}
              onChose={chose}
              withGit={withGit}
              onSignedIn={() => setSignedIn(true)}
              stepSaid={stepSaid}
              signInEnd={
                step === "sign-in" && (
                  <div className="doing">
                    <Button
                      variant={signedIn ? "primary" : "secondary"}
                      onPress={() => dispatch({ kind: "skip" })}
                    >
                      {signedIn ? "Continue" : "Skip for now"}
                    </Button>
                  </div>
                )
              }
            />
          </div>
        </>
      )}
    </div>
  );
}

/**
 * **Step 4, the git check** (#1813's guided screen, mounted): the act the person asked for waits
 * on the gate, and carries on by itself once git is there. A git check resumed after a quit has
 * no act waiting, so it looks again and goes back to the project's page, where the person asks
 * again; Not now goes back there too.
 */
function GitStep({ gate, onSettled }: { gate: GitGate; onSettled: () => void }) {
  const looked = useRef(false);
  useEffect(() => {
    if (looked.current || gate.open) return;
    looked.current = true;
    void gate.withGit(onSettled);
  }, [gate, onSettled]);
  return (
    <DoorPage title="Install git" className="git-step" onBack={gate.open ? gate.cancel : onSettled}>
      {!gate.open && (
        <p className="pending" aria-busy="true">
          Looking for git…
        </p>
      )}
      <GitMissing gate={gate} />
    </DoorPage>
  );
}

/**
 * **Step 5, Pick your harness**: the Set up a harness tab's rows, installers and Check again
 * (`HarnessSetupTab.tsx`, #622/#963), about the project the steps just made.
 *
 * Install and Start a chat are that tab's own asks of the project (`harnessSetup.ts`); either
 * one ends the steps, because the installer runs in a shell tab the person may have to answer,
 * and the chat starts in the project. Skip for now opens the project with neither.
 */
function HarnessStep({
  made,
  here,
  trouble,
  reopen,
  onOn,
  onSkip,
  onStartOver,
}: {
  made: { plane: string; cwd: string; workspace: string };
  /** Whether the project is open in this window, so its tab can be asked of. */
  here: boolean;
  /** Why the project did not open again, in the core's words. */
  trouble?: string;
  reopen: (path: string) => void;
  onOn: () => void;
  onSkip: () => void;
  onStartOver: () => void;
}) {
  // A harness step resumed at a launch that did not bring its project back opens it again,
  // once, through the trust gate like any open. A project the person closes from its tab
  // while the step holds it ends the steps instead: they are at the welcome again.
  const wasHere = useRef<boolean>(undefined);
  useEffect(() => {
    const before = wasHere.current;
    wasHere.current = here;
    if (before === undefined && !here) reopen(made.plane);
    else if (before === true && !here) onStartOver();
  }, [here, made.plane, onStartOver, reopen]);

  // The tab's asks go to the project; the steps end with either.
  useEffect(() => {
    const asked = (event: Event) => {
      if ((event as CustomEvent<HarnessSetupAsk>).detail.plane === made.plane) onOn();
    };
    window.addEventListener(HARNESS_SETUP, asked);
    return () => window.removeEventListener(HARNESS_SETUP, asked);
  }, [made.plane, onOn]);

  return (
    <DoorPage title="Pick your harness" className="harness-step">
      <p className="welcome-lede">Your project is ready. A chat in it runs a coding harness.</p>
      {here ? (
        <HarnessSetupTab
          plane={made.plane}
          cwd={made.cwd || made.plane}
          workspace={made.workspace}
        />
      ) : (
        <>
          {trouble === undefined && (
            <p className="pending" aria-busy="true">
              Opening your project…
            </p>
          )}
          {trouble !== undefined && (
            <>
              <p className="trouble said-in-full">{trouble}</p>
              <div className="doing">
                <Button onPress={onStartOver}>Start over</Button>
              </div>
            </>
          )}
        </>
      )}
      <div className="doing">
        <Button isDisabled={!here} onPress={onSkip}>
          Skip for now
        </Button>
      </div>
    </DoorPage>
  );
}
