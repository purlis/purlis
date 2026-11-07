// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Resuming a stopped grade** (#5718, the operator's rulings of 2026-11-08): the scorecard's way
 * on beside Grade again, and the question that confirms it.
 *
 * - **Offered only while nothing that decides the result changed** (the core's plan says what
 *   did, `skill_grade_resume_plan`): otherwise Grade again alone, and a line saying what changed.
 * - **Confirmed like any grade**: the chats left, about how many tokens they would spend, and a
 *   cap of its own that counts only them; the scorecard then shows both runs' spend.
 * - **It says when it can run**: a usage limit's reset is on the button, which waits for it.
 *   Nothing resumes by itself.
 */
import { useState } from "react";
import { commands, type SkillGradeResumePlan } from "./bindings";
import { counted } from "./dispatches";
import { Notice, type NoticeAction } from "./Notice";
import { dollarsSaid, tokensSaid } from "./SkillGrade";
import { AlertDialog } from "./ui/dialog/Dialog";

/** A number as the window writes it in a line: `40,000`. */
function number(value: number): string {
  return Math.round(value).toLocaleString("en-US");
}

/** **The time a usage limit resets, on this machine's clock**: `14:00`, or with its day where it
 *  is not today. */
export function resetSaid(at: string, now: Date = new Date()): string {
  const when = new Date(at);
  const time = when.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
  if (when.toDateString() === now.toDateString()) return time;
  return `${when.toLocaleDateString("en-GB", { weekday: "short" })} ${time}`;
}

/** **The Resume button's words**: when it can run, where the program said (ruling 4). */
export function resumeLabel(plan: SkillGradeResumePlan, now: Date = new Date()): string {
  return plan.waits_until === null
    ? "Resume"
    : `Resume after your limit resets at ${resetSaid(plan.waits_until, now)}`;
}

/** **What resuming would run and spend**, in the operator's words (ruling 3): `About 40,000
 *  tokens left to run; cap 200,000.` */
export function leftSaid(plan: SkillGradeResumePlan): string {
  const cap = `cap ${number(plan.token_cap ?? 0)}`;
  return plan.tokens_left === null
    ? `${counted(plan.left, "chat", "chats")} left to run; what they spend is not known yet; ${cap}.`
    : `About ${number(plan.tokens_left)} tokens left to run; ${cap}.`;
}

/** **What the stopped run spent**, which the resumed grade's scorecard adds to its own. */
export function earlierSaid(plan: SkillGradeResumePlan): string {
  const spent = [
    plan.earlier_tokens === null ? null : tokensSaid(plan.earlier_tokens),
    plan.earlier_usd === null ? null : dollarsSaid(plan.earlier_usd),
  ].filter((part) => part !== null);
  const so_far =
    spent.length === 0
      ? "What it spent so far was not said"
      : `It spent ${spent.join(", ")} so far`;
  return `${so_far}. The resumed grade's scorecard shows both runs, and what they spent together.`;
}

/** **The way on Resume gives a stopped grade's end**, or none where it is not offered. */
export function resumeFix(
  plan: SkillGradeResumePlan | undefined,
  onAsk: () => void,
  now: Date = new Date(),
): NoticeAction | undefined {
  if (plan === undefined || plan.refused !== null) return undefined;
  return {
    label: resumeLabel(plan, now),
    onPress: onAsk,
    off:
      plan.waits_until !== null
        ? `Your usage limit resets at ${resetSaid(plan.waits_until, now)}; Resume can run then.`
        : plan.running !== null
          ? "A grade of this skill is running: Resume can run once it ends."
          : undefined,
  };
}

/**
 * **The question that confirms a resume** (ruling 3): what stopped it, the chats it leaves, about
 * what they would spend and the cap they are held to, and what the stopped run spent. Resume
 * starts it; a refusal is said in the dialog, which stays.
 */
export function ResumeQuestion({
  plane,
  plan,
  onClose,
  onResumed,
}: {
  plane: string;
  plan: SkillGradeResumePlan;
  onClose: () => void;
  onResumed: (grade: string) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const resume = () => {
    if (plan.consent === null) return;
    setBusy(true);
    setRefused(null);
    void commands
      .skillGradeResume(plane, plan.grade, plan.consent)
      .then((answer) => {
        if (answer.status === "ok") onResumed(answer.data);
        else setRefused(answer.error);
      })
      .catch((err: unknown) => setRefused(String(err)))
      .finally(() => setBusy(false));
  };
  const on = plan.model === null ? plan.profile : `${plan.profile ?? ""}, ${plan.model}`;
  return (
    <AlertDialog
      onClose={onClose}
      title={`Resume the grade of ${plan.skill}?`}
      description={
        <>
          <p data-testid="grade-resume-left">{leftSaid(plan)}</p>
          <p>
            {`${plan.ended}. Resume runs only the cases it did not finish, on ${on ?? "the same profile"}, and keeps what ran. It stops at its cap or at ${dollarsSaid(plan.limit_usd ?? 0)}, whichever comes first.`}
          </p>
          <p data-testid="grade-resume-earlier">{earlierSaid(plan)}</p>
          {refused !== null && (
            <Notice
              cause={`skill-grade-resume:${plan.grade}`}
              tone="trouble"
              label="Not resumed"
              onDismiss={() => setRefused(null)}
            >
              <span data-testid="grade-resume-refused">{refused}</span>
            </Notice>
          )}
        </>
      }
      main={{
        children: busy ? "Resuming…" : "Resume",
        reserve: ["Resume", "Resuming…"],
        isDisabled: busy || plan.consent === null,
        onPress: resume,
      }}
    />
  );
}
