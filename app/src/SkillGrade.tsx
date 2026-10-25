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
import { TextField } from "./ui/field/Field";
import { Select } from "./ui/select/Select";

/** The project's default profile, in the picker. */
const DEFAULT = "";

/** A number of tokens as the window says it: `200,000 tokens`. */
export function tokensSaid(tokens: number): string {
  return `${Math.round(tokens).toLocaleString("en-US")} ${tokens === 1 ? "token" : "tokens"}`;
}

/** How a grade that ended is said, in one line. */
export function endedSaid(step: SkillGradeStep): string {
  const chats = counted(step.ran, "chat", "chats");
  const spent = step.tokens === null ? "" : `, having spent ${tokensSaid(step.tokens)}`;
  switch (step.ended) {
    case "whole":
      return `Graded in ${chats}. The scorecard is in Grades below.`;
    case "stopped":
      return `Stopped after ${chats}. What ran is scored in Grades below.`;
    case "chats-did-not-start":
      return `The grade stopped: its chats did not start. ${step.said ?? ""}`.trim();
    case "over-cap":
      return `Stopped at its cap of ${tokensSaid(step.cap ?? 0)} after ${chats}${spent}. What ran is scored in Grades below.`;
    case "spend-not-reported":
      return `Stopped after ${chats}: ${step.said ?? "a chat did not say what it spent, so the grade could not be held to its cap."}`;
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
  const picked = profile === DEFAULT ? null : profile;

  // A grade still running from before this tab was drawn is shown, with its Stop: the app's
  // word, never this tab's memory of it (the review of #3229).
  useEffect(() => {
    let live = true;
    void commands.skillGradePlan(plane, level, name, null).then((answer) => {
      if (!live || answer.status !== "ok" || answer.data.running === null) return;
      const grade = answer.data.running;
      setStep(
        (was) =>
          was ?? {
            plane,
            skill: name,
            grade,
            ran: 0,
            of: answer.data.chats,
            tokens: null,
            cap: answer.data.token_cap,
            ended: null,
            said: null,
          },
      );
    });
    return () => {
      live = false;
    };
  }, [plane, level, name]);

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

  const read = (on: string | null) =>
    void commands.skillGradePlan(plane, level, name, on).then((answer) => {
      if (answer.status === "ok") {
        setPlan(answer.data);
        setRefused(undefined);
      } else setRefused(answer.error);
    });

  const open = () => {
    setAsking(true);
    setSaid(undefined);
    void commands.skillGradePlan(plane, level, name, picked).then((answer) => {
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
      .skillGradeStart(plane, level, name, picked, plan?.consent ?? "")
      .then((answer) => {
        if (answer.status === "ok") {
          setAsking(false);
          setStep({
            plane,
            skill: name,
            grade: answer.data,
            ran: 0,
            of: plan?.chats ?? 0,
            tokens: null,
            cap: plan?.token_cap ?? 0,
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
          label={
            said.tone === "trouble" ? "Not graded" : step?.ended === "whole" ? "Graded" : "Stopped"
          }
          onDismiss={() => setSaid(undefined)}
        >
          {said.text}
        </Notice>
      )}
      {running ? (
        <ButtonRow>
          <p data-testid="skill-grade-running">
            Grading {name}: {step.ran} of {counted(step.of, "chat", "chats")} done
            {step.tokens === null
              ? ""
              : `, ${tokensSaid(step.tokens)} of ${tokensSaid(step.cap ?? 0)}`}
            .
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
          onCap={(tokens) =>
            void commands.skillGradeSetCap(plane, level, name, picked, tokens).then((answer) => {
              if (answer.status === "ok") {
                setPlan(answer.data);
                setSaid(undefined);
              } else setSaid({ tone: "trouble", text: answer.error });
            })
          }
          profiles={profiles}
          profile={profile}
          onProfile={(to) => {
            setProfile(to);
            read(to === DEFAULT ? null : to);
          }}
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
  onCap,
  profiles,
  profile,
  onProfile,
  onStart,
  onCancel,
}: {
  name: string;
  plan: SkillGradePlan;
  onCap: (tokens: number) => void;
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
  const [cap, setCap] = useState(String(plan.token_cap ?? 0));
  const capped = Number(cap.replace(/[,\s]/g, ""));
  return (
    <div data-testid="skill-grade-plan">
      {plan.tests_refused !== null && (
        <p className="honest" data-testid="skill-grade-tests-refused">
          {plan.tests_refused}
        </p>
      )}
      {plan.refused !== null && (
        <p className="honest" data-testid="skill-grade-cannot-run">
          No grade can run on {plan.profile ?? "this profile"}: {plan.refused}
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
            {counted(plan.runs, "time", "times")} without it, up to{" "}
            {counted(plan.chats, "chat", "chats")} × {plan.chat_minutes} min. {costSaid(plan)} It
            stops once its chats have spent {tokensSaid(plan.token_cap ?? 0)}
            {plan.cap_set_here ? ", the cap you set on this machine" : ", the default cap"}. The
            chats run on the profile you pick
            {plan.sandbox === null ? "" : `, in the project's ${plan.sandbox}`}, with none of your
            own Allows and no tools but purlis's that only read, and its account pays for them as
            for any chat.
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
      {plan.outcomes_refused !== null && <p className="honest">{plan.outcomes_refused}</p>}
      {!nothing && (
        <ButtonRow>
          <TextField
            size="sm"
            aria-label="Token cap"
            inputMode="numeric"
            value={cap}
            onChange={setCap}
          />
          <Button
            size="sm"
            isDisabled={!Number.isInteger(capped) || capped === plan.token_cap}
            onPress={() => onCap(capped)}
          >
            Set the cap
          </Button>
        </ButtonRow>
      )}
      <ButtonRow>
        {!nothing && (
          <>
            <Select
              size="sm"
              aria-label={`Grade ${name} on`}
              value={profile}
              options={options}
              onValueChange={onProfile}
            />
            <Button
              size="sm"
              variant="primary"
              isDisabled={plan.running !== null || plan.refused !== null || plan.consent === null}
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
