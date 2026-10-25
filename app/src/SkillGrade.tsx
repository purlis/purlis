// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  commands,
  SKILL_GRADE,
  type ProfileRow,
  type SkillGradePlan,
  type SkillGradeStep,
  type SkillLevel,
} from "./bindings";
import { counted } from "./dispatches";
import { Notice } from "./Notice";
import { Button, ButtonRow } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Select } from "./primitives/Select";

/** The project's default profile, in the picker. */
const DEFAULT = "";

/** How a grade that ended is said, in one line. */
export function endedSaid(step: SkillGradeStep): string {
  const chats = counted(step.ran, "chat", "chats");
  switch (step.ended) {
    case "whole":
      return `Graded in ${chats}. The scorecard is in Grades below.`;
    case "stopped":
      return `Stopped after ${chats}. What ran is scored in Grades below.`;
    case "chats-did-not-start":
      return `The grade stopped: its chats did not start. ${step.said ?? ""}`.trim();
    default:
      return "";
  }
}

/** What a grade should spend, from the last one, in one sentence: never a figure that was not
 *  measured. */
export function costSaid(plan: SkillGradePlan): string {
  const estimate = plan.estimate;
  if (estimate === null || (estimate.tokens === null && estimate.cost_usd === null))
    return "This skill has not been graded here before, so purlis cannot say yet what a grade spends. Each chat is one turn, and ends once the skill starts.";
  const parts: string[] = [];
  if (estimate.tokens !== null)
    parts.push(`about ${Math.round(estimate.tokens).toLocaleString("en-US")} tokens`);
  if (estimate.cost_usd !== null) parts.push(`about $${estimate.cost_usd.toFixed(2)}`);
  return `From its last grade, of ${counted(estimate.chats_then, "chat", "chats")}, this one should spend ${parts.join(", ")}.`;
}

/**
 * **Grade, on a copy of a skill** (SK-16, #2779; decisions 22 and 43): the cost first, then the
 * grade. The plan says what it runs (each trigger test, with the skill and without it, the
 * times it runs), what the last grade spent, and whose account pays; nothing runs until the
 * person presses Grade on it. A grade's chats are drawn nowhere: this says how far it is, offers
 * Stop, and says how it ended. Its scorecard is the skill's Grades panel.
 */
export function SkillGrade({
  plane,
  name,
  level,
  onGraded,
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  /** Told when a grade of this skill ended, so the tab reads its grades again. */
  onGraded: () => void;
}) {
  const [plan, setPlan] = useState<SkillGradePlan>();
  const [refused, setRefused] = useState<string>();
  const [asking, setAsking] = useState(false);
  const [profiles, setProfiles] = useState<readonly ProfileRow[]>([]);
  const [profile, setProfile] = useState(DEFAULT);
  const [step, setStep] = useState<SkillGradeStep>();
  const [said, setSaid] = useState<{ tone: "news" | "trouble"; text: string }>();
  const running = step !== undefined && step.ended === null;

  useEffect(() => {
    const listening = listen<SkillGradeStep>(SKILL_GRADE, (event) => {
      const now = event.payload;
      if (now.plane !== plane || now.skill !== name) return;
      setStep(now);
      if (now.ended !== null) {
        setSaid({
          tone: now.ended === "chats-did-not-start" ? "trouble" : "news",
          text: endedSaid(now),
        });
        onGraded();
      }
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane, name, onGraded]);

  const open = () => {
    setAsking(true);
    setSaid(undefined);
    void commands.skillGradePlan(plane, level, name).then((answer) => {
      if (answer.status === "ok") {
        setPlan(answer.data);
        setRefused(undefined);
      } else setRefused(answer.error);
    });
    void commands
      .startOptions(plane)
      .then((answer) => {
        if (answer.status === "ok") setProfiles(answer.data.profiles);
      })
      .catch(() => undefined);
  };

  const start = () => {
    void commands
      .skillGradeStart(plane, level, name, profile === DEFAULT ? null : profile)
      .then((answer) => {
        if (answer.status === "ok") {
          setAsking(false);
          setStep({
            plane,
            skill: name,
            grade: answer.data,
            ran: 0,
            of: plan?.chats ?? 0,
            ended: null,
            said: null,
          });
        } else setSaid({ tone: "trouble", text: answer.error });
      });
  };

  return (
    <section aria-labelledby="skill-grade-heading" data-testid="skill-grade">
      <h3 id="skill-grade-heading">Grade</h3>
      {said !== undefined && (
        <Notice
          cause="skill-grade"
          tone={said.tone}
          label={said.tone === "trouble" ? "Not graded" : "Graded"}
          onDismiss={() => setSaid(undefined)}
        >
          {said.text}
        </Notice>
      )}
      {running ? (
        <ButtonRow>
          <p data-testid="skill-grade-running">
            Grading {name}: {step.ran} of {counted(step.of, "chat", "chats")} done.
          </p>
          <Button
            size="sm"
            variant="danger"
            onPress={() => void commands.skillGradeStop(plane, step.grade)}
          >
            Stop
          </Button>
        </ButtonRow>
      ) : !asking ? (
        <ButtonRow>
          <p className="honest">
            A grade runs {name}'s trigger tests in chats with it and without it, and shows what it
            costs before anything runs.
          </p>
          <Button size="sm" onPress={open}>
            Grade…
          </Button>
        </ButtonRow>
      ) : refused !== undefined ? (
        <EmptyState
          headline={`purlis could not plan a grade of ${name}`}
          body={refused}
          testid="skill-grade-refused"
        />
      ) : plan === undefined ? (
        <p className="pending" aria-busy="true">
          Reading what a grade runs…
        </p>
      ) : (
        <GradePlan
          name={name}
          plan={plan}
          profiles={profiles}
          profile={profile}
          onProfile={setProfile}
          onStart={start}
          onCancel={() => setAsking(false)}
        />
      )}
    </section>
  );
}

/** The cost-first plan: what the grade runs and spends, and the press that starts it. */
function GradePlan({
  name,
  plan,
  profiles,
  profile,
  onProfile,
  onStart,
  onCancel,
}: {
  name: string;
  plan: SkillGradePlan;
  profiles: readonly ProfileRow[];
  profile: string;
  onProfile: (to: string) => void;
  onStart: () => void;
  onCancel: () => void;
}) {
  const options = [
    { value: DEFAULT, label: "The project's default" },
    ...profiles.map((one) => ({ value: one.name, label: one.name })),
  ];
  const nothing = plan.tests.length === 0;
  return (
    <div data-testid="skill-grade-plan">
      {plan.tests_refused !== null && (
        <p className="honest" data-testid="skill-grade-tests-refused">
          The trigger tests could not be read: {plan.tests_refused}
        </p>
      )}
      {nothing ? (
        <EmptyState
          headline={`No trigger tests for ${name} yet`}
          body="Trigger tests are the queries a skill should start on and the near misses it should leave alone, kept with the skill. Add some to its files, and Grade runs them."
          testid="skill-grade-none"
        />
      ) : (
        <>
          <p data-testid="skill-grade-cost">
            Grading runs {counted(plan.chats, "chat", "chats")}, one turn each: each of the{" "}
            {counted(plan.tests.length, "trigger test", "trigger tests")}{" "}
            {counted(plan.runs, "time", "times")} with the skill and{" "}
            {counted(plan.runs, "time", "times")} without it. {costSaid(plan)} The chats run on the
            profile you pick, under this project's sandbox, and its account pays for them as for any
            chat.
          </p>
          <ol aria-label={`Trigger tests of ${name}`}>
            {plan.tests.map((test, at) => (
              <li key={at} data-testid="skill-grade-test">
                <code>{test.query}</code> ·{" "}
                {test.should_trigger ? "should start it" : "should leave it alone"}
                {test.hides && (
                  <span className="honest"> · holds a character no reader would see</span>
                )}
              </li>
            ))}
          </ol>
          {plan.left_out.length > 0 && (
            <p className="honest" data-testid="skill-grade-left-out">
              Left out: {plan.left_out.join(" ")}
            </p>
          )}
        </>
      )}
      {plan.outcome_evals > 0 && (
        <p className="honest">
          {counted(plan.outcome_evals, "outcome eval waits", "outcome evals wait")} for a later
          version of purlis: this grade runs the trigger tests alone.
        </p>
      )}
      {plan.outcomes_refused !== null && (
        <p className="honest">The outcome evals could not be read: {plan.outcomes_refused}</p>
      )}
      <ButtonRow>
        {!nothing && (
          <>
            <Select
              aria-label={`Grade ${name} on`}
              value={profile}
              options={options}
              onValueChange={onProfile}
            />
            <Button
              size="sm"
              variant="primary"
              isDisabled={plan.running !== null}
              onPress={onStart}
            >
              Grade
            </Button>
          </>
        )}
        <Button size="sm" variant="ghost" onPress={onCancel}>
          Cancel
        </Button>
      </ButtonRow>
    </div>
  );
}
