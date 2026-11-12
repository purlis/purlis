// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch `prototype/pool-setup-remote-docker`, #6265, ADR 0123 §6). Never
 * merges to main.
 *
 * The question: can someone who has never set up Docker on another machine get through Connect
 * alone, in every state ADR 0123 names? One design, not variants: #4469's accepted prototype fixed
 * the layout (step list on the left, one step with one main action on the right), and this draws
 * the one new step in it, from the real step parts (`../parts`) and the families in `app/src/ui`.
 *
 * Everything is stand-in (`data.ts`). Where a screen waits on the other machine, it answers by
 * itself after a moment, with what the prototype's own strip (marked PROTOTYPE) says to pretend.
 */
import { useCallback, useEffect, useId, useState, type ReactNode } from "react";
import type { RadioGroupProps } from "../../ui/choice/Radio";
import { ExternalLink, KeyRound, Server } from "lucide-react";
import { Notice } from "../../Notice";
import { Button } from "../../ui/button/Button";
import { Badge } from "../../ui/badge/Badge";
import { Card } from "../../ui/card/Card";
import { Fact, Facts } from "../../ui/card/Facts";
import { CheckLine, CheckList, type CheckVerdict } from "../../ui/checks/Checks";
import { Checkbox } from "../../ui/choice/Checkbox";
import { OptionCard, OptionCards, Radio, RadioGroup } from "../../ui/choice/Radio";
import { CodeBlock } from "../../ui/code/CodeBlock";
import { TextField } from "../../ui/field/Field";
import { Select } from "../../ui/select/Select";
import { StepList, type StepState } from "../../ui/steps/Steps";
import { Action, Busy, Options, Recommended, Say, SeeWhat, StepHead } from "../parts";
import {
  CHECKS,
  ENGINE_ID,
  FINDINGS,
  FOUND,
  AT,
  HOST_KEY,
  TLS_CA,
  said,
  type FindingId,
  type Found,
  type Route,
} from "./data";

// --- The prototype's state ---------------------------------------------------------------------

type Phase =
  /** Route and machine; nothing has been contacted. */
  | "pick"
  /** A plain `tcp://` engine, refused. */
  | "refused"
  /** Asking the machine for its key: only reads. */
  | "reading"
  /** The key (or the CA) to confirm, and Trust this machine. */
  | "confirm"
  /** The engine check, running. */
  | "checking"
  /** The engine check found something. */
  | "found"
  /** Every check passed: the owned mark, and Continue. */
  | "owned";

type Screen = "where" | "connect" | "pool" | "reopened";

/** What the pool reopened with: the pin-changed Notice, in its three causes, or doctor's context
 *  finding. */
export type Reopened = "machine" | "ca" | "engine" | "context";

export type WhereCase = "docker-here" | "no-docker" | "elsewhere";

export type State = {
  screen: Screen;
  where: WhereCase;
  kind: "local" | "remote";
  route: Route;
  /** A found machine's id, or `typed`. */
  machine: string;
  typed: string;
  phase: Phase;
  /** Whether the person's own ssh already knows a key for the host. */
  known: boolean;
  /** What the engine check pretends to find. */
  pretend: "pass" | FindingId;
  finding?: FindingId;
  owned: "not" | "mine";
  reopened?: Reopened;
  /** Connect run again from the pin-changed Notice: the old key beside the new. */
  retrust?: Reopened;
  /** Holds a waiting screen still, so its story can be looked at. */
  hold?: boolean;
  optionsOpen?: boolean;
};

export const START: State = {
  screen: "where",
  where: "docker-here",
  kind: "local",
  route: "ssh",
  machine: "build-box",
  typed: "",
  phase: "pick",
  known: true,
  pretend: "pass",
  owned: "not",
};

// --- What a choice resolves to -----------------------------------------------------------------

type Machine = {
  name: string;
  host: string;
  user: string;
  port: number;
  address: string;
  /** The route the pool keeps: a context or connection becomes one of these. */
  over: "ssh" | "tls";
  plainTcp: boolean;
  local: boolean;
  found?: Found;
};

function parseTyped(typed: string, route: Route): Machine | undefined {
  const text = typed.trim();
  if (text === "") return undefined;
  const tcp = /^tcp:\/\/([^:/]+)(?::(\d+))?/.exec(text);
  if (tcp) {
    const port = Number(tcp[2] ?? 2375);
    return {
      name: tcp[1],
      host: tcp[1],
      user: "sam",
      port,
      address: `tcp://${tcp[1]}:${port}`,
      over: "tls",
      plainTcp: route !== "tls" || port === 2375,
      local: false,
    };
  }
  const ssh = /^(?:ssh:\/\/)?(?:([^@]+)@)?([^:/]+)(?::(\d+))?/.exec(text);
  if (!ssh) return undefined;
  const host = ssh[2];
  if (route === "tls") {
    const port = Number(ssh[3] ?? 2376);
    return {
      name: host,
      host,
      user: "sam",
      port,
      address: `tcp://${host}:${port}`,
      over: "tls",
      plainTcp: port === 2375,
      local: false,
    };
  }
  const user = ssh[1] ?? "sam";
  const port = Number(ssh[3] ?? 22);
  return {
    name: host,
    host,
    user,
    port,
    address: `ssh://${user}@${host}${port === 22 ? "" : `:${port}`}`,
    over: "ssh",
    plainTcp: false,
    local: false,
  };
}

function machineOf(state: State): Machine | undefined {
  if (state.machine === "typed" || state.route === "tls")
    return parseTyped(state.typed, state.route);
  const found = FOUND[state.route].find((one) => one.id === state.machine);
  if (!found) return undefined;
  return {
    name: found.name,
    host: found.host,
    user: found.user,
    port: found.port,
    address: found.address,
    over: found.becomes ?? "ssh",
    plainTcp: found.not?.kind === "plain-tcp",
    local: found.not?.kind === "local",
    found,
  };
}

const ROUTE_NAMES: Record<Route, string> = {
  ssh: "SSH",
  tls: "Docker's API over TLS",
  context: "A Docker context",
  podman: "A Podman connection",
};

// --- The prototype's own strip, outside the design ---------------------------------------------

function PrototypeStrip({ state, set }: { state: State; set: (to: Partial<State>) => void }) {
  return (
    <Card variant="ask" aria-label="Prototype controls">
      <p className="pool-setup-does">
        <Badge variant="waiting">Prototype</Badge> Not part of the design. Pretend what the other
        machine does.
      </p>
      <div className="pool-setup-grid">
        <Select
          aria-label="When purlis checks the engine, pretend it finds"
          value={state.pretend}
          onValueChange={(to) => set({ pretend: to as State["pretend"] })}
          options={[
            { value: "pass", label: "Everything passes" },
            ...FINDINGS.map((id) => ({
              value: id,
              label: `Finds: ${said(id, "host", "sam", 22).title}`,
            })),
          ]}
          size="sm"
        />
        <Checkbox isSelected={state.known} onChange={(known) => set({ known })}>
          My ssh already knows this machine&apos;s key
        </Checkbox>
      </div>
    </Card>
  );
}

/** A radio group under its own heading, which names it. */
function Asked({ label, ...group }: RadioGroupProps & { label: string }) {
  const id = useId();
  return (
    <section className="pool-setup-action">
      <h2 id={id} className="pool-setup-h2">
        {label}
      </h2>
      <RadioGroup {...group} aria-labelledby={id} />
    </section>
  );
}

// --- Where -------------------------------------------------------------------------------------

function Where({ state, set }: { state: State; set: (to: Partial<State>) => void }) {
  const counts = `purlis found ${FOUND.ssh.length} SSH hosts, ${FOUND.context.length - 1} Docker contexts and ${FOUND.podman.length} Podman connection in your settings.`;
  if (state.where === "elsewhere")
    return (
      <>
        <StepHead
          title="Where should your chats run?"
          lead="Docker on this machine is set to use another machine's Docker."
        />
        <Card>
          <p className="pool-setup-p">
            <strong>Your Docker points to:</strong> {`ssh://${AT("sam", "build-box.lan")}`}, from
            DOCKER_HOST.
          </p>
          <p className="pool-setup-p">
            purlis can&apos;t tell from here which machine that is, so it makes no runner there on
            its own. Set it up as another machine, and purlis checks it and remembers it.
          </p>
        </Card>
        <Action
          does="Opens the steps for another machine, with build-box.lan filled in. Nothing is changed yet."
          main="Set up Docker on another machine"
          onMain={() =>
            set({
              kind: "remote",
              screen: "connect",
              route: "ssh",
              machine: "typed",
              typed: `ssh://${AT("sam", "build-box.lan")}`,
              phase: "pick",
            })
          }
        >
          <Button variant="secondary" onPress={() => set({ kind: "local", screen: "pool" })}>
            Use this machine&apos;s Docker
          </Button>
        </Action>
      </>
    );
  const here = state.where === "docker-here";
  return (
    <>
      <StepHead
        title="Where should your chats run?"
        lead="Right now your chats run directly on this machine. A pool gives each chat a separate machine, so it can work on its own."
      />
      <OptionCards
        aria-label="Where your chats run"
        value={state.kind}
        onChange={(to) => set({ kind: to === "remote" ? "remote" : "local" })}
      >
        <OptionCard
          value="local"
          icon={<Server size={18} />}
          isDisabled={!here}
          title={<span className="pool-setup-choice">This machine {here && <Recommended />}</span>}
          detail={
            here
              ? "Docker is running here, so you can have a working chat in a few minutes."
              : "Docker isn't on this machine. Install it, or use another machine that has it."
          }
        />
        <OptionCard
          value="remote"
          icon={<Server size={18} />}
          title={
            <span className="pool-setup-choice">
              Another machine with Docker {!here && <Recommended />}
            </span>
          }
          detail={`A second Mac, a Linux box or a server, reached over SSH or Docker's own settings. Your chats keep running there while this machine sleeps. ${counts}`}
        />
      </OptionCards>
      <Action
        does={
          state.kind === "local"
            ? "Uses Docker on this machine. Next, you choose the pool's image and limits."
            : "Next, you choose the machine and check it's the right one. Nothing is changed on it yet."
        }
        main="Continue"
        onMain={() =>
          set(state.kind === "local" ? { screen: "pool" } : { screen: "connect", phase: "pick" })
        }
      />
    </>
  );
}

// --- Connect -----------------------------------------------------------------------------------

function RouteChoice({ state, set }: { state: State; set: (to: Partial<State>) => void }) {
  const first = (route: Route) => FOUND[route].find((one) => !one.not)?.id ?? "typed";
  return (
    <Asked
      label="How should purlis reach it?"
      value={state.route}
      onChange={(to) => {
        const route = to as Route;
        set({ route, machine: route === "tls" ? "typed" : first(route), phase: "pick" });
      }}
    >
      <Radio
        value="ssh"
        description="Signs in with your SSH keys and settings. Nothing changes on the other machine but what SSH needs."
      >
        <span className="pool-setup-choice">
          SSH <Recommended />
        </span>
      </Radio>
      <Radio
        value="tls"
        description="For a machine whose Docker is already set up to take connections, with certificates."
      >
        Docker&apos;s API over TLS
      </Radio>
      <Radio value="context" description="One you already use with docker context.">
        A Docker context
      </Radio>
      <Radio value="podman" description="One you already use with podman system connection.">
        A Podman connection
      </Radio>
    </Asked>
  );
}

const PICK_LABEL: Record<Route, string> = {
  ssh: "Which machine?",
  tls: "Which machine?",
  context: "Which context?",
  podman: "Which connection?",
};

function MachinePick({ state, set }: { state: State; set: (to: Partial<State>) => void }) {
  const found = FOUND[state.route];
  const typing = state.machine === "typed" || state.route === "tls";
  const typedOne = typing ? parseTyped(state.typed, state.route) : undefined;
  const field = (
    <TextField
      label={state.route === "tls" ? "The machine's address" : "Its address"}
      value={state.typed}
      onChange={(typed) => set({ typed })}
      placeholder={state.route === "tls" ? "lab.example.internal:2376" : AT("sam", "build-box.lan")}
      help={
        state.route === "tls"
          ? "Its name, and the port Docker takes TLS connections on: 2376 unless it was set otherwise."
          : "The same address you'd give ssh: your account on it, an @ and its name. An alias from your SSH settings works too."
      }
      error={
        typedOne?.plainTcp && state.route !== "tls"
          ? undefined
          : state.typed.trim() !== "" && !typedOne
            ? `That isn't an address purlis can read. Type it as ${AT("sam", "build-box.lan")}.`
            : undefined
      }
    />
  );
  if (state.route === "tls") return field;
  return (
    <>
      <Asked
        label={PICK_LABEL[state.route]}
        value={state.machine}
        onChange={(machine) => set({ machine })}
      >
        {found.map((one) => (
          <Radio
            key={one.id}
            value={one.id}
            isDisabled={one.not?.kind === "local"}
            description={one.not ? `${one.from}. ${one.not.why}` : one.from}
          >
            {one.name}
          </Radio>
        ))}
        <Radio
          value="typed"
          description={
            state.route === "ssh"
              ? "A machine that isn't in your SSH settings."
              : "Type the machine's address instead."
          }
        >
          Another machine
        </Radio>
      </Asked>
      {state.machine === "typed" && field}
    </>
  );
}

function ConnectOptions({ state, over }: { state: State; over: "ssh" | "tls" }) {
  return (
    <Options open={state.optionsOpen}>
      {over === "ssh" ? (
        <>
          <TextField
            label="Docker's socket on the machine"
            value=""
            onChange={() => undefined}
            placeholder="/var/run/docker.sock"
            help="Leave it empty and purlis looks in the usual places: Docker's, rootless Docker's, then Podman's. Recommended: empty."
          />
          <p className="pool-setup-does">
            Whoever can use Docker on a machine can do anything on it. If the account you sign in
            with shouldn&apos;t, use rootless Docker or Podman there and name its socket here.
          </p>
        </>
      ) : (
        <>
          <p className="pool-setup-does">
            purlis found these in ~/.docker/lab/. It reads them where they are.
          </p>
          <div className="pool-setup-grid">
            <TextField
              label="Authority (CA) file"
              value={TLS_CA.files.ca}
              onChange={() => undefined}
            />
            <TextField label="Certificate" value={TLS_CA.files.cert} onChange={() => undefined} />
            <TextField
              label="Key"
              value={TLS_CA.files.key}
              onChange={() => undefined}
              help="Or keep the key in a vault entry instead: purlis writes it only for each run, and removes it after."
            />
          </div>
        </>
      )}
    </Options>
  );
}

function seeWhat(m: Machine): string {
  if (m.over === "tls")
    return [
      `docker --host ${m.address} --tlsverify \\`,
      `  --tlscacert ${TLS_CA.files.ca} --tlscert ${TLS_CA.files.cert} --tlskey ${TLS_CA.files.key} \\`,
      "  info --format '{{.ID}}'",
    ].join("\n");
  return [
    `ssh-keyscan -p ${m.port} ${m.host} | ssh-keygen -lf -`,
    `ssh -N -o StrictHostKeyChecking=yes -o UserKnownHostsFile=<this pool's key> \\`,
    `  -L <purlis's socket>:/var/run/docker.sock ${m.user}@${m.host}`,
    "docker --host unix://<purlis's socket> info --format '{{.ID}}'",
  ].join("\n");
}

function Chosen({
  m,
  state,
  set,
  pinned,
}: {
  m: Machine;
  state: State;
  set: (to: Partial<State>) => void;
  pinned: boolean;
}) {
  return (
    <Card>
      <Facts aria-label="What this pool connects to">
        <Fact label="Machine">{m.name === m.host ? m.host : `${m.name} (${m.host})`}</Fact>
        <Fact label="Reached by">
          {m.found && state.route !== "ssh"
            ? `${ROUTE_NAMES[state.route]}, read once as ${m.over === "ssh" ? "SSH" : "TLS"}`
            : ROUTE_NAMES[m.over]}
        </Fact>
        {pinned && (
          <Fact label={m.over === "ssh" ? "Its key" : "Its authority"}>
            <span className="pool-setup-choice">
              <Badge variant="ok">Trusted</Badge>
              {m.over === "ssh"
                ? HOST_KEY.fingerprint.slice(0, 22)
                : TLS_CA.fingerprint.slice(0, 23)}
              …
            </span>
          </Fact>
        )}
        {pinned && state.phase === "owned" && (
          <Fact label="Its Docker">{`${ENGINE_ID.id.slice(0, 14)}…`}</Fact>
        )}
      </Facts>
      <Button variant="link" onPress={() => set({ phase: "pick", retrust: undefined })}>
        Change
      </Button>
    </Card>
  );
}

function Confirm({
  m,
  state,
  set,
}: {
  m: Machine;
  state: State;
  set: (to: Partial<State>) => void;
}) {
  const tls = m.over === "tls";
  const again = state.retrust;
  const fingerprint = tls
    ? again
      ? TLS_CA.changed
      : TLS_CA.fingerprint
    : again
      ? HOST_KEY.changed
      : HOST_KEY.fingerprint;
  const command = tls
    ? `openssl x509 -in ca.pem -noout -subject -fingerprint -sha256`
    : "ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub";
  const lead = again
    ? `This is the ${tls ? "authority" : "key"} ${m.host} shows now. Check it on the machine before you trust it again.`
    : tls
      ? `${m.host}'s certificate is signed by this authority. Check it's the one you set up.`
      : state.known
        ? `Your SSH settings already know ${m.host} by this key. purlis will trust this key alone for this pool.`
        : `This is the first time ${m.host} has been reached from here. Check that the key it showed is the one on ${m.host}.`;
  return (
    <>
      <h2 className="pool-setup-h2">
        {tls ? "Check it's the right machine's authority" : "Check it's the right machine"}
      </h2>
      <p className="pool-setup-does">{lead}</p>
      {again && (
        <Facts aria-label="The key before and now">
          <Fact label="You trusted">{tls ? TLS_CA.fingerprint : HOST_KEY.fingerprint}</Fact>
          <Fact label="It shows now">{fingerprint}</Fact>
        </Facts>
      )}
      {tls && !again && (
        <Facts aria-label="The authority">
          <Fact label="Issued to">{TLS_CA.subject}</Fact>
        </Facts>
      )}
      <CodeBlock
        label={tls ? `${m.host}'s authority, SHA-256` : `${m.host}'s key, ${HOST_KEY.type}`}
        text={fingerprint}
        copy="Copy fingerprint"
      />
      <SeeWhat
        open={!state.known || !!again || tls}
        note={`Run this on ${m.host} itself, at its own keyboard or in a session you already trust. It prints the ${tls ? "authority's" : "key's"} fingerprint, which should read the same as above.`}
        text={command}
      />
      <Action
        does={`purlis remembers this ${tls ? "authority" : "key"} for this pool, and stops if ${m.host} ever shows another. Then it checks Docker there.`}
        main="Trust this machine"
        reserve={["Trust this machine", "Continue"]}
        icon={<KeyRound aria-hidden size={16} />}
        onMain={() => set({ phase: "checking", finding: undefined })}
      >
        <Button variant="ghost" onPress={() => set({ phase: "pick", retrust: undefined })}>
          This isn&apos;t it
        </Button>
      </Action>
    </>
  );
}

/** Each check's verdict, as far as the check got. */
function verdicts(
  state: State,
  m: Machine,
): { title: string; verdict: CheckVerdict; at: number }[] {
  const lines = CHECKS.filter((check) => {
    const ids = Array.isArray(check.finding) ? check.finding : [check.finding];
    const only = said(ids[0], "", "", 0).only;
    return only === undefined || only === m.over;
  });
  const failing =
    state.phase === "found" && state.finding
      ? lines.findIndex((check) =>
          (Array.isArray(check.finding) ? check.finding : [check.finding]).includes(
            state.finding ?? "name",
          ),
        )
      : -1;
  return lines.map((check, at) => ({
    title: check.title,
    at,
    verdict:
      state.phase === "checking"
        ? at === 0
          ? "running"
          : "todo"
        : failing === -1
          ? "ok"
          : at < failing
            ? "ok"
            : at === failing
              ? "missing"
              : "todo",
  }));
}

function EngineCheck({ m, state }: { m: Machine; state: State }) {
  return (
    <CheckList aria-label={`What purlis checked on ${m.host}`}>
      {verdicts(state, m).map((line) => (
        <CheckLine key={line.title} verdict={line.verdict} title={line.title} />
      ))}
    </CheckList>
  );
}

function FindingSay({
  m,
  state,
  set,
}: {
  m: Machine;
  state: State;
  set: (to: Partial<State>) => void;
}) {
  const id = state.finding ?? "name";
  const words = said(id, m.host, m.user, m.port);
  const again = () => set({ phase: "checking", finding: undefined, pretend: "pass" });
  const remedy = () => {
    if (words.remedy === "Change the machine") set({ phase: "pick", finding: undefined });
    else if (words.remedy === "Choose other files")
      set({ phase: "pick", finding: undefined, optionsOpen: true });
    else if (words.remedy.startsWith("Use "))
      set({
        typed: "docker.lab.example.internal:2376",
        phase: "checking",
        finding: undefined,
        pretend: "pass",
      });
    else again();
  };
  return (
    <>
      <Say
        tone="trouble"
        title={words.title}
        actions={
          <>
            <Button variant="primary" onPress={remedy}>
              {words.remedy}
            </Button>
            {words.remedy !== "Change the machine" && (
              <Button
                variant="secondary"
                onPress={() => set({ phase: "pick", finding: undefined })}
              >
                Change the machine
              </Button>
            )}
          </>
        }
      >
        <p className="pool-setup-p">{words.says}</p>
        {words.plainly && <p className="pool-setup-p">{words.plainly}</p>}
      </Say>
      {words.commands && (
        <SeeWhat
          open
          note={`Run these on ${m.host}, or from here where they start with ssh. purlis notices the change when you come back.`}
          text={words.commands}
        />
      )}
    </>
  );
}

function Owned({ m, state, set }: { m: Machine; state: State; set: (to: Partial<State>) => void }) {
  return (
    <>
      <Asked
        label={`Who can sign in to ${m.host}?`}
        value={state.owned}
        onChange={(to) => set({ owned: to === "mine" ? "mine" : "not" })}
      >
        <Radio
          value="not"
          description="Chats there never get your saved logins. You can change this later, on the pool's page."
        >
          <span className="pool-setup-choice">
            Others might, or I&apos;m not sure <Recommended />
          </span>
        </Radio>
        <Radio
          value="mine"
          description={`Chats there may use your saved logins, each only when you allow it. If ${m.host}'s key ever changes, this goes back to not sure.`}
        >
          Only me, and people I trust with my logins
        </Radio>
      </Asked>
      <Action
        does={`Keeps ${m.host} as this pool's machine. Next, you choose its image and limits.`}
        main="Continue"
        reserve={["Trust this machine", "Continue"]}
        onMain={() => set({ screen: "pool" })}
      />
    </>
  );
}

function Connect({ state, set }: { state: State; set: (to: Partial<State>) => void }) {
  const m = machineOf(state);
  const pinned = ["checking", "found", "owned"].includes(state.phase);
  const shown = state.phase !== "pick" && state.phase !== "refused";
  useEffect(() => {
    if (state.hold) return;
    if (state.phase === "reading") {
      const t = setTimeout(() => set({ phase: "confirm" }), 1200);
      return () => clearTimeout(t);
    }
    if (state.phase === "checking") {
      const t = setTimeout(
        () =>
          state.pretend === "pass" ||
          (said(state.pretend, "", "", 0).only !== undefined &&
            said(state.pretend, "", "", 0).only !== m?.over)
            ? set({ phase: "owned" })
            : set({ phase: "found", finding: state.pretend }),
        1600,
      );
      return () => clearTimeout(t);
    }
  }, [state.phase, state.hold, state.pretend, m?.over, set]);

  const lead =
    state.phase === "owned"
      ? `${m?.host} is ready to run your chats.`
      : "Choose the machine, and check it's the one you mean. purlis keeps no password or key of yours: SSH signs in with your own agent.";
  return (
    <>
      <StepHead
        title={
          state.retrust ? `Check ${m?.host ?? "the machine"} again` : "Connect to the other machine"
        }
        tag={state.phase === "owned" ? <Badge variant="ok">Every check passed</Badge> : undefined}
        lead={lead}
      />
      {shown && m ? (
        <Chosen m={m} state={state} set={set} pinned={pinned} />
      ) : (
        <>
          <RouteChoice state={state} set={set} />
          <MachinePick state={state} set={set} />
          {m && !m.plainTcp && <ConnectOptions state={state} over={m.over} />}
        </>
      )}
      {state.phase === "pick" && (
        <>
          {m && <SeeWhat text={seeWhat(m)} />}
          <Action
            does={
              m
                ? `Asks ${m.host} for its ${m.over === "tls" ? "certificate" : "key"}, so you can check it's the right machine. Nothing is kept, and nothing changes there.`
                : "Pick a machine, or type its address."
            }
            main={m ? `Connect to ${m.name}` : "Connect"}
            onMain={() => {
              if (!m) return;
              set({ phase: m.plainTcp ? "refused" : "reading" });
            }}
            busy={!m}
          />
        </>
      )}
      {state.phase === "refused" && m && (
        <>
          <Say
            tone="attention"
            title={`purlis won't use ${m.host}'s Docker without TLS`}
            actions={
              <>
                <Button
                  variant="primary"
                  onPress={() =>
                    set({ route: "ssh", machine: "typed", typed: `sam@${m.host}`, phase: "pick" })
                  }
                >
                  Use SSH instead
                </Button>
                <Button
                  variant="secondary"
                  icon={<ExternalLink aria-hidden size={14} />}
                  onPress={() => undefined}
                >
                  Turn on TLS
                </Button>
              </>
            }
          >
            <p className="pool-setup-p">
              {`${m.address} takes connections without TLS, so anyone who can reach port ${m.port} can run anything on ${m.host}.`}
            </p>
            <p className="pool-setup-p">
              <strong>SSH</strong> needs no change on {m.host} but SSH itself, and is what purlis
              recommends. <strong>Turning on TLS</strong> follows Docker&apos;s own guide; then
              connect with Docker&apos;s API over TLS.
            </p>
          </Say>
          <SeeWhat
            note={`What Docker's guide has you run on ${m.host} to turn on TLS, after making its certificates.`}
            text={
              "dockerd --tlsverify \\\n  --tlscacert=ca.pem --tlscert=server-cert.pem --tlskey=server-key.pem \\\n  -H=0.0.0.0:2376"
            }
          />
        </>
      )}
      {state.phase === "reading" && m && (
        <Busy>{`Asking ${m.host} for its ${m.over === "tls" ? "certificate" : "key"}. This only reads.`}</Busy>
      )}
      {state.phase === "confirm" && m && <Confirm m={m} state={state} set={set} />}
      {(state.phase === "checking" || state.phase === "found" || state.phase === "owned") && m && (
        <>
          <h2 className="pool-setup-h2">{`Docker on ${m.host}`}</h2>
          <EngineCheck m={m} state={state} />
        </>
      )}
      {state.phase === "checking" && m && (
        <Busy>{`Checking Docker on ${m.host}. This only reads.`}</Busy>
      )}
      {state.phase === "found" && m && <FindingSay m={m} state={state} set={set} />}
      {state.phase === "owned" && m && <Owned m={m} state={state} set={set} />}
    </>
  );
}

// --- The pool reopened: the pin-changed Notice, and doctor's context finding --------------------

const CHANGED: Record<Exclude<Reopened, "context">, { what: string; old: string; now: string }> = {
  machine: { what: "key", old: HOST_KEY.fingerprint, now: HOST_KEY.changed },
  ca: { what: "certificate authority", old: TLS_CA.fingerprint, now: TLS_CA.changed },
  engine: { what: "Docker", old: ENGINE_ID.id, now: ENGINE_ID.changed },
};

function ReopenedPage({ state, set }: { state: State; set: (to: Partial<State>) => void }) {
  const [removed, setRemoved] = useState(false);
  const why = state.reopened ?? "machine";
  const host = why === "ca" ? "lab.example.internal" : "build-box.lan";
  if (removed)
    return (
      <Say tone="news" title="build-box would be removed">
        <p className="pool-setup-p">
          The prototype stops here: Remove pool asks first, as it does today.
        </p>
      </Say>
    );
  if (why === "context") {
    const words = said("context-moved", host, "sam", 22);
    return (
      <>
        <StepHead title="build-box" lead="A pool on another machine, reached by SSH." />
        <Say
          tone="attention"
          title={words.title}
          actions={
            <>
              <Button
                variant="primary"
                onPress={() =>
                  set({
                    screen: "connect",
                    route: "context",
                    machine: "build-box-ssh",
                    phase: "pick",
                  })
                }
              >
                Connect again
              </Button>
              <Button variant="secondary" onPress={() => set({ reopened: undefined })}>
                Keep it as it is
              </Button>
            </>
          }
        >
          <p className="pool-setup-p">{words.says}</p>
          <p className="pool-setup-p">
            The pool still runs on build-box.lan, which you checked. It doesn&apos;t follow the
            context by itself.
          </p>
        </Say>
      </>
    );
  }
  const changed = CHANGED[why];
  return (
    <>
      <Notice
        cause="pool-pin-changed"
        tone="trouble"
        title={`${host}'s ${changed.what} has changed, so the pool build-box stopped`}
        fixes={[
          {
            label: "Check and trust again",
            primary: true,
            onPress: () =>
              set(
                why === "engine"
                  ? {
                      screen: "connect",
                      phase: "checking",
                      retrust: why,
                      route: "ssh",
                      machine: "build-box",
                    }
                  : {
                      screen: "connect",
                      phase: "confirm",
                      retrust: why,
                      route: why === "ca" ? "context" : "ssh",
                      machine: why === "ca" ? "lab-tls" : "build-box",
                      known: false,
                    },
              ),
          },
          { label: "Remove pool", onPress: () => setRemoved(true) },
        ]}
      >
        {why === "engine"
          ? `${host} is the machine you trusted, but its Docker is a different one now. This happens when Docker is reset or reinstalled. Its runners were made on the old Docker, so they're on another engine, not removed.`
          : `${host} shows a different ${changed.what} than the one you trusted. This happens when the machine is reinstalled. Check it on the machine before you trust it again.`}{" "}
        Until then purlis starts no runner there and sends it none of your logins, and the pool is
        no longer marked as only yours.
      </Notice>
      <StepHead title="build-box" lead={`A pool on another machine, ${host}.`} />
      <Facts aria-label="What changed">
        <Fact label="You trusted">{changed.old}</Fact>
        <Fact label="It shows now">{changed.now}</Fact>
      </Facts>
    </>
  );
}

// --- The tab -----------------------------------------------------------------------------------

function stepsOf(
  state: State,
): { id: string; name: string; state: StepState; line: string | null }[] {
  const remote =
    state.kind === "remote" || state.screen === "connect" || state.screen === "reopened";
  const at = state.screen === "reopened" ? "connect" : state.screen;
  const ids = remote
    ? ["where", "connect", "pool", "signins", "identity", "try"]
    : ["where", "pool", "signins", "identity", "try"];
  const names: Record<string, string> = {
    where: "Where",
    connect: "Connect",
    pool: "Pool",
    signins: "Sign-ins",
    identity: "Agent identity",
    try: "Try it",
  };
  const reached = ids.indexOf(at);
  const m = machineOf(state);
  return ids.map((id, index) => ({
    id,
    name: names[id],
    state:
      state.screen === "reopened"
        ? id === "connect"
          ? "attention"
          : "done"
        : index < reached
          ? "done"
          : index === reached
            ? id === "connect" && (state.phase === "found" || state.phase === "refused")
              ? "attention"
              : "current"
            : "later",
    line:
      id === "where" && index < reached
        ? remote
          ? "Another machine with Docker"
          : "Docker on this machine"
        : id === "connect" && (index < reached || state.screen === "reopened")
          ? (m?.host ?? "build-box.lan")
          : null,
  }));
}

export function ConnectPrototype({ start }: { start: Partial<State> }) {
  const [state, setState] = useState<State>({ ...START, ...start });
  const set = useCallback(
    (to: Partial<State>) => setState((was) => ({ ...was, ...to, hold: false })),
    [],
  );
  let page: ReactNode;
  if (state.screen === "where") page = <Where state={state} set={set} />;
  else if (state.screen === "connect") page = <Connect state={state} set={set} />;
  else if (state.screen === "reopened") page = <ReopenedPage state={state} set={set} />;
  else
    page = (
      <>
        <StepHead
          title="Pool"
          lead={
            state.kind === "remote"
              ? `The Docker kind's step as it is today, its image and limits checked against ${machineOf(state)?.host ?? "the other machine"}.`
              : "The Docker kind's step as it is today."
          }
        />
        <p className="pool-setup-does">
          The prototype ends here: the steps after Connect are unchanged.
        </p>
        <Action main="Back to Where" onMain={() => setState({ ...START, ...start })} />
      </>
    );
  return (
    <div className="pool-setup" style={{ height: 820 }}>
      <StepList
        aria-label="Steps of Pool setup"
        className="pool-setup-rail"
        steps={stepsOf(state)}
        showing={state.screen === "reopened" ? "connect" : state.screen}
        onGo={(id) => {
          if (id === "where") set({ screen: "where" });
          if (id === "connect") set({ screen: "connect" });
        }}
      >
        <p className="pool-setup-saved">
          Your choices are saved as you go. Close this tab whenever you like.
        </p>
      </StepList>
      <div className="pool-setup-main">
        <div className="pool-setup-page">
          <PrototypeStrip state={state} set={set} />
          {page}
        </div>
      </div>
    </div>
  );
}
