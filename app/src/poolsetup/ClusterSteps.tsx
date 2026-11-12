// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import {
  commands,
  type PoolSetupCheck,
  type PoolSetupContexts,
  type PoolSetupPart,
  type PoolSetupStep,
} from "../bindings";
import { Button } from "../ui/button/Button";
import { CheckLine, CheckList, type CheckVerdict } from "../ui/checks/Checks";
import { Radio, RadioGroup } from "../ui/choice/Radio";
import { CodeBlock } from "../ui/code/CodeBlock";
import { Disclosure } from "../ui/disclosure/Disclosure";
import { TextField } from "../ui/field/Field";
import { addClusterPool, checkClusterPoolKey } from "../pools";
import { Action, Busy, Options, Recommended, Say, SeeWhat, StepHead } from "./parts";

/**
 * **Pool setup's cluster steps** (#4492, #4493, spec #4469): what Where chooses for a cluster,
 * Check the cluster, doctor's lines in its own words, and Install, a part at a time. What a step
 * finds is the core's (`purlis_core::poolsetup::cluster`), so `purlis pool setup` says the same.
 */

const VERDICTS: readonly string[] = ["ok", "missing", "running", "todo", "warn", "skip"];
const verdictOf = (check: PoolSetupCheck): CheckVerdict =>
  VERDICTS.includes(check.verdict) ? (check.verdict as CheckVerdict) : "warn";

/**
 * **Which of the kubeconfig's contexts reaches the cluster** (Q30): every one, the recommended
 * marked. Only its name is kept, never what it signs in with.
 */
export function ContextOptions({
  contexts,
  value,
  onChange,
}: {
  contexts: PoolSetupContexts;
  value: string | undefined;
  onChange: (to: string) => void;
}) {
  if (contexts.names.length === 0) return null;
  return (
    <Options open={contexts.recommended === null && value === undefined}>
      <RadioGroup aria-label="Kubeconfig context" value={value ?? null} onChange={onChange}>
        {contexts.names.map((name) => (
          <Radio key={name} value={name}>
            <span className="pool-setup-choice">
              {name}
              {name === contexts.recommended && <Recommended />}
            </span>
          </Radio>
        ))}
      </RadioGroup>
      <p className="pool-setup-does">
        purlis reaches the cluster with your own sign-in, through kubectl, and keeps only the
        context&apos;s name.
      </p>
    </Options>
  );
}

const CHECK_LEAD =
  "Doctor looks at what the cluster has before anything changes. It only reads, with your own sign-in.";

/** What the waiting step says of its rechecks, held to the tab's and the background's pace. */
const WAITING_SAYS =
  "purlis checks the cluster again by itself, every half minute while this is open and every ten minutes while it's closed. Needs you gets one entry when it's ready.";

/**
 * **Check the cluster** (#4492): doctor's findings first, and a cluster that lacks something stops
 * here. Where doctor shows the person can't install (#4496), Send to your admin gives the words
 * to send, with what's missing and the commands, and the step then waits for the admin, across a
 * close, checking again by itself.
 */
export function CheckStep({
  step,
  plane,
  onNext,
  onRecheck,
}: {
  step: PoolSetupStep;
  /** The project the tab is in: where the ready entry is listed. */
  plane?: string;
  onNext: () => void;
  onRecheck: () => void;
}) {
  const [refused, setRefused] = useState<string>();
  const waiting = step.state === "waiting";
  const press = (id: string) => {
    setRefused(undefined);
    const done =
      id === "send-to-admin"
        ? commands.poolSetupAskAdmin(plane ?? null)
        : commands.poolSetupStopWaiting();
    void done.then((answer) => {
      if (answer.status === "error") setRefused(answer.error);
      onRecheck();
    });
  };
  const also = step.also;
  return (
    <>
      <StepHead title="Check the cluster" lead={CHECK_LEAD} />
      {waiting && (
        <Say tone="news" title="Waiting for your admin">
          {WAITING_SAYS}
        </Say>
      )}
      {step.findings.map((finding) => (
        <p key={finding} className="pool-setup-p">
          {finding}
        </p>
      ))}
      {step.checks.length > 0 && (
        <CheckList aria-label="What doctor found">
          {step.checks.map((check) => (
            <CheckLine key={check.title} verdict={verdictOf(check)} title={check.title}>
              {check.more}
            </CheckLine>
          ))}
        </CheckList>
      )}
      {waiting && step.admin && (
        <>
          <p className="pool-setup-p">
            Send these words to your cluster&apos;s admin, by chat or email. They hold no password,
            token, key or credential.
          </p>
          <CodeBlock label="For your admin" text={step.admin} />
        </>
      )}
      {refused && (
        <Say tone="trouble" title="That didn't work">
          {refused}
        </Say>
      )}
      {step.see && <SeeWhat text={step.see.text} note={step.see.note} />}
      {step.next ? (
        <Action does={also?.does ?? step.next.does} main={step.next.label} onMain={onRecheck}>
          {also && (also.id === "send-to-admin" || also.id === "stop-waiting") && (
            <Button variant="secondary" onPress={() => press(also.id)}>
              {also.label}
            </Button>
          )}
        </Action>
      ) : (
        step.state === "done" && <Action main="Continue" onMain={onNext} />
      )}
    </>
  );
}

/** How long typing rests before the operator's image is kept. */
const KEEP_IMAGE_MS = 400;

/** **The operator's image, by its digest** (Q30): kept as it is typed, once it rests. */
function OperatorImage({ open, changed }: { open: boolean; changed: () => void }) {
  const [typed, setTyped] = useState("");
  const [refused, setRefused] = useState<string>();
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    const later = setTimeout(() => {
      const image = typed.trim();
      void commands.poolSetupOperatorImage(image === "" ? null : image).then((answer) => {
        setRefused(answer.status === "error" ? answer.error : undefined);
        if (answer.status === "ok") changed();
      });
    }, KEEP_IMAGE_MS);
    return () => clearTimeout(later);
  }, [typed, changed]);
  return (
    <Options open={open}>
      <TextField
        label="Operator image"
        value={typed}
        onChange={setTyped}
        placeholder="ghcr.io/purlis/purlis-cluster-operator@sha256:…"
        help="By its digest, so the cluster runs exactly the image you checked. See what this does says how to find it and check its signature."
        error={refused}
      />
    </Options>
  );
}

/** One part of the install: whether it is there, what it is, and its own button. */
function PartLine({
  part,
  busy,
  onApply,
}: {
  part: PoolSetupPart;
  busy: boolean;
  onApply: (part: PoolSetupPart) => void;
}) {
  const next = part.next;
  const pressable = next !== null && next.id !== "choose-operator-image";
  return (
    <CheckLine
      verdict={busy ? "running" : part.done ? "ok" : "todo"}
      title={part.name}
      trail={
        pressable && (
          <Button
            variant="primary"
            isDisabled={busy}
            aria-busy={busy}
            onPress={() => onApply(part)}
          >
            {next.label}
          </Button>
        )
      }
    >
      {part.line}
      {next && <span className="pool-setup-does"> {next.does}</span>}
    </CheckLine>
  );
}

/**
 * **Install** (#4493): upstream, definitions, signing key and operator, each on its own button
 * that says what it applies, each run again on its own. A failure is said under the part, with
 * what to try, and nothing else changes. Uninstalling is never a button: its commands, and how
 * many people they affect.
 */
export function InstallStep({
  step,
  onNext,
  changed,
}: {
  step: PoolSetupStep;
  onNext: () => void;
  changed: () => void;
}) {
  const [applying, setApplying] = useState<string>();
  const [failed, setFailed] = useState<{ part: PoolSetupPart; why: string }>();
  const apply = (part: PoolSetupPart) => {
    if (applying !== undefined) return;
    setFailed(undefined);
    setApplying(part.id);
    void commands.poolSetupInstall(part.id).then((answer) => {
      setApplying(undefined);
      if (answer.status === "error") setFailed({ part, why: answer.error });
      changed();
    });
  };
  const naming = step.parts.some((part) => part.next?.id === "choose-operator-image");
  return (
    <>
      <StepHead title="Install" />
      {step.findings.map((finding) => (
        <p key={finding} className="pool-setup-p">
          {finding}
        </p>
      ))}
      {step.parts.length > 0 && (
        <CheckList aria-label="Parts of the install">
          {step.parts.map((part) => (
            <PartLine key={part.id} part={part} busy={applying === part.id} onApply={apply} />
          ))}
        </CheckList>
      )}
      {applying !== undefined && (
        <Busy>Applying it to your cluster. This can take a few minutes.</Busy>
      )}
      {failed && (
        <Say
          tone="trouble"
          title={`${failed.part.name} wasn't installed`}
          actions={
            <Button variant="secondary" onPress={() => apply(failed.part)}>
              Try again
            </Button>
          }
        >
          {failed.why}
        </Say>
      )}
      {step.state !== "done" && <OperatorImage open={naming} changed={changed} />}
      {step.see && <SeeWhat text={step.see.text} note={step.see.note} />}
      {step.undo && (
        <Disclosure label="Uninstalling" level={2} className="pool-setup-fold">
          <div className="pool-setup-fold-body">
            <p className="pool-setup-does">{step.undo.note}</p>
            <CodeBlock label="Commands" text={step.undo.text} />
          </div>
        </Disclosure>
      )}
      {step.next?.id === "check-again" ? (
        <Action does={step.next.does} main={step.next.label} onMain={changed} />
      ) : (
        step.state === "done" && <Action main="Continue" onMain={onNext} />
      )}
    </>
  );
}

const POOL_LEAD =
  "A pool is a named place your chats run. On a cluster, each chat's runner is a pod the cluster operator makes for you alone.";

/** Where the Pool step on a cluster has got to, held above its screen so leaving loses nothing. */
type Claiming =
  | { state: "idle" }
  | { state: "busy"; said: string }
  | { state: "keyed"; pool: string; key: string; namespace: string }
  | { state: "refused"; why: string };

/** The main button's words as a key is pinned. */
const PIN_AND_ADD = "Pin the key and add the pool";
const PIN_AGAIN = "Pin the new key";
const PASTE_FIRST =
  "Paste the key your cluster's admin gave you first. purlis pins nothing it can't check against it.";

/**
 * **The key a person checks before it is pinned** (#4495, F on #4469): the key the Pool names,
 * shown whole, and a box for the key their cluster's admin gave them. A teammate must paste it:
 * the core compares the two and pins nothing else. An admin who set the cluster up may leave it
 * empty, and pins the key their own cluster shows them.
 */
function KeyToCheck({
  pool,
  named,
  pinned,
  joining,
  pasted,
  paste,
  error,
}: {
  pool: string;
  named: string;
  /** The key pinned before, where the Pool names another now. */
  pinned?: string;
  joining: boolean;
  pasted: string;
  paste: (pasted: string) => void;
  /** Why nothing was pinned yet, said on the box. */
  error?: string;
}) {
  return (
    <>
      <Say tone="attention" title="Check the cluster operator key">
        {pinned === undefined
          ? `Pool ${pool} names the key below. purlis pins it on this machine and links only to runners its operator vouches for with it.`
          : `Pool ${pool} now names the key below, and not the one pinned here. Your cluster's admin knows whether they changed it.`}{" "}
        {joining
          ? "Paste the key your admin gave you, and purlis pins it only if it is the same."
          : "If someone else set this cluster up, paste the key they gave you, and purlis pins it only if it is the same."}
      </Say>
      {pinned !== undefined && <CodeBlock label="Key pinned here" text={pinned} />}
      <CodeBlock
        label={pinned === undefined ? "Key the pool names" : "Key the pool names now"}
        text={named}
      />
      <TextField
        label={joining ? "Key your admin gave you" : "Key your admin gave you (optional)"}
        value={pasted}
        onChange={paste}
        help="Spaces, line breaks and capitals don't matter."
        error={error}
      />
    </>
  );
}

/**
 * **The Pool step on a cluster** (#4494, #4495): makes a Pool in the cluster where it has none, on
 * its own button, then claims this person's runner of one: it reads the cluster operator key the
 * Pool names and shows it whole, and only a second press pins it and claims the runner, which then
 * becomes the pool this setup goes on with. A teammate joining types the pool's name where they
 * can't list the pools, and pastes their admin's key before anything is pinned. A key that changed
 * since it was pinned is checked the same way, and pinned again.
 */
export function ClusterPoolStep({
  step,
  contexts,
  image,
  pool: kept,
  onNext,
  onOpenPools,
  changed,
}: {
  step: PoolSetupStep;
  contexts: PoolSetupContexts;
  /** The runner image this build names, by digest, where it names one. */
  image: string | undefined;
  /** The pool this setup made, where it made one. */
  pool?: string | null;
  onNext: () => void;
  onOpenPools?: () => void;
  changed: () => void;
}) {
  const pools = contexts.pools;
  const [name, setName] = useState("");
  const [teamImage, setTeamImage] = useState("");
  const [typedPool, setTypedPool] = useState("");
  const [pasted, setPasted] = useState("");
  const [pasteWhy, setPasteWhy] = useState<string>();
  const paste = (typed: string) => {
    setPasted(typed);
    setPasteWhy(undefined);
  };
  const [chosen, choose] = useState<string | undefined>(pools[0]);
  const [claiming, setClaiming] = useState<Claiming>({ state: "idle" });
  const context = contexts.chosen ?? contexts.recommended ?? undefined;
  const next = step.next;
  const busy = claiming.state === "busy";
  const making = next?.id === "create-cluster-pool";
  const typing = next?.id === "add-cluster-pool" && pools.length === 0;
  const checkingAgain = next?.id === "check-key-again" && contexts.named_now !== null;
  const named = name.trim() === "" ? "team" : name.trim();
  const runnerImage = teamImage.trim() === "" ? image : teamImage.trim();

  const create = async () => {
    if (runnerImage === undefined) {
      setClaiming({
        state: "refused",
        why: "This build of purlis names no runner image, so give one by its digest under Options.",
      });
      return;
    }
    setClaiming({ state: "busy", said: "Making the pool in your cluster." });
    const made = await commands.poolSetupClusterPool(named, runnerImage);
    if (made.status === "error") setClaiming({ state: "refused", why: made.error });
    else {
      choose(named);
      setClaiming({ state: "idle" });
      changed();
    }
  };

  const readKey = async () => {
    const pool = typing ? typedPool.trim() : (chosen ?? pools[0]);
    if (pool === undefined || context === undefined) return;
    if (pool === "") {
      setClaiming({
        state: "refused",
        why: "Type the pool's name your cluster's admin gave you, then read its key.",
      });
      return;
    }
    setClaiming({ state: "busy", said: "Reading the cluster operator key the pool names." });
    const key = await checkClusterPoolKey("", context, pool);
    if ("trouble" in key) setClaiming({ state: "refused", why: key.trouble });
    else setClaiming({ state: "keyed", pool, key: key.key, namespace: key.namespace });
  };

  const needsPaste = contexts.joining && pasted.trim() === "";

  const claim = async (keyed: Extract<Claiming, { state: "keyed" }>) => {
    if (context === undefined) return;
    if (needsPaste) {
      setPasteWhy(PASTE_FIRST);
      return;
    }
    setClaiming({ state: "busy", said: "Claiming your runner. This can take a few minutes." });
    const refused = await addClusterPool({
      name: keyed.pool,
      kubeconfig: null,
      context,
      pool: keyed.pool,
      key: pasted.trim() === "" ? keyed.key : pasted,
      vouch_wait: null,
    });
    if (refused !== undefined) {
      setClaiming({ state: "refused", why: refused.words });
      return;
    }
    const kept = await commands.poolSetupPool(keyed.pool);
    if (kept.status === "error") setClaiming({ state: "refused", why: kept.error });
    else {
      setClaiming({ state: "idle" });
      changed();
    }
  };

  const pinAgain = async () => {
    if (kept === null || kept === undefined) return;
    if (pasted.trim() === "") {
      setPasteWhy(PASTE_FIRST);
      return;
    }
    setClaiming({ state: "busy", said: "Checking the key and pinning it." });
    const pinned = await commands.poolSetupPinAgain(kept, pasted);
    if (pinned.status === "error") setClaiming({ state: "refused", why: pinned.error });
    else {
      setPasted("");
      setClaiming({ state: "idle" });
      changed();
    }
  };

  return (
    <>
      <StepHead title="Pool" lead={POOL_LEAD} />
      {step.state === "done" ? (
        <p className="pool-setup-p">
          <strong>Chats will run on:</strong> {step.line}.
        </p>
      ) : (
        step.findings.map((finding) => (
          <p key={finding} className="pool-setup-p">
            {finding}
          </p>
        ))
      )}
      {step.state === "done" && contexts.pinned !== null && (
        <>
          <p className="pool-setup-p">
            This machine pinned the cluster operator key below. Teammates who join this pool check
            theirs against it, so send it to them yourself, apart from the pool&apos;s name.
          </p>
          <CodeBlock label="Cluster operator key pinned here" text={contexts.pinned} />
        </>
      )}
      {typing && claiming.state !== "keyed" && (
        <TextField
          label="Pool's name"
          value={typedPool}
          onChange={setTypedPool}
          help="As your cluster's admin gave it to you."
        />
      )}
      {!making && pools.length > 1 && step.state !== "done" && (
        <RadioGroup aria-label="Pool" value={chosen ?? null} onChange={choose}>
          {pools.map((pool, at) => (
            <Radio key={pool} value={pool}>
              <span className="pool-setup-choice">
                {pool}
                {at === 0 && <Recommended />}
              </span>
            </Radio>
          ))}
        </RadioGroup>
      )}
      {making && (
        <Options>
          <TextField
            label="Name"
            value={name}
            onChange={setName}
            placeholder="team"
            help="Lower-case letters, digits and dashes. Recommended: team."
          />
          <TextField
            label="Runner image"
            value={teamImage}
            onChange={setTeamImage}
            placeholder={image}
            help={
              image === undefined
                ? "By its digest. This build of purlis names none."
                : "By its digest. Recommended: purlis's image of this version, shown in the box."
            }
          />
        </Options>
      )}
      {claiming.state === "busy" && <Busy>{claiming.said}</Busy>}
      {claiming.state === "keyed" && (
        <>
          <KeyToCheck
            pool={claiming.pool}
            named={claiming.key}
            joining={contexts.joining}
            pasted={pasted}
            paste={paste}
            error={pasteWhy}
          />
          <p className="pool-setup-p">Your runner is claimed in namespace {claiming.namespace}.</p>
        </>
      )}
      {checkingAgain && contexts.named_now !== null && (
        <KeyToCheck
          pool={kept ?? ""}
          named={contexts.named_now}
          pinned={contexts.pinned ?? undefined}
          joining
          pasted={pasted}
          paste={paste}
          error={pasteWhy}
        />
      )}
      {claiming.state === "refused" && (
        <Say tone="trouble" title="That didn't work">
          {claiming.why}
        </Say>
      )}
      {step.see && <SeeWhat text={step.see.text} note={step.see.note} />}
      {step.state === "done" ? (
        <Action main="Continue" onMain={onNext} />
      ) : claiming.state === "keyed" ? (
        <Action
          does={
            pasted.trim() === ""
              ? "Pins the key shown on this machine, claims your runner of the pool and pairs it here."
              : "Pins the key only if it is the one you pasted, then claims your runner of the pool and pairs it here."
          }
          main={PIN_AND_ADD}
          onMain={() => void claim(claiming)}
          busy={busy}
        />
      ) : checkingAgain ? (
        <Action
          does="Pins the key the pool names now, only if it is the one you pasted. Your runner stays the one paired here."
          main={PIN_AGAIN}
          onMain={() => void pinAgain()}
          busy={busy}
        />
      ) : next?.id === "remove-pool" ? (
        <Action does={next.does} main={next.label} onMain={onOpenPools} />
      ) : next && (next.id === "create-cluster-pool" || next.id === "add-cluster-pool") ? (
        <Action
          does={next.does}
          main={next.label}
          busy={busy}
          onMain={() => void (making ? create() : readKey())}
        />
      ) : (
        next && <Action does={next.does} />
      )}
    </>
  );
}
