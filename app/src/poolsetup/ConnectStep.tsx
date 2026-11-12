// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useState, type ReactNode } from "react";
import { KeyRound } from "lucide-react";
import { commands, type PlaneId, type PoolSetupShown, type PoolSetupStep } from "../bindings";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import { Card } from "../ui/card/Card";
import { Fact, Facts } from "../ui/card/Facts";
import { CheckLine, CheckList, type CheckVerdict } from "../ui/checks/Checks";
import { Radio, RadioGroup } from "../ui/choice/Radio";
import { CodeBlock } from "../ui/code/CodeBlock";
import { TextField } from "../ui/field/Field";
import { Action, Busy, Say, SeeWhat, StepHead } from "./parts";

/**
 * **Connect, the step of a pool on another machine** (#6266, ADR 0123 §3 and §6), as the accepted
 * prototype draws it (#6265): one page that grows. The machine is picked from the person's SSH
 * settings or typed; Connect to only reads the key the machine shows; the person checks it and
 * presses Trust this machine, or This isn't it; Docker there is checked, and each finding marks
 * its line of the check list with one trouble card. Every sentence of a state is the core's
 * (`purlis_core::poolsetup::connecting`), so this and `purlis pool setup` say the same.
 *
 * Only SSH is built: TLS, contexts and Podman are later tickets of #6257, so they aren't offered.
 */

/** What the person may be doing on the page, beside what the core read. */
type Pressing = "connecting" | "trusting" | "checking" | undefined;

/** What Connect checks, in order, and the causes that fail each line. */
const CHECKS: readonly { title: (host: string) => string; causes: readonly string[] }[] = [
  { title: (host) => `${host} is found`, causes: ["no-such-name"] },
  { title: (host) => `${host} answers`, causes: ["no-answer", "not-met"] },
  {
    title: () => "Its key is the one you trusted",
    causes: ["pool-pin-changed"],
  },
  { title: () => "It accepts your sign-in", causes: ["sign-in-refused"] },
  { title: () => "Docker answers there", causes: ["no-engine", "engine-silent"] },
  { title: () => "Your account may use Docker", causes: ["not-allowed"] },
  { title: () => "Its Docker is the one you trusted", causes: ["pool-engine-changed"] },
];

/** The trouble card's title for each cause. */
const TITLES: Record<string, string> = {
  "no-such-name": "purlis couldn't find that machine",
  "no-answer": "The machine didn't answer",
  "not-met": "SSH didn't answer there",
  "pool-pin-changed": "The machine shows a different key",
  "sign-in-refused": "The machine didn't accept your sign-in",
  "no-engine": "No Docker there that you can use",
  "engine-silent": "Docker didn't answer there",
  "not-allowed": "Your account can't use Docker there",
  "pool-engine-changed": "Docker there isn't the one you trusted",
  "not-a-route": "That isn't an address purlis can read",
  here: "That didn't work",
};

/** **Each check's verdict**: those before the failing line passed, the rest were not reached. */
export function verdictsOf(cause: string | null, done: boolean): CheckVerdict[] {
  const failing = cause === null ? -1 : CHECKS.findIndex((one) => one.causes.includes(cause));
  return CHECKS.map((_, at) =>
    failing === -1
      ? done
        ? "ok"
        : "todo"
      : at < failing
        ? "ok"
        : at === failing
          ? "missing"
          : "todo",
  );
}

/** The route a picked or typed machine is kept as: Docker's spelling. */
export function routeOf(typed: string): string {
  const text = typed.trim();
  return text === "" || text.includes("://") ? text : `ssh://${text}`;
}

function Asked({
  label,
  value,
  onChange,
  children,
}: {
  label: string;
  value: string;
  onChange: (to: string) => void;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section className="pool-setup-action">
      <h2 id={id} className="pool-setup-h2">
        {label}
      </h2>
      <RadioGroup aria-labelledby={id} value={value} onChange={onChange}>
        {children}
      </RadioGroup>
    </section>
  );
}

/** The machine picked, folded into its facts once purlis contacts it, with Change. */
function Chosen({ step, onChange }: { step: PoolSetupStep; onChange?: () => void }) {
  const machine = step.machine;
  return (
    <Card>
      <Facts aria-label="What this pool connects to">
        <Fact label="Machine">{machine?.host ?? "The machine"}</Fact>
        <Fact label="Reached by">SSH</Fact>
        {machine?.trusted && (
          <Fact label="Its key">
            <span className="pool-setup-choice">
              <Badge variant="ok">Trusted</Badge>
              {machine.trusted.fingerprint}
            </span>
          </Fact>
        )}
      </Facts>
      {onChange && (
        <Button variant="link" onPress={onChange}>
          Change
        </Button>
      )}
    </Card>
  );
}

/** **The pick**: the machines the person's SSH settings name, and another one typed. */
function Pick({
  step,
  hosts,
  busy,
  onPick,
  onConnect,
}: {
  step: PoolSetupStep;
  hosts: readonly string[];
  busy: boolean;
  onPick: (route: string | null) => void;
  onConnect: () => void;
}) {
  const kept = step.machine?.route ?? "";
  const keptHost = hosts.find((host) => routeOf(host) === kept);
  // Until the person picks, the pick follows what is kept and what the SSH settings name, which
  // are read after the step is drawn.
  const [chosen, setPicked] = useState<string>();
  const picked =
    chosen ?? keptHost ?? (kept === "" && hosts.length > 0 ? (hosts[0] ?? "typed") : "typed");
  const [typed, setTyped] = useState(
    hosts.some((host) => routeOf(host) === kept) ? "" : kept.replace(/^ssh:\/\//, ""),
  );
  const route = picked === "typed" ? routeOf(typed) : routeOf(picked);
  // What is picked is kept as it is picked, so coming back finds it. Nothing is contacted.
  useEffect(() => {
    if (route === kept) return;
    const later = setTimeout(() => onPick(route === "" ? null : route), 400);
    return () => clearTimeout(later);
  }, [route, kept, onPick]);
  const next = step.next;
  const host = step.machine?.host;
  return (
    <>
      {hosts.length > 0 ? (
        <Asked label="Which machine?" value={picked} onChange={setPicked}>
          {hosts.map((one) => (
            <Radio key={one} value={one} description="From your SSH settings">
              {one}
            </Radio>
          ))}
          <Radio value="typed" description="A machine that isn't in your SSH settings.">
            Another machine
          </Radio>
        </Asked>
      ) : null}
      {(hosts.length === 0 || picked === "typed") && (
        <TextField
          label="Its address"
          value={typed}
          onChange={setTyped}
          placeholder="you@build-box"
          help="The same address you'd give ssh: your account on it, an @ and its name. An alias from your SSH settings works too."
        />
      )}
      {step.see && <SeeWhat text={step.see.text} note={step.see.note} />}
      <Action
        does={next?.does}
        main={route !== "" && route === kept && host ? `Connect to ${host}` : "Connect"}
        onMain={onConnect}
        busy={busy || route === "" || route !== kept}
      />
    </>
  );
}

/** **The key the machine showed**, to check before it is trusted (#6265 choices 6 and 7). */
function Confirm({
  step,
  busy,
  onTrust,
  onNotIt,
}: {
  step: PoolSetupStep;
  busy: boolean;
  onTrust: (fingerprint: string) => void;
  onNotIt: () => void;
}) {
  const machine = step.machine;
  const key = machine?.key;
  if (!machine || !key) return null;
  const host = machine.host ?? "the machine";
  return (
    <>
      <h2 className="pool-setup-h2">Check it&apos;s the right machine</h2>
      {step.findings.map((finding) => (
        <p key={finding} className="pool-setup-does">
          {finding}
        </p>
      ))}
      <CodeBlock
        label={`${host}'s key, ${key.algorithm}`}
        text={key.fingerprint}
        copy="Copy fingerprint"
      />
      {step.see && (
        <SeeWhat open={machine.known !== true} note={step.see.note} text={step.see.text} />
      )}
      <Action
        does={step.next?.does}
        main="Trust this machine"
        reserve={["Trust this machine", "Continue"]}
        icon={<KeyRound aria-hidden size={16} />}
        busy={busy}
        onMain={() => onTrust(key.fingerprint)}
      >
        <Button variant="ghost" onPress={onNotIt} isDisabled={busy}>
          This isn&apos;t it
        </Button>
      </Action>
    </>
  );
}

/** **What was checked on the machine**, the failing line marked. */
function Checked({ step, done }: { step: PoolSetupStep; done: boolean }) {
  const host = step.machine?.host ?? "the machine";
  const verdicts = verdictsOf(step.cause, done);
  return (
    <>
      <h2 className="pool-setup-h2">{`Docker on ${host}`}</h2>
      <CheckList aria-label={`What purlis checked on ${host}`}>
        {CHECKS.map((check, at) => (
          <CheckLine
            key={check.title(host)}
            verdict={verdicts[at] ?? "todo"}
            title={check.title(host)}
          />
        ))}
      </CheckList>
    </>
  );
}

export function ConnectStep({
  step,
  shown,
  plane,
  onShown,
  changed,
  onNext,
}: {
  step: PoolSetupStep;
  shown: PoolSetupShown;
  plane: PlaneId;
  /** What a press of Connect to or Trust this machine read, with its finding laid over. */
  onShown: (shown: PoolSetupShown) => void;
  /** Something was kept: read again. */
  changed: () => void;
  onNext: () => void;
}) {
  const [hosts, setHosts] = useState<readonly string[]>([]);
  const [pressing, setPressing] = useState<Pressing>();
  const [trouble, setTrouble] = useState<string>();
  useEffect(() => {
    let live = true;
    void commands.poolSetupSshHosts().then((answer) => {
      if (live && answer.status === "ok") setHosts(answer.data);
    });
    return () => {
      live = false;
    };
  }, []);
  const host = step.machine?.host ?? "the machine";
  const pick = (route: string | null) => {
    void commands.poolSetupMachine(route).then((answer) => {
      if (answer.status === "error") setTrouble(answer.error);
      else changed();
    });
  };
  const connect = () => {
    setPressing("connecting");
    setTrouble(undefined);
    void commands.poolSetupConnect(plane).then((answer) => {
      setPressing(undefined);
      if (answer.status === "error") setTrouble(answer.error);
      else onShown(answer.data);
    });
  };
  /** A made pool's machine looked at again now, whatever wait a failing forward was in. */
  const checkAgain = () => {
    setPressing("checking");
    setTrouble(undefined);
    void commands.poolSetupCheckAgain(plane).then((answer) => {
      setPressing(undefined);
      if (answer.status === "error") setTrouble(answer.error);
      else onShown(answer.data);
    });
  };
  const trust = (fingerprint: string) => {
    setPressing("trusting");
    setTrouble(undefined);
    void commands.poolSetupTrust(plane, fingerprint).then((answer) => {
      setPressing(undefined);
      if (answer.status === "error") setTrouble(answer.error);
      else onShown(answer.data);
    });
  };
  const notIt = () => pick(null);
  const madePool = shown.pool !== null;
  const done = step.state === "done";
  const meeting = step.next?.id === "trust-machine";
  const refused = step.cause === "plain-tcp" || step.cause === "not-a-route";
  const finding = step.state === "attention" && !refused;

  return (
    <>
      <StepHead
        title={finding && madePool ? `Check ${host} again` : "Connect to the other machine"}
        tag={done ? <Badge variant="ok">Every check passed</Badge> : undefined}
        lead={
          done
            ? `${host} is ready to run your chats.`
            : "Choose the machine, and check it's the one you mean. purlis keeps no password or key of yours: SSH signs in with your own agent."
        }
      />
      {trouble && (
        <Say tone="trouble" title="That didn't work">
          <p className="pool-setup-p">{trouble}</p>
        </Say>
      )}
      {done || finding || meeting ? (
        <Chosen step={step} onChange={madePool ? undefined : notIt} />
      ) : null}
      {!done && !finding && !meeting && !refused && (
        <Pick
          step={step}
          hosts={hosts}
          busy={pressing !== undefined}
          onPick={pick}
          onConnect={connect}
        />
      )}
      {pressing === "connecting" && <Busy>{`Asking ${host} for its key. This only reads.`}</Busy>}
      {meeting && (
        <Confirm step={step} busy={pressing !== undefined} onTrust={trust} onNotIt={notIt} />
      )}
      {pressing === "trusting" && <Busy>{`Checking Docker on ${host}. This only reads.`}</Busy>}
      {pressing === "checking" && <Busy>{`Checking ${host} again. This only reads.`}</Busy>}
      {refused && (
        <Say
          tone="attention"
          title={
            step.cause === "plain-tcp"
              ? `purlis won't use ${host}'s Docker without TLS`
              : (TITLES[step.cause ?? "here"] ?? TITLES.here ?? "")
          }
          actions={
            <Button variant="primary" onPress={notIt}>
              {step.next?.label ?? "Change the machine"}
            </Button>
          }
        >
          {step.findings.map((one) => (
            <p key={one} className="pool-setup-p">
              {one}
            </p>
          ))}
        </Say>
      )}
      {(done || finding) && <Checked step={step} done={done} />}
      {finding && (
        <Say
          tone="trouble"
          title={TITLES[step.cause ?? "here"] ?? TITLES.here ?? ""}
          actions={
            <>
              <Button
                variant="primary"
                onPress={
                  step.next?.id === "change-machine" ? notIt : madePool ? checkAgain : connect
                }
              >
                {step.next?.id === "change-machine" ? "Change the machine" : "Check again"}
              </Button>
              {!madePool && step.next?.id !== "change-machine" && (
                <Button variant="secondary" onPress={notIt}>
                  Change the machine
                </Button>
              )}
            </>
          }
        >
          {step.findings.map((one) => (
            <p key={one} className="pool-setup-p">
              {one}
            </p>
          ))}
        </Say>
      )}
      {done && (
        <Action
          does={`Keeps ${host} as this pool's machine. Next, you choose its image and limits.`}
          main="Continue"
          reserve={["Trust this machine", "Continue"]}
          onMain={onNext}
        />
      )}
      {done && <SeeWhat text={step.see?.text ?? ""} note={step.see?.note} />}
    </>
  );
}
