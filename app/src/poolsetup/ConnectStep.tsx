// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useState, type ReactNode } from "react";
import { KeyRound } from "lucide-react";
import {
  commands,
  type PlaneId,
  type PoolSetupPodmanConnection,
  type PoolSetupShown,
  type PoolSetupStep,
} from "../bindings";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import { Card } from "../ui/card/Card";
import { Fact, Facts } from "../ui/card/Facts";
import { CheckLine, CheckList, type CheckVerdict } from "../ui/checks/Checks";
import { Radio, RadioGroup } from "../ui/choice/Radio";
import { CodeBlock } from "../ui/code/CodeBlock";
import { TextField } from "../ui/field/Field";
import { Action, Busy, Options, Recommended, Say, SeeWhat, StepHead } from "./parts";

/**
 * **Connect, the step of a pool on another machine** (#6266, ADR 0123 §3 and §6), as the accepted
 * prototype draws it (#6265): one page that grows. The machine is picked from the person's SSH
 * settings or typed; Connect to only reads the key the machine shows; the person checks it and
 * presses Trust this machine, or This isn't it; Docker there is checked, and each finding marks
 * its line of the check list with one trouble card. Every sentence of a state is the core's
 * (`purlis_core::poolsetup::connecting`), so this and `purlis pool setup` say the same.
 *
 * The machines offered are the person's SSH hosts and their Podman connections (#6269), each
 * read into an SSH route: a Podman connection's carries its key file, which the core keeps beside
 * the route. **Two routes** (#6267): SSH, recommended, and Docker's API over TLS, whose CA, client
 * certificate and key are named under Options, the key a file or a vault entry. Contexts are a
 * later ticket of #6257, so they aren't offered.
 */

/** What the person may be doing on the page, beside what the core read. */
type Pressing = "connecting" | "trusting" | "checking" | undefined;

/** How purlis reaches the machine (ADR 0123 §2). */
export type Over = "ssh" | "tls";

/** The causes of a TLS check's failure (#6267), each the core's. */
const TLS_CAUSES = [
  "tls-not-signed",
  "tls-expired",
  "tls-wrong-name",
  "tls-client-refused",
  "tls-client-expired",
  "tls-file",
] as const;

type Check = { title: (host: string) => string; causes: readonly string[] };

/** What Connect checks over SSH, in order, and the causes that fail each line. */
const SSH_CHECKS: readonly Check[] = [
  { title: (host) => `${host} is found`, causes: ["no-such-name"] },
  { title: (host) => `${host} answers`, causes: ["no-answer", "not-met"] },
  {
    title: () => "Its key is the one you trusted",
    causes: ["pool-pin-changed"],
  },
  { title: () => "It accepts your sign-in", causes: ["sign-in-refused"] },
  {
    title: () => "Docker answers there",
    causes: ["no-engine", "engine-silent", "forwarding-refused", "needs-podman"],
  },
  { title: () => "Your account may use Docker", causes: ["not-allowed"] },
  { title: () => "Its Docker is the one you trusted", causes: ["pool-engine-changed"] },
];

/** What Connect checks over TLS, in order: there is no sign-in, and the certificate is checked. */
const TLS_CHECKS: readonly Check[] = [
  { title: (host) => `${host} is found`, causes: ["no-such-name"] },
  { title: (host) => `${host} answers`, causes: ["no-answer"] },
  { title: () => "Its authority is the one you trusted", causes: ["pool-pin-changed"] },
  { title: () => "Its certificate checks out", causes: TLS_CAUSES },
  { title: () => "Docker answers there", causes: ["engine-silent"] },
  { title: () => "Its Docker is the one you trusted", causes: ["pool-engine-changed"] },
];

const checksOf = (over: Over) => (over === "tls" ? TLS_CHECKS : SSH_CHECKS);

/** The trouble card's title for each cause. */
const TITLES: Record<string, string> = {
  "no-such-name": "purlis couldn't find that machine",
  "no-answer": "The machine didn't answer",
  "not-met": "SSH didn't answer there",
  "pool-pin-changed": "The machine shows a different key",
  "sign-in-refused": "The machine didn't accept your sign-in",
  "no-engine": "No Docker there that you can use",
  "engine-silent": "Docker didn't answer there",
  "forwarding-refused": "SSH there doesn't allow forwarding",
  "not-allowed": "Your account can't use Docker there",
  "pool-engine-changed": "Docker there isn't the one you trusted",
  "needs-podman": "purlis needs Podman on this machine too",
  "tls-not-signed": "Its certificate is from another authority",
  "tls-expired": "Its certificate has expired",
  "tls-wrong-name": "Its certificate is for another name",
  "tls-client-refused": "Docker there didn't accept your certificate",
  "tls-client-expired": "Your certificate has expired",
  "tls-file": "purlis can't use one of the files you named",
  "not-a-route": "That isn't an address purlis can read",
  here: "That didn't work",
};

/** **Each check's verdict**: those before the failing line passed, the rest were not reached. */
export function verdictsOf(
  cause: string | null,
  done: boolean,
  over: Over = "ssh",
): CheckVerdict[] {
  const checks = checksOf(over);
  const failing = cause === null ? -1 : checks.findIndex((one) => one.causes.includes(cause));
  return checks.map((_, at) =>
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

/** The route a machine typed for Docker's API over TLS is kept as: `tcp://` and the address. */
export function tcpRouteOf(typed: string): string {
  const text = typed.trim();
  return text === "" || text.includes("://") ? text : `tcp://${text}`;
}

/**
 * **How a kept machine is reached**: over TLS where it is a `tcp://` address with its files named.
 * A `tcp://` address with none is plain TCP, refused on its card until the person chooses TLS.
 */
export const overOf = (machine: PoolSetupStep["machine"] | undefined): Over =>
  machine?.route?.startsWith("tcp://") === true && machine.tls !== null ? "tls" : "ssh";

/** **How should purlis reach it?**, SSH first and recommended (#6265's route choice). */
function RouteChoice({ over, onChange }: { over: Over; onChange: (to: Over) => void }) {
  return (
    <Asked
      label="How should purlis reach it?"
      value={over}
      onChange={(to) => onChange(to === "tls" ? "tls" : "ssh")}
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
    </Asked>
  );
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
  const tls = machine?.over === "tls";
  return (
    <Card>
      <Facts aria-label="What this pool connects to">
        <Fact label="Machine">{machine?.host ?? "The machine"}</Fact>
        <Fact label="Reached by">{tls ? "Docker's API over TLS" : "SSH"}</Fact>
        {tls && machine?.trusted_ca && (
          <Fact label="Its authority">
            <span className="pool-setup-choice">
              <Badge variant="ok">Trusted</Badge>
              {/* Its first eight pairs, as the prototype shows it: the whole is in the line. */}
              {`${machine.trusted_ca.slice(0, 23)}…`}
            </span>
          </Fact>
        )}
        {!tls && machine?.trusted && (
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

/** A Podman connection's choice, apart from any SSH host's name. */
const PODMAN = "podman:";

/** **The pick**: the machines the person's SSH settings and Podman connections name, and another
 * one typed. */
function Pick({
  step,
  hosts,
  podmans,
  busy,
  onPick,
  onConnect,
}: {
  step: PoolSetupStep;
  hosts: readonly string[];
  podmans: readonly PoolSetupPodmanConnection[];
  busy: boolean;
  onPick: (route: string | null, identity: string | null) => void;
  onConnect: () => void;
}) {
  const kept = step.machine?.route ?? "";
  const keptIdentity = step.machine?.identity ?? null;
  const keptHost = keptIdentity === null ? hosts.find((host) => routeOf(host) === kept) : undefined;
  const keptPodman = podmans.find(
    (one) => one.route === kept && (one.identity ?? null) === keptIdentity,
  );
  const offered = [...hosts, ...podmans.map((one) => PODMAN + one.name)];
  // Until the person picks, the pick follows what is kept and what the SSH settings and Podman
  // name, which are read after the step is drawn.
  const [chosen, setPicked] = useState<string>();
  const picked =
    chosen ??
    keptHost ??
    (keptPodman ? PODMAN + keptPodman.name : undefined) ??
    (kept === "" && offered.length > 0 ? (offered[0] ?? "typed") : "typed");
  const [typed, setTyped] = useState(
    keptHost !== undefined || keptPodman !== undefined ? "" : kept.replace(/^ssh:\/\//, ""),
  );
  const podman = picked.startsWith(PODMAN)
    ? podmans.find((one) => PODMAN + one.name === picked)
    : undefined;
  const route = podman ? podman.route : picked === "typed" ? routeOf(typed) : routeOf(picked);
  const identity = podman?.identity ?? null;
  // What is picked is kept as it is picked, so coming back finds it: the route and the key file
  // it signs in with, either of which may change alone. Nothing is contacted.
  const asKept = route === kept && identity === keptIdentity;
  useEffect(() => {
    if (asKept) return;
    const later = setTimeout(() => onPick(route === "" ? null : route, identity), 400);
    return () => clearTimeout(later);
  }, [asKept, route, identity, onPick]);
  const next = step.next;
  const host = step.machine?.host;
  return (
    <>
      {offered.length > 0 ? (
        <Asked label="Which machine?" value={picked} onChange={setPicked}>
          {hosts.map((one) => (
            <Radio key={one} value={one} description="From your SSH settings">
              {one}
            </Radio>
          ))}
          {podmans.map((one) => (
            <Radio
              key={PODMAN + one.name}
              value={PODMAN + one.name}
              description="From your Podman connections"
            >
              {one.name}
            </Radio>
          ))}
          <Radio value="typed" description="A machine that isn't in your SSH settings.">
            Another machine
          </Radio>
        </Asked>
      ) : null}
      {(offered.length === 0 || picked === "typed") && (
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
        busy={busy || route === "" || !asKept}
      />
    </>
  );
}

/** The files a `tcp://` machine is reached with, as the person types them. */
type TlsTyped = {
  ca: string;
  cert: string;
  keyIn: "file" | "vault";
  key: string;
  vault: string;
  name: string;
};

/** **The machine reached over Docker's API over TLS** (#6267): its address, and under Options the
 * CA, the client certificate and the key, a file or a vault entry. Each is kept as it is typed,
 * as the SSH machine is, and nothing is contacted until Connect to. */
function TlsPick({
  step,
  busy,
  onPick,
  onFiles,
  onConnect,
}: {
  step: PoolSetupStep;
  busy: boolean;
  onPick: (route: string | null) => void;
  onFiles: (files: TlsTyped | null) => void;
  onConnect: () => void;
}) {
  const kept = step.machine?.route ?? "";
  const keptFiles = step.machine?.tls ?? null;
  const [typed, setTyped] = useState(
    kept.startsWith("tcp://") ? kept.replace(/^tcp:\/\//, "") : "",
  );
  const [files, setFiles] = useState<TlsTyped>({
    ca: keptFiles?.ca ?? "",
    cert: keptFiles?.cert ?? "",
    keyIn: keptFiles?.key_vault ? "vault" : "file",
    key: keptFiles?.key ?? "",
    vault: keptFiles?.key_vault ?? "",
    name: keptFiles?.key_name ?? "",
  });
  // Once kept, the fields show the files as purlis keeps them: a path typed from the home
  // (`~/.docker/ca.pem`) is kept absolute.
  const keptOnce = keptFiles === null ? null : JSON.stringify(keptFiles);
  const [shownKept, setShownKept] = useState(keptOnce);
  if (keptOnce !== shownKept) {
    setShownKept(keptOnce);
    if (keptFiles !== null)
      setFiles({
        ca: keptFiles.ca,
        cert: keptFiles.cert,
        keyIn: keptFiles.key_vault ? "vault" : "file",
        key: keptFiles.key ?? "",
        vault: keptFiles.key_vault ?? "",
        name: keptFiles.key_name ?? "",
      });
  }
  const route = tcpRouteOf(typed);
  // The route is kept as it is typed, so coming back finds it. Nothing is contacted.
  useEffect(() => {
    if (route === kept) return;
    const later = setTimeout(() => onPick(route === "" ? null : route), 400);
    return () => clearTimeout(later);
  }, [route, kept, onPick]);
  const named =
    files.ca.trim() !== "" &&
    files.cert.trim() !== "" &&
    (files.keyIn === "file"
      ? files.key.trim() !== ""
      : files.vault.trim() !== "" && files.name.trim() !== "");
  const same =
    keptFiles !== null &&
    keptFiles.ca === files.ca.trim() &&
    keptFiles.cert === files.cert.trim() &&
    (files.keyIn === "file"
      ? keptFiles.key === files.key.trim()
      : keptFiles.key_vault === files.vault.trim() && keptFiles.key_name === files.name.trim());
  // The files are kept once the route is, and once all of them are named.
  useEffect(() => {
    if (route === "" || route !== kept || !named || same) return;
    const later = setTimeout(() => onFiles(files), 400);
    return () => clearTimeout(later);
  }, [route, kept, named, same, files, onFiles]);
  const set = (to: Partial<TlsTyped>) => setFiles((was) => ({ ...was, ...to }));
  const host = step.machine?.host;
  return (
    <>
      <TextField
        label="The machine's address"
        value={typed}
        onChange={setTyped}
        placeholder="lab.example.internal:2376"
        help="Its name, and the port Docker takes TLS connections on: 2376 unless it was set otherwise."
      />
      <Options open={!same}>
        <p className="pool-setup-does">
          purlis reads these where they are, and checks the authority before every run.
        </p>
        <div className="pool-setup-grid">
          <TextField
            label="Authority (CA) file"
            value={files.ca}
            onChange={(ca) => set({ ca })}
            placeholder="~/.docker/ca.pem"
          />
          <TextField
            label="Certificate"
            value={files.cert}
            onChange={(cert) => set({ cert })}
            placeholder="~/.docker/cert.pem"
          />
        </div>
        <Asked
          label="Where is the key?"
          value={files.keyIn}
          onChange={(to) => set({ keyIn: to === "vault" ? "vault" : "file" })}
        >
          <Radio value="file" description="A file on this machine, beside the certificate.">
            In a file
          </Radio>
          <Radio
            value="vault"
            description="purlis writes it only for each run, in a folder of the run's own, and removes it after."
          >
            In a vault entry
          </Radio>
        </Asked>
        {files.keyIn === "file" ? (
          <TextField
            label="Key"
            value={files.key}
            onChange={(key) => set({ key })}
            placeholder="~/.docker/key.pem"
          />
        ) : (
          <div className="pool-setup-grid">
            <TextField label="Vault" value={files.vault} onChange={(vault) => set({ vault })} />
            <TextField
              label="Key in the vault"
              value={files.name}
              onChange={(name) => set({ name })}
            />
          </div>
        )}
      </Options>
      {step.see && <SeeWhat text={step.see.text} note={step.see.note} />}
      <Action
        does={step.next?.id === "connect-machine" ? step.next.does : undefined}
        // Named for the machine only once everything typed is kept, as the SSH pick is: what a
        // press of it reads is what Connect keeps.
        main={route !== "" && route === kept && same && host ? `Connect to ${host}` : "Connect"}
        onMain={onConnect}
        busy={busy || route === "" || route !== kept || !same}
      />
    </>
  );
}

/** **The authority a TLS machine's certificate is signed by**, to check before it is trusted. */
function ConfirmCa({
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
  const ca = machine?.ca;
  if (!machine || !ca) return null;
  const host = machine.host ?? "the machine";
  return (
    <>
      <h2 className="pool-setup-h2">Check it&apos;s the right machine&apos;s authority</h2>
      {step.findings.map((finding) => (
        <p key={finding} className="pool-setup-does">
          {finding}
        </p>
      ))}
      <Facts aria-label="The authority">
        <Fact label="Issued to">{ca.subject ?? "No name it gives"}</Fact>
        <Fact label="Its file">{ca.file}</Fact>
      </Facts>
      <CodeBlock
        label={`${host}'s authority, SHA-256`}
        text={ca.fingerprint}
        copy="Copy fingerprint"
      />
      {step.see && <SeeWhat open note={step.see.note} text={step.see.text} />}
      <Action
        does={step.next?.does}
        main="Trust this machine"
        reserve={["Trust this machine", "Continue"]}
        icon={<KeyRound aria-hidden size={16} />}
        busy={busy}
        onMain={() => onTrust(ca.fingerprint)}
      >
        <Button variant="ghost" onPress={onNotIt} isDisabled={busy}>
          This isn&apos;t it
        </Button>
      </Action>
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
  const over = overOf(step.machine);
  const verdicts = verdictsOf(step.cause, done, over);
  return (
    <>
      <h2 className="pool-setup-h2">{`Docker on ${host}`}</h2>
      <CheckList aria-label={`What purlis checked on ${host}`}>
        {checksOf(over).map((check, at) => (
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
  const [podmans, setPodmans] = useState<readonly PoolSetupPodmanConnection[]>([]);
  const [pressing, setPressing] = useState<Pressing>();
  const [trouble, setTrouble] = useState<string>();
  useEffect(() => {
    let live = true;
    void commands.poolSetupSshHosts().then((answer) => {
      if (live && answer.status === "ok") setHosts(answer.data);
    });
    void commands.poolSetupPodmanConnections().then((answer) => {
      if (live && answer.status === "ok") setPodmans(answer.data);
    });
    return () => {
      live = false;
    };
  }, []);
  const host = step.machine?.host ?? "the machine";
  // The route chosen: a kept tcp:// machine is reached over TLS, until the person picks another.
  const [chosenOver, setOver] = useState<Over>();
  const over = chosenOver ?? overOf(step.machine);
  const pick = (route: string | null, identity: string | null = null) => {
    void commands.poolSetupMachine(route, identity).then((answer) => {
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
  /** **Check and trust again** (#6270): a made pool's machine met again, without its pin, for the
   *  person to check what it shows now. */
  const checkAndTrustAgain = (pool: string) => {
    setPressing("connecting");
    setTrouble(undefined);
    void commands.poolSetupCheckAndTrustAgain(plane, pool).then((answer) => {
      setPressing(undefined);
      if (answer.status === "error") setTrouble(answer.error);
      else onShown(answer.data);
    });
  };
  const trust = (fingerprint: string) => {
    setPressing("trusting");
    setTrouble(undefined);
    // A made pool met again keeps the pin confirmed in place of its own (#6270).
    const made = shown.pool;
    void (
      made === null
        ? commands.poolSetupTrust(plane, fingerprint)
        : commands.poolSetupTrustAgain(plane, made, fingerprint)
    ).then((answer) => {
      setPressing(undefined);
      if (answer.status === "error") setTrouble(answer.error);
      else onShown(answer.data);
    });
  };
  const keepFiles = (files: TlsTyped | null) => {
    void commands
      .poolSetupTlsFiles(
        plane,
        files?.ca.trim() ?? "",
        files?.cert.trim() ?? "",
        files?.keyIn === "file" ? files.key.trim() : null,
        files?.keyIn === "vault" ? files.vault.trim() : null,
        files?.keyIn === "vault" ? files.name.trim() : null,
      )
      .then((answer) => {
        if (answer.status === "error") setTrouble(answer.error);
        else onShown(answer.data);
      });
  };
  const madePool = shown.pool !== null;
  // A made pool's machine is never forgotten: This isn't it reads it again as it stands.
  const notIt = () => (madePool ? checkAgain() : pick(null));
  const done = step.state === "done";
  const meeting = step.next?.id === "trust-machine";
  // A tcp:// machine is refused on its card until the person chooses a route (#6267): chosen
  // TLS, the pick with Options open says how to name its files; chosen SSH, it is read again.
  const refused =
    (step.cause === "plain-tcp" && chosenOver === undefined) || step.cause === "not-a-route";
  // Plain TCP and an address that isn't one are refusals of what was typed, never findings of
  // the machine: once the person chose a route they are only stale until the next read.
  const declined = step.cause === "plain-tcp" || step.cause === "not-a-route";
  const finding = step.state === "attention" && !declined;
  // A made pool whose pin no longer holds is checked and trusted again (#6270).
  const againPool = madePool && step.next?.id === "check-and-trust-again" ? shown.pool : null;

  return (
    <>
      <StepHead
        title={
          (finding || meeting) && madePool ? `Check ${host} again` : "Connect to the other machine"
        }
        tag={done ? <Badge variant="ok">Every check passed</Badge> : undefined}
        lead={
          done
            ? `${host} is ready to run your chats.`
            : over === "tls"
              ? "Choose the machine, and check it's the one you mean. purlis keeps no copy of your certificates: it reads them where they are."
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
      {!done && !finding && !meeting && !refused && !madePool && (
        <RouteChoice
          over={over}
          onChange={(to) => {
            setOver(to);
            // A machine kept for the other route is forgotten; one that fits this one stays.
            const route = step.machine?.route;
            const tcp = route?.startsWith("tcp://") === true;
            if (route && tcp !== (to === "tls")) pick(null);
          }}
        />
      )}
      {!done && !finding && !meeting && !refused && over === "ssh" && (
        <Pick
          step={step}
          hosts={hosts}
          podmans={podmans}
          busy={pressing !== undefined}
          onPick={pick}
          onConnect={connect}
        />
      )}
      {!done && !finding && !meeting && !refused && over === "tls" && (
        <TlsPick
          step={step}
          busy={pressing !== undefined}
          onPick={pick}
          onFiles={keepFiles}
          onConnect={connect}
        />
      )}
      {pressing === "connecting" && (
        <Busy>
          {over === "tls"
            ? `Checking ${host}'s certificate. This only reads.`
            : `Asking ${host} for its key. This only reads.`}
        </Busy>
      )}
      {meeting && over === "ssh" && (
        <Confirm step={step} busy={pressing !== undefined} onTrust={trust} onNotIt={notIt} />
      )}
      {meeting && over === "tls" && (
        <ConfirmCa step={step} busy={pressing !== undefined} onTrust={trust} onNotIt={notIt} />
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
            <>
              <Button variant="primary" onPress={notIt}>
                {step.next?.label ?? "Change the machine"}
              </Button>
              {step.cause === "plain-tcp" && (
                <Button variant="secondary" onPress={() => setOver("tls")}>
                  Use TLS instead
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
      {(done || finding) && <Checked step={step} done={done} />}
      {finding && (
        <Say
          tone="trouble"
          title={
            step.cause === "pool-pin-changed" && overOf(step.machine) === "tls"
              ? "The authority isn't the one you trusted"
              : (TITLES[step.cause ?? "here"] ?? TITLES.here ?? "")
          }
          actions={
            <>
              {againPool !== null ? (
                <>
                  <Button variant="primary" onPress={() => checkAndTrustAgain(againPool)}>
                    {step.next?.label ?? "Check and trust again"}
                  </Button>
                  <Button variant="secondary" onPress={checkAgain}>
                    Check again
                  </Button>
                </>
              ) : (
                <Button
                  variant="primary"
                  onPress={
                    step.next?.id === "change-machine" ? notIt : madePool ? checkAgain : connect
                  }
                >
                  {step.next?.id === "change-machine" ? "Change the machine" : "Check again"}
                </Button>
              )}
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
