// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
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
import { makeRunnerAgain, readReleaseImage, rereadPools, type ReleaseImage } from "./pools";
import { Button, ButtonRow } from "./ui/button/Button";
import { Card } from "./ui/card/Card";
import { TextField } from "./ui/field/Field";
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
 * This ticket builds Where and Pool. The later steps show what the core finds and what to do next,
 * and get their own screens in their tickets (#4482, #4483, #4490).
 */

/** How often the tab asks again while it waits on Docker. */
const RECHECK_MS = 5000;

/** How long typing rests before the name is kept. */
const KEEP_NAME_MS = 400;

/** A pool being made, held above the step so leaving it loses neither. */
type Adding = {
  making: boolean;
  refused?: string;
  set: (to: { making: boolean; refused?: string }) => void;
};

const STATES: readonly string[] = STEP_STATES;

function stateOf(step: PoolSetupStep): StepState {
  return STATES.includes(step.state) ? (step.state as StepState) : "later";
}

type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; shown: PoolSetupShown };

/** A step's heading and its plain sentences. Takes the focus as a step opens. */
function StepHead({ title, lead }: { title: string; lead?: ReactNode }) {
  const ref = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (document.activeElement?.closest('[data-scope="steps"]')) return;
    ref.current?.focus({ preventScroll: true });
  }, [title]);
  return (
    <header className="pool-setup-head">
      <h1 ref={ref} tabIndex={-1} className="pool-setup-title">
        {title}
      </h1>
      {lead && <p className="pool-setup-lead">{lead}</p>}
    </header>
  );
}

/** One main action, and what it will do, said before it is pressed (Q31). */
function Action({
  does,
  main,
  onMain,
  busy,
  icon,
}: {
  does?: string;
  main?: string;
  onMain?: () => void;
  busy?: boolean;
  icon?: ReactNode;
}) {
  return (
    <section className="pool-setup-action" aria-label="What to do next">
      {does && <p className="pool-setup-does">{does}</p>}
      {main && (
        <ButtonRow size="lg">
          <Button variant="primary" onPress={onMain} isDisabled={busy} aria-busy={busy} icon={icon}>
            {main}
          </Button>
        </ButtonRow>
      )}
    </section>
  );
}

/** A recheck that runs by itself, said once and politely (Q28). */
function Rechecking() {
  return (
    <p className="pool-setup-recheck" role="status">
      <Loader2 aria-hidden size={14} className="pool-setup-spin" />
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
  onNext,
  onChose,
  onRecheck,
  said,
}: {
  step: PoolSetupStep;
  onNext: () => void;
  onChose: () => void;
  onRecheck: () => void;
  said: (trouble: string) => void;
}) {
  const next = step.next;
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

const IMAGE_NOT_NAMED = "This build of purlis names no runner image, so give one by its digest.";

/** The Pool step: a name, and its own button. Image and limits come under Options with #4481. */
function PoolStep({
  step,
  shown,
  plane,
  adding,
  changed,
  onNext,
}: {
  step: PoolSetupStep;
  shown: PoolSetupShown;
  plane: PlaneId;
  adding: Adding;
  changed: () => void;
  onNext: () => void;
}) {
  const [name, setName] = useState(shown.name ?? "");
  const [image, setImage] = useState("");
  const [offered, setOffered] = useState<ReleaseImage>();
  useEffect(() => {
    if (shown.pool !== null) return;
    let live = true;
    void readReleaseImage().then((read) => {
      if (live) setOffered(read);
    });
    return () => {
      live = false;
    };
  }, [shown.pool]);
  // Kept as it is typed, so a tab closed mid-word comes back with it. A name no pool could have
  // yet is left unkept, and Create says why.
  useEffect(() => {
    if (shown.pool !== null || name === (shown.name ?? "")) return;
    const later = setTimeout(() => {
      const kept = name.trim();
      void commands.poolSetupName(kept === "" ? null : kept);
    }, KEEP_NAME_MS);
    return () => clearTimeout(later);
  }, [name, shown.name, shown.pool]);

  if (shown.pool !== null) {
    const action = step.next;
    return (
      <>
        <StepHead title={`Pool ${shown.pool}`} />
        {step.state === "attention" ? (
          <Findings step={step} />
        ) : (
          <p className="pool-setup-p">
            Pool {shown.pool} is made, and its runner is on this machine's Docker.
          </p>
        )}
        {adding.refused && (
          <Notice
            cause="pool-setup.pool"
            tone="trouble"
            title="That didn't work"
            onDismiss={() => adding.set({ making: false })}
          >
            {adding.refused}
          </Notice>
        )}
        {action?.id === "make-runner-again" ? (
          <Action
            does={action.does}
            main={action.label}
            busy={adding.making}
            onMain={() => {
              adding.set({ making: true });
              void makeRunnerAgain(shown.pool ?? "").then((why) => {
                adding.set({ making: false, refused: why });
                changed();
              });
            }}
          />
        ) : action?.id === "check-again" ? (
          <Action does={action.does} main={action.label} onMain={changed} />
        ) : action ? (
          <Action does={action.does} />
        ) : (
          <Action main="Continue" onMain={onNext} />
        )}
      </>
    );
  }

  const named = offered && "image" in offered ? offered.image : undefined;
  const reading = offered === undefined;
  const newer = offered?.state === "newer" ? offered : undefined;
  const trouble = offered?.state === "trouble" ? offered.trouble : undefined;
  const create = async () => {
    const kept = name.trim();
    const using = image.trim() || named || "";
    if (kept === "") return adding.set({ making: false, refused: "Give the pool a name first." });
    if (using === "") return adding.set({ making: false, refused: IMAGE_NOT_NAMED });
    adding.set({ making: true });
    // Docker may have changed since this step was read: what is made is held to now.
    const now = await commands.poolSetup(plane);
    const where =
      now.status === "ok" ? now.data.steps.find((one) => one.id === "where") : undefined;
    if (where?.state !== "done") {
      adding.set({ making: false });
      changed();
      return;
    }
    const keptName = await commands.poolSetupName(kept);
    if (keptName.status === "error") return adding.set({ making: false, refused: keptName.error });
    const answer = await commands.addDockerPool({
      name: kept,
      image: using,
      cpus: null,
      memory_mib: null,
      processes: null,
      tmp_mib: null,
      suspend_after: null,
      frames: null,
      width: null,
    });
    adding.set({
      making: false,
      refused: answer.status === "error" ? answer.error : undefined,
    });
    // The rest of the window lists the new pool too.
    await rereadPools();
    changed();
  };
  return (
    <>
      <StepHead
        title="Name your pool"
        lead="Pick any short name you'll recognise. The rest is already set to what we recommend."
      />
      {adding.making ? (
        <p className="pool-setup-busy" role="status">
          <Loader2 aria-hidden size={18} className="pool-setup-spin" />
          <span>Making the pool and its runner. The first time, this downloads the runner.</span>
        </p>
      ) : (
        <>
          <TextField
            label="Name"
            value={name}
            onChange={setName}
            help="Small letters, digits and dashes."
          />
          {!reading && named === undefined && (
            <TextField
              label="Runner image"
              value={image}
              onChange={setImage}
              help={trouble ?? IMAGE_NOT_NAMED}
            />
          )}
          {named && (
            <p className="pool-setup-does">
              Runner image: {named}
              {newer && `, from purlis ${newer.version}. An update to it is waiting.`}
            </p>
          )}
          {adding.refused && (
            <Notice
              cause="pool-setup.pool"
              tone="trouble"
              title="The pool wasn't made"
              onDismiss={() => adding.set({ making: false })}
            >
              {adding.refused}
            </Notice>
          )}
        </>
      )}
      <Action
        does={step.next?.does}
        main={step.next?.label ?? "Create the pool"}
        busy={adding.making || reading}
        onMain={() => void create()}
      />
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
      {step.next?.command && <code className="pool-setup-command">{step.next.command}</code>}
    </>
  );
}

export function PoolSetupTab({ plane }: { plane: PlaneId }) {
  const [read, setRead] = useState<Read>({ state: "reading" });
  const [showing, setShowing] = useState<string>();
  const [trouble, setTrouble] = useState<string>();
  const [made, setMade] = useState<{ making: boolean; refused?: string }>({ making: false });
  const reads = useRef(0);

  const reread = useCallback(() => {
    const mine = ++reads.current;
    void commands.poolSetup(plane).then((answer) => {
      if (mine !== reads.current) return;
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

  const shown = read.state === "read" ? read.shown : undefined;
  const at = shown?.at ?? null;
  // What only the outside world can change: Docker starting, a runner being made.
  const waiting =
    (at === "where" || at === "pool") &&
    shown?.steps.find((step) => step.id === at)?.state === "attention";
  useEffect(() => {
    if (!waiting) {
      setTrouble(undefined);
      return;
    }
    const every = setInterval(reread, RECHECK_MS);
    return () => clearInterval(every);
  }, [waiting, reread]);

  if (read.state === "reading")
    return (
      <div className="pool-setup-alone">
        <p className="pool-setup-busy" role="status">
          <Loader2 aria-hidden size={18} className="pool-setup-spin" />
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
          {step?.id === "where" ? (
            <Where
              step={step}
              said={setTrouble}
              onNext={() => after("where")}
              onRecheck={reread}
              onChose={() => {
                setTrouble(undefined);
                setShowing(undefined);
                void commands.poolSetupChooseDocker().then((answer) => {
                  if (answer.status === "error") setTrouble(answer.error);
                  reread();
                });
              }}
            />
          ) : step?.id === "pool" ? (
            <PoolStep
              step={step}
              shown={setup}
              plane={plane}
              adding={{ ...made, set: setMade }}
              onNext={() => after("pool")}
              changed={() => {
                setShowing(undefined);
                reread();
              }}
            />
          ) : (
            step && <Found step={step} onRecheck={reread} />
          )}
        </div>
      </div>
    </div>
  );
}
