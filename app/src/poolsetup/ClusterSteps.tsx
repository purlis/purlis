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

/** **Check the cluster** (#4492): doctor's findings first, and a cluster that lacks something stops here. */
export function CheckStep({
  step,
  onNext,
  onRecheck,
}: {
  step: PoolSetupStep;
  onNext: () => void;
  onRecheck: () => void;
}) {
  return (
    <>
      <StepHead title="Check the cluster" lead={CHECK_LEAD} />
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
      {step.see && <SeeWhat text={step.see.text} note={step.see.note} />}
      {step.next ? (
        <Action does={step.next.does} main={step.next.label} onMain={onRecheck} />
      ) : (
        step.state === "done" && <Action main="Continue" onMain={onNext} />
      )}
    </>
  );
}

const INSTALL_LEAD =
  "Install puts purlis's cluster operator on your cluster with your own sign-in. Each part has its own button, and nothing changes until you press it.";

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
      <StepHead title="Install" lead={INSTALL_LEAD} />
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
