// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import {
  commands,
  type IdentitiesShown,
  type IdentityFinished,
  type IdentityRow,
  type PlaneId,
  type PoolSetupStep,
} from "../bindings";
import { AgentIdentityFlow, answer, whyOf } from "../AgentIdentityFlow";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import { CodeBlock } from "../ui/code/CodeBlock";
import { Action, Busy, Say, StepHead } from "./parts";

/**
 * **Pool setup's Agent identity step** (#4490, stories 32 to 36 of #4469). An identity is set up in
 * the step itself, with the flow the Agent identity tab uses ({@link AgentIdentityFlow}, story 34), so the
 * person never leaves the guide. The tab stays where an identity is inspected and removed.
 *
 * What the step stands at is the core's (`purlis_core::poolsetup`): set up, to set up, or repos on a
 * forge host purlis can't set up by itself yet, which is said plainly with the tool line to add by
 * hand (story 36). What an identity acts as, and on how many repos, is GitHub's, read as the tab reads it.
 * Every state but done can be skipped, so the step never dead-ends.
 */
export function IdentityStep({
  step,
  pool,
  plane,
  changed,
  onNext,
  onOpenIdentity,
  onOpenSettings,
}: {
  step: PoolSetupStep;
  pool: string;
  plane: PlaneId;
  /** Read the setup again: the step moved. */
  changed: () => void;
  /** Go on to the next step. */
  onNext: () => void;
  /** Open the Agent identity tab, where an identity is inspected and removed. */
  onOpenIdentity?: () => void;
  /** Open the project's Settings, where a persona's tool is allowed on this machine. */
  onOpenSettings?: () => void;
}) {
  const [shown, setShown] = useState<IdentitiesShown>();
  const [trouble, setTrouble] = useState<string>();
  const [finished, setFinished] = useState<IdentityFinished>();
  const [skipping, setSkipping] = useState(false);
  const done = step.state === "done";
  const byHand = step.next?.id === "check-again" && step.see !== null;
  const flows = !done && step.next?.id === "set-up-identity";
  const reads = done || flows;

  const read = useCallback(() => {
    setTrouble(undefined);
    void answer(commands.agentIdentities(plane)).then(setShown, (why: unknown) =>
      setTrouble(whyOf(why)),
    );
  }, [plane]);
  useEffect(() => {
    if (reads) read();
  }, [reads, read]);

  const skip = (skipped: boolean) => {
    setSkipping(true);
    setTrouble(undefined);
    void commands.poolSetupSkip(pool, "identity", skipped).then((said) => {
      setSkipping(false);
      if (said.status === "error") setTrouble(said.error);
      else changed();
    });
  };
  const skipButton = (
    <Button variant="ghost" isDisabled={skipping} onPress={() => skip(true)}>
      Skip for now
    </Button>
  );
  const told = trouble && (
    <Say tone="trouble" title="That didn't work">
      {trouble}
    </Say>
  );

  if (step.state === "skipped")
    return (
      <>
        <StepHead title="Agent identity" />
        <Say tone="attention" title="You skipped this for now">
          In chats on {pool}, gh and git push say they have no identity until you set one up.
        </Say>
        {told}
        <Action main="Set it up now" busy={skipping} onMain={() => skip(false)} />
      </>
    );

  if (done) {
    const row = shown?.rows.find((one) => one.acts_as !== null) ?? null;
    return (
      <>
        <StepHead
          title={finished ? "The agent has its identity" : "Already set up"}
          tag={<Badge variant="ok">Done</Badge>}
          lead={`${actingAs(row, step.line)} ${finished ? "Chats won't ask about this again." : "There's nothing to repeat."}`}
        />
        {told}
        <p className="pool-setup-does">To inspect or remove it, open Agent identity.</p>
        <Action main="Continue to Try it" onMain={onNext}>
          {onOpenIdentity && (
            <Button variant="ghost" onPress={onOpenIdentity}>
              Open Agent identity
            </Button>
          )}
        </Action>
      </>
    );
  }

  if (byHand && step.see)
    return (
      <>
        <StepHead
          title="Set up the agent's identity by hand"
          lead="So that gh and git push work in a chat on this pool."
        />
        {step.findings.map((finding) => (
          <p key={finding} className="pool-setup-p">
            {finding}
          </p>
        ))}
        <p className="pool-setup-does">{step.see.note}</p>
        <CodeBlock label="The token, and the tool line" text={step.see.text} />
        {told}
        <Action does={step.next?.does} main={step.next?.label} onMain={changed}>
          {skipButton}
        </Action>
      </>
    );

  if (flows) {
    if (shown === undefined)
      return (
        <>
          <StepHead title="Agent identity" />
          {told ? (
            <Action main="Read again" onMain={read}>
              {skipButton}
            </Action>
          ) : (
            <Busy>Reading what's set up. This only reads.</Busy>
          )}
          {told}
        </>
      );
    return (
      <>
        {told}
        <AgentIdentityFlow
          key={shown.pending[0]?.app ?? "new"}
          plane={plane}
          shown={shown}
          onDone={(made) => {
            setFinished(made);
            read();
            changed();
          }}
          onAbandoned={() => read()}
          extra={skipButton}
        />
      </>
    );
  }

  // A persona's tool to allow on this machine (after Finish, or a line added by hand), or a forge
  // host to declare before anything can be set up there.
  const allow = step.next?.id === "allow-tool" && onOpenSettings !== undefined;
  const again = step.next?.id === "check-again";
  return (
    <>
      <StepHead title="Agent identity" />
      {step.findings.map((finding) => (
        <p key={finding} className="pool-setup-p">
          {finding}
        </p>
      ))}
      {told}
      <Action
        does={step.next?.does}
        main={allow || again ? step.next?.label : undefined}
        onMain={allow ? onOpenSettings : changed}
      >
        {allow && (
          <Button variant="secondary" onPress={changed}>
            Check again
          </Button>
        )}
        {skipButton}
      </Action>
    </>
  );
}

/** What the identity acts as, in a sentence: GitHub's word where it has one, else the core's. */
function actingAs(row: IdentityRow | null, line: string | null): string {
  if (row?.acts_as) {
    const repos = row.repos ?? 0;
    return `Acting as ${row.acts_as} on ${repos} ${repos === 1 ? "repo" : "repos"}.`;
  }
  return line ? `Set up as ${line}.` : "Set up.";
}
