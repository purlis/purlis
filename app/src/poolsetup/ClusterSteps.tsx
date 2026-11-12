// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { PoolSetupCheck, PoolSetupContexts, PoolSetupStep } from "../bindings";
import { CheckLine, CheckList, type CheckVerdict } from "../ui/checks/Checks";
import { Radio, RadioGroup } from "../ui/choice/Radio";
import { Action, Options, Recommended, SeeWhat, StepHead } from "./parts";

/**
 * **Pool setup's cluster steps** (#4492, spec #4469): what Where chooses for a cluster, and Check
 * the cluster, doctor's lines in its own words. What a step finds is the core's
 * (`purlis_core::poolsetup::cluster`), so `purlis pool setup` says the same.
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
