// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { MessageSquarePlus } from "lucide-react";
import { commands, type PlaneId, type PoolSetupCheck, type PoolSetupStep } from "../bindings";
import { Badge } from "../ui/badge/Badge";
import { Button, ButtonRow } from "../ui/button/Button";
import { CheckLine, CheckList, type CheckVerdict } from "../ui/checks/Checks";
import { Action, Busy, Say, StepHead } from "./parts";

const VERDICTS: readonly string[] = ["ok", "missing", "running", "todo", "warn", "skip"];
const verdictOf = (check: PoolSetupCheck): CheckVerdict =>
  VERDICTS.includes(check.verdict) ? (check.verdict as CheckVerdict) : "todo";

/** What went wrong on a try, in the core's words, and where to go about it. */
type Failed = { why: string; step: string | null };

/** Every word Try it's main button can say, so the button doesn't move as it changes. */
const START_WORDS = ["Start a test chat", "Starting a test chat"] as const;

/**
 * **Try it, and the last screen** (#4483, Q32, stories 47 to 50): its own start button, a real chat
 * started on the pool, each check as the core reads it, and at the end what passed with one button
 * to open a chat there. A skipped step reads "works, except for this" and never as done. A failure
 * stays on this step, says what happened and the one thing to try, and is tried again here.
 */
export function TryStep({
  step,
  pool,
  plane,
  workspace,
  changed,
  onGo,
  onOpenChat,
}: {
  step: PoolSetupStep;
  pool: string;
  plane: PlaneId;
  /** The workspace the chat starts in, where the tab is filed in one; else the project's first. */
  workspace?: string;
  changed: () => void;
  /** Goes to another step: the one a failure names. */
  onGo: (step: string) => void;
  /** Opens a chat on the pool, in the window's own start: in `workspace`, where Try it's chat
   *  started, once it has here. */
  onOpenChat: (workspace?: string) => void;
}) {
  const [trying, setTrying] = useState(false);
  const [failed, setFailed] = useState<Failed>();
  /** Where Try it's chat started, this visit: where Open a chat opens. */
  const [triedIn, setTriedIn] = useState<string>();
  const checks = step.checks;
  const passed = step.state === "done";
  // Anything that didn't pass, skipped or not done, keeps the end from reading as done.
  const except = checks.some((check) => check.verdict !== "ok");

  const run = async () => {
    setTrying(true);
    setFailed(undefined);
    const tried = await commands.poolSetupTry(plane, pool, workspace ?? null);
    setTrying(false);
    if (tried.status === "error") return setFailed({ why: tried.error, step: null });
    if (tried.data.failed !== null) return setFailed(tried.data.failed);
    setTriedIn(tried.data.workspace ?? undefined);
    changed();
  };

  const lines = (label: string) => (
    <CheckList aria-label={label}>
      {checks.map((check, at) => (
        <CheckLine
          key={check.title}
          // The chat's start is the one check a try makes; the core puts it first.
          verdict={trying && at === 0 && check.verdict === "todo" ? "running" : verdictOf(check)}
          title={check.title}
        >
          {check.more}
        </CheckLine>
      ))}
    </CheckList>
  );

  if (passed && failed === undefined)
    return (
      <>
        <StepHead
          title={except ? `${pool} works, except for this` : `${pool} is ready`}
          tag={except ? undefined : <Badge variant="ok">Everything passed</Badge>}
          lead={
            except
              ? "You can start chats there now. Here's what they can and can't do."
              : `A real chat started on ${pool}, so chats you start there will run.`
          }
        />
        {lines("What was checked")}
        <ButtonRow size="lg">
          <Button
            variant="primary"
            icon={<MessageSquarePlus aria-hidden size={16} />}
            onPress={() => onOpenChat(triedIn)}
          >
            {`Open a chat on ${pool}`}
          </Button>
        </ButtonRow>
        <p className="pool-setup-does">
          {except
            ? "Finish the rest any time: Pool setup in the palette comes back to this pool."
            : "Change anything later from Pool setup in the palette."}
        </p>
      </>
    );

  return (
    <>
      <StepHead
        title={trying ? "Trying it for real" : failed ? "Try it didn't pass" : "Try it for real"}
        lead={
          failed
            ? failed.step === null
              ? "Your earlier steps are fine and stay done."
              : "Mend this on the step it names, then try again here."
            : `Setup ends when a real chat has started on ${pool}, so you know it works before you rely on it.`
        }
      />
      {lines(trying ? "The checks" : "What gets checked")}
      {trying && (
        <Busy>
          {`Starting a chat on ${pool}. This can take a minute or two the first time, while the runner gets ready.`}
        </Busy>
      )}
      {failed && !trying && (
        <Say
          tone="trouble"
          title="What happened"
          actions={
            failed.step !== null ? (
              <>
                <Button variant="primary" onPress={() => onGo(failed.step ?? "")}>
                  {failed.step === "signins" ? "Go to Sign-ins" : "Go to that step"}
                </Button>
                <Button variant="secondary" onPress={() => void run()}>
                  Try again
                </Button>
              </>
            ) : (
              <Button variant="primary" onPress={() => void run()}>
                Try again
              </Button>
            )
          }
        >
          {failed.why}
        </Say>
      )}
      {!failed && (
        <Action
          does={step.next?.does}
          main={trying ? START_WORDS[1] : (step.next?.label ?? START_WORDS[0])}
          reserve={START_WORDS}
          busy={trying}
          onMain={() => void run()}
        />
      )}
    </>
  );
}
