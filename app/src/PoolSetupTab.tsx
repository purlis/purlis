// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, Loader2 } from "lucide-react";
import {
  commands,
  type PlaneId,
  type PoolSetupAction,
  type PoolSetupShown,
  type PoolSetupStep,
} from "./bindings";
import { Notice } from "./Notice";
import { Action, Recommended, Say, SeeWhat, StepHead } from "./poolsetup/parts";
import { ConnectStep } from "./poolsetup/ConnectStep";
import { IdentityStep } from "./poolsetup/IdentityStep";
import { NOTHING_CHOSEN, PoolStep, type Chosen } from "./poolsetup/PoolStep";
import { SignInsStep } from "./poolsetup/SignInsStep";
import { TryStep } from "./poolsetup/TryStep";
import { startANewPoolSetup, startPoolSetupOver, usePoolSetupStart } from "./pools";
import { Button } from "./ui/button/Button";
import { Card } from "./ui/card/Card";
import { OptionCard, OptionCards } from "./ui/choice/Radio";
import { Server } from "lucide-react";
import { STEP_STATES, StepList, type StepState } from "./ui/steps/Steps";

/**
 * **Pool setup** (spec #4469, #4480): the view tab that takes anyone from nothing to a working chat
 * on a pool, as the accepted prototype draws it: the step list on the left, one step with one main
 * action on the right.
 *
 * **What it shows, the core reads** (`purlis_core::poolsetup`): each step's state, what purlis found
 * and what to do next, read again as the tab opens, after each press, and every few seconds while
 * it waits on Docker (Q28). The sentences that frame a step are the tab's; a state's are the core's,
 * so the tab and `purlis pool setup` say the same.
 *
 * **Nothing changes but on a step's own button** (Q31), and each says what it will do before it is
 * pressed. Choosing Docker and the name given are kept as they are made, so coming back resumes.
 *
 * **Each step has two more levels** (#4481, Q30): Options, with every setting and the recommended
 * value marked, and See what this does, with the commands the button runs ({@link SeeWhat}). A pool
 * made by hand under the name typed is noticed, and setup follows it.
 *
 * **Add a pool… starts a new pool's setup** (#6256): from its first step, the name empty and one
 * no pool has recommended. A new pool's setup started and not finished is gone on with instead,
 * and the tab says so, with Start over. A finished pool's setup is reached only from its own Open
 * setup, and its last screen offers Add another pool. Each start draws the tab from its first
 * read ({@link usePoolSetupStart}).
 *
 * Every step has its screen. Agent identity sets one up with the flow the Agent identity tab uses
 * (#4490). Try it ends setup on a real chat started on the pool (#4483), and its last screen opens
 * a chat there in the window's own picker.
 */

/** How often the tab asks again while it waits on Docker. */
const RECHECK_MS = 5000;

const STATES: readonly string[] = STEP_STATES;

function stateOf(step: PoolSetupStep): StepState {
  return STATES.includes(step.state) ? (step.state as StepState) : "later";
}

type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; shown: PoolSetupShown };

/** A recheck that runs by itself, said once and politely (Q28). */
function Rechecking() {
  return (
    <p className="pool-setup-recheck" role="status">
      <Loader2 aria-hidden size={14} className="spinning" />
      <span>Checking again by itself every few seconds.</span>
    </p>
  );
}

function Findings({ step }: { step: PoolSetupStep }) {
  return (
    <>
      {step.findings.map((finding) => (
        <p key={finding} className="pool-setup-p">
          {finding}
        </p>
      ))}
    </>
  );
}

const WHERE_TITLE = "Where should your chats run?";
const WHERE_LEAD =
  "Right now your chats run directly on this machine. A pool gives each chat a separate machine, so it can work on its own.";

/** The Where step: what purlis found of Docker, and the one thing to do about it. */
function Where({
  step,
  kind,
  onNext,
  onChose,
  onChoseRemote,
  onRecheck,
  said,
}: {
  step: PoolSetupStep;
  /** Where the chats of this setup run: `docker` or `remote-docker` (#6266). */
  kind: string;
  onNext: () => void;
  onChose: () => void;
  onChoseRemote: () => void;
  onRecheck: () => void;
  said: (trouble: string) => void;
}) {
  const [picking, pick] = useState(kind === "remote-docker" ? "remote" : "local");
  const next = step.next;
  // **This machine, or another one with Docker** (ADR 0123 §6, #6265): offered while there is no
  // pool, the core's own next for each.
  if (step.other && step.state !== "attention") {
    const local = kind === "docker" ? next : step.other;
    const remote = kind === "remote-docker" ? next : step.other;
    const chosen = picking === "remote" ? remote : local;
    const here = kind === "docker" && step.state !== "attention";
    // The core's own words for each way on, and Continue where it is already the one chosen.
    const chosenAlready =
      step.state === "done" && picking === (kind === "docker" ? "local" : "remote");
    const mainLabel = chosenAlready ? "Continue" : (chosen?.label ?? "Continue");
    return (
      <>
        <StepHead title={WHERE_TITLE} lead={WHERE_LEAD} />
        <OptionCards aria-label="Where your chats run" value={picking} onChange={pick}>
          <OptionCard
            value="local"
            icon={<Server size={18} />}
            title={
              <span className="pool-setup-choice">This machine {here && <Recommended />}</span>
            }
            detail="Docker on this machine runs your chats, each in its own container."
          />
          <OptionCard
            value="remote"
            icon={<Server size={18} />}
            title={<span className="pool-setup-choice">Another machine with Docker</span>}
            detail="A second Mac, a Linux box or a server, reached over SSH. Your chats keep running there while this machine sleeps."
          />
        </OptionCards>
        {step.see && <SeeWhat text={step.see.text} note={step.see.note} />}
        <Action
          does={
            step.state === "done" && picking === (kind === "docker" ? "local" : "remote")
              ? undefined
              : chosen?.does
          }
          main={mainLabel}
          reserve={[local?.label, remote?.label, "Continue"].filter(
            (one): one is string => one !== undefined,
          )}
          onMain={() => {
            if (picking === "remote") {
              if (kind === "remote-docker" && step.state === "done") onNext();
              else onChoseRemote();
            } else if (kind === "docker" && step.state === "done") onNext();
            else onChose();
          }}
        />
      </>
    );
  }
  const press = (action: PoolSetupAction) => {
    if (action.url) void openUrl(action.url);
    else if (action.id === "start-docker") {
      void commands.poolSetupOpenDocker().then((answer) => {
        if (answer.status === "error") said(answer.error);
      });
    } else if (action.id === "use-docker") onChose();
    else onRecheck();
  };
  // Nothing here can do it for the person: the words say what to do, and the recheck notices.
  const onlySaid = next?.id === "use-local-docker";
  return (
    <>
      <StepHead title={WHERE_TITLE} lead={WHERE_LEAD} />
      <div className="pool-setup-terms">
        <p className="pool-setup-term">
          <strong>A pool</strong> is the place your chats run instead of directly on this machine.
        </p>
        <p className="pool-setup-term">
          <strong>A runner</strong> is one separate machine in that pool. A chat there can only
          touch what you send to it.
        </p>
      </div>
      {step.state === "attention" ? (
        <Card>
          <Findings step={step} />
          <Rechecking />
        </Card>
      ) : step.state === "done" ? (
        <p className="pool-setup-p">
          <strong>Chats will run on:</strong> {step.line}.
        </p>
      ) : (
        <>
          <Findings step={step} />
          <p className="pool-setup-p">
            <strong>Why Docker:</strong> it is already running here, so you can have a working chat
            in a few minutes.
          </p>
        </>
      )}
      {step.see && <SeeWhat text={step.see.text} note={step.see.note} />}
      {next && onlySaid ? (
        <Action does={next.does} />
      ) : next ? (
        <Action
          does={next.does}
          main={next.label}
          icon={next.url ? <ExternalLink aria-hidden size={14} /> : undefined}
          onMain={() => press(next)}
        />
      ) : (
        <Action main="Continue" onMain={onNext} />
      )}
    </>
  );
}

/** A later step, whose own screen comes with its ticket: what the core finds, and what to do. */
function Found({ step, onRecheck }: { step: PoolSetupStep; onRecheck: () => void }) {
  return (
    <>
      <StepHead title={step.name} />
      <Findings step={step} />
      {step.next?.id === "check-again" ? (
        <Action does={step.next.does} main={step.next.label} onMain={onRecheck} />
      ) : (
        step.next && <Action does={step.next.does} />
      )}
      {step.see && <SeeWhat text={step.see.text} note={step.see.note} />}
    </>
  );
}

type TabProps = {
  plane: PlaneId;
  /** The workspace the tab is filed in, where it is in one: where Try it starts its chat. */
  workspace?: string;
  /** Opens the picker for a new chat on pool `pool`, chosen on its Runs on row. */
  onOpenChat?: (pool: string, workspace?: string) => void;
  /** Opens the Agent identity tab, where an identity is inspected and removed. */
  onOpenIdentity?: () => void;
  /** Opens the project's Settings, where a persona's tool is allowed. */
  onOpenSettings?: () => void;
};

export function PoolSetupTab(props: TabProps) {
  const start = usePoolSetupStart();
  return <PoolSetupGuide key={start.count} {...props} continuing={start.continuing} />;
}

function PoolSetupGuide({
  plane,
  workspace,
  onOpenChat,
  onOpenIdentity,
  onOpenSettings,
  continuing,
}: TabProps & {
  /** Add went on with a new pool's unfinished setup: the tab says so, with Start over. */
  continuing: boolean;
}) {
  const [read, setRead] = useState<Read>({ state: "reading" });
  const [showing, setShowing] = useState<string>();
  const [trouble, setTrouble] = useState<string>();
  const [made, setMade] = useState<{
    making: boolean;
    refused?: string;
    breaks?: readonly string[];
  }>({ making: false });
  const [typed, setTyped] = useState<string>();
  const [chosen, choose] = useState<Chosen>(NOTHING_CHOSEN);
  const reads = useRef(0);
  /** The read on its way, so a slow Docker is waited for and never asked twice at once. */
  const reading = useRef(0);

  const reread = useCallback(() => {
    const mine = ++reads.current;
    reading.current = mine;
    void commands.poolSetup(plane).then((answer) => {
      if (reading.current === mine) reading.current = 0;
      if (mine !== reads.current) return;
      // A refusal of a pool that stands now is over.
      if (answer.status === "ok" && answer.data.pool !== null) {
        const step = answer.data.steps.find((one) => one.id === "pool");
        if (step?.state !== "attention")
          setMade((was) => (was.refused ? { making: was.making } : was));
      }
      // What went wrong starting Docker is over once Docker needs nothing.
      if (answer.status === "ok" && answer.data.steps[0]?.state !== "attention")
        setTrouble(undefined);
      setRead(
        answer.status === "ok"
          ? { state: "read", shown: answer.data }
          : { state: "trouble", trouble: answer.error },
      );
    });
  }, [plane]);

  useEffect(() => {
    reread();
    return () => {
      reads.current++;
    };
  }, [reread]);
  // Work done elsewhere is picked up as the person comes back to the window (Q30): a command
  // run from See what this does, in their own terminal.
  useEffect(() => {
    const back = () => {
      if (document.visibilityState === "visible" && reading.current === 0) reread();
    };
    window.addEventListener("focus", back);
    document.addEventListener("visibilitychange", back);
    return () => {
      window.removeEventListener("focus", back);
      document.removeEventListener("visibilitychange", back);
    };
  }, [reread]);

  const shown = read.state === "read" ? read.shown : undefined;
  const at = shown?.at ?? null;
  // What only the outside world can change: Docker starting, a runner being made.
  const waiting =
    (at === "where" || at === "pool") &&
    shown?.steps.find((step) => step.id === at)?.state === "attention";
  useEffect(() => {
    if (!waiting) return;
    const every = setInterval(() => {
      if (reading.current === 0) reread();
    }, RECHECK_MS);
    return () => clearInterval(every);
  }, [waiting, reread]);

  if (read.state === "reading")
    return (
      <div className="pool-setup-alone">
        <p className="pool-setup-busy" role="status">
          <Loader2 aria-hidden size={18} className="spinning" />
          <span>Looking at what this machine has. This only reads.</span>
        </p>
      </div>
    );
  if (read.state === "trouble")
    return (
      <div className="pool-setup-alone">
        <Notice
          cause="pool-setup.read"
          tone="trouble"
          title="purlis couldn't read Pool setup"
          fixes={[{ label: "Check again", onPress: reread, primary: true }]}
        >
          {read.trouble}
        </Notice>
      </div>
    );

  const { shown: setup } = read;
  const madePool = setup.pool;
  const here = showing ?? at ?? setup.steps[setup.steps.length - 1]?.id ?? null;
  const step = setup.steps.find((one) => one.id === here);
  const after = (id: string) => {
    const was = setup.steps.findIndex((one) => one.id === id);
    const to = setup.steps[was + 1];
    setShowing(to && stateOf(to) !== "later" ? to.id : undefined);
  };
  return (
    <div className="pool-setup">
      <StepList
        aria-label="Steps of Pool setup"
        className="pool-setup-rail"
        steps={setup.steps.map((one) => ({
          id: one.id,
          name: one.name,
          state: stateOf(one),
          line: one.line,
        }))}
        showing={here}
        onGo={(id) => setShowing(id)}
      >
        <p className="pool-setup-saved">
          Your choices are saved as you go. Close this tab whenever you like.
        </p>
      </StepList>
      <div className="pool-setup-main">
        <div className="pool-setup-page">
          {trouble && (
            <Notice
              cause="pool-setup.where"
              tone="trouble"
              title="That didn't work"
              onDismiss={() => setTrouble(undefined)}
            >
              {trouble}
            </Notice>
          )}
          {continuing && setup.at !== null && (
            <Say
              tone="news"
              title="Continuing the pool you started"
              actions={
                <Button
                  onPress={() => {
                    setTrouble(undefined);
                    void startPoolSetupOver().then((why) => why && setTrouble(why));
                  }}
                >
                  Start over
                </Button>
              }
            >
              {madePool === null
                ? "Your choices so far are filled in. Start over to begin a new pool from the first step."
                : `Start over to begin a new pool from the first step. Pool ${madePool} stays, in Settings, under Pools.`}
            </Say>
          )}
          {step?.id === "where" ? (
            <Where
              step={step}
              kind={setup.kind}
              onChoseRemote={() => {
                setTrouble(undefined);
                setShowing(undefined);
                void commands.poolSetupChooseRemoteDocker().then((answer) => {
                  if (answer.status === "error") setTrouble(answer.error);
                  else reread();
                });
              }}
              said={setTrouble}
              onNext={() => after("where")}
              onRecheck={reread}
              onChose={() => {
                setTrouble(undefined);
                setShowing(undefined);
                void commands.poolSetupChooseDocker().then((answer) => {
                  if (answer.status === "error") setTrouble(answer.error);
                  else reread();
                });
              }}
            />
          ) : step?.id === "connect" ? (
            <ConnectStep
              step={step}
              shown={setup}
              plane={plane}
              onShown={(now) => setRead({ state: "read", shown: now })}
              changed={() => {
                setShowing(undefined);
                reread();
              }}
              onNext={() => after("connect")}
            />
          ) : step?.id === "pool" ? (
            <PoolStep
              step={step}
              shown={setup}
              plane={plane}
              adding={{ ...made, set: setMade, typed, type: setTyped, chosen, choose }}
              onNext={() => after("pool")}
              changed={() => {
                setShowing(undefined);
                reread();
              }}
            />
          ) : step?.id === "signins" && setup.pool !== null ? (
            <SignInsStep
              key={setup.pool}
              step={step}
              pool={setup.pool}
              plane={plane}
              changed={() => {
                setShowing(undefined);
                reread();
              }}
            />
          ) : step?.id === "identity" && madePool !== null ? (
            <IdentityStep
              key={madePool}
              step={step}
              pool={madePool}
              plane={plane}
              changed={() => {
                setShowing(undefined);
                reread();
              }}
              onNext={() => after("identity")}
              onOpenIdentity={onOpenIdentity}
              onOpenSettings={onOpenSettings}
            />
          ) : step?.id === "try" && madePool !== null ? (
            <TryStep
              key={madePool}
              step={step}
              pool={madePool}
              plane={plane}
              workspace={workspace}
              changed={() => {
                setShowing(undefined);
                reread();
              }}
              onGo={(id) => setShowing(id)}
              onAddAnother={() => void startANewPoolSetup()}
              onOpenChat={(triedIn) => onOpenChat?.(madePool, triedIn ?? workspace)}
            />
          ) : (
            step && <Found step={step} onRecheck={reread} />
          )}
        </div>
      </div>
    </div>
  );
}
