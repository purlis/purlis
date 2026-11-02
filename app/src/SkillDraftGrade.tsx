// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A draft's grade, beside Accept** (#2780; `prototype/skill-draft-grade`, layout A): the line in
 * a draft's fixed footer that says how the draft did against the current version, and the ways on
 * from it. The plan, the running page and the scorecard are a skill's grade's. No state of a grade
 * touches Accept: it is what the person weighs, never a gate.
 */
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  commands,
  SKILL_GRADE,
  type DraftScore,
  type GradeRow,
  type SkillDraftGrade,
  type SkillGradeStep,
} from "./bindings";
import { counted } from "./dispatches";
import { pointsSaid, signed } from "./GradeScorecard";
import { useRead } from "./SkillPanels";
import { Badge } from "./ui/badge/Badge";
import { Button } from "./ui/button/Button";
import { ProgressBar } from "./ui/meter/Meter";

/** **What a draft's footer shows of its grade.** */
export type DraftGradeState =
  | { at: "not-graded" }
  | { at: "running"; step: SkillGradeStep }
  /** A grade, whole, stopped early (`stopped`) or of what the draft held before (`changed`). */
  | {
      at: "graded";
      row: GradeRow;
      scores: readonly DraftScore[];
      stopped: string | null;
      changed: boolean;
    }
  /** A grade that ended with nothing to compare. */
  | { at: "nothing-compared"; said: string }
  | { at: "only-evals" }
  | { at: "no-tests"; refused: string | null };

/** How a grade that compared nothing ended, in a sentence. */
const NOTHING_SAID: Record<GradeRow["outcome"], string> = {
  graded: "Finished with nothing to compare.",
  stopped: "Stopped before anything could be compared.",
  "at-limit": "Stopped at its limit before anything could be compared.",
  interrupted: "Interrupted before anything could be compared.",
};

/** **The state of `grade`**, the core's word on it, read once. */
export function draftGradeState(grade: SkillDraftGrade): DraftGradeState {
  if (grade.only_evals) return { at: "only-evals" };
  if (grade.running !== null) return { at: "running", step: grade.running };
  const row = grade.latest;
  if (row !== null) {
    if (grade.scores.length === 0)
      return { at: "nothing-compared", said: NOTHING_SAID[row.outcome] };
    const of = grade.asked === 0 ? "" : ` of ${grade.asked}`;
    return {
      at: "graded",
      row,
      scores: grade.scores,
      changed: grade.changed,
      stopped:
        row.outcome === "graded"
          ? null
          : `Stopped after ${row.spent.chats}${of} chats, so this compares what ran.`,
    };
  }
  if (grade.tests === 0 && grade.evals === 0)
    return { at: "no-tests", refused: grade.tests_refused };
  return { at: "not-graded" };
}

/** The draft's difference from what it is weighed against, the core's word on it: none where
 *  that arm did not run, as a difference from nothing is no difference. */
function Difference({ delta }: { delta: number | null }) {
  if (delta === null) return null;
  if (delta === 0) return <Badge>No difference</Badge>;
  return <Badge variant={delta > 0 ? "ok" : "waiting"}>{`${signed(delta)} points`}</Badge>;
}

/** One arm's pass rate, as the line says it. */
function Figure({ label, points }: { label: string; points: number | null }) {
  return (
    <span className="skill-draft-figure">
      <span>{label}</span> <b className="ui-text-strong">{pointsSaid(points)}</b>
    </span>
  );
}

/** **The line in a draft's footer**: see the module. */
export function DraftGradeLine({ name, state }: { name: string; state: DraftGradeState }) {
  switch (state.at) {
    case "not-graded":
      return <p>Not graded yet. A grade shows its cost before it runs.</p>;
    case "running": {
      const label = `Grading: ${state.step.ran} of ${counted(state.step.of, "chat", "chats")}`;
      return (
        <ProgressBar label={label} value={state.step.ran} maxValue={Math.max(state.step.of, 1)} />
      );
    }
    case "nothing-compared":
      return <p>{state.said}</p>;
    case "only-evals":
      return (
        <p>
          This draft changes only the skill&apos;s tests, so there is nothing to grade against the
          current version.
        </p>
      );
    case "no-tests":
      return (
        <p>{state.refused ?? `${name} has no trigger tests yet, so a grade has nothing to run.`}</p>
      );
    case "graded": {
      return (
        <>
          {state.scores.map(({ evals, arms, delta }) => (
            <div
              className="skill-draft-figures"
              data-testid="skill-draft-figures"
              key={String(evals)}
            >
              {evals && <span>Outcome evals</span>}
              <Figure label="Draft" points={arms.with} />
              {arms.previous !== null && <Figure label="Current" points={arms.previous} />}
              <Figure label="No skill" points={arms.without} />
              <Difference delta={delta} />
            </div>
          ))}
          {state.changed && (
            <p>
              <Badge variant="waiting">Out of date</Badge>{" "}
              <span>The draft changed after this grade.</span>
            </p>
          )}
          {state.stopped !== null && <p>{state.stopped}</p>}
        </>
      );
    }
  }
}

/** **The ways on from the line**, between it and Discard. */
export function DraftGradeActions({
  state,
  isNew,
  onGrade,
  onShow,
  onScorecard,
  onAskForTests,
}: {
  state: DraftGradeState;
  /** A new skill: it has no current version, and no folder a chat could write its tests in. */
  isNew: boolean;
  onGrade: () => void;
  /** Shows the grade running. */
  onShow: (grade: string) => void;
  onScorecard: (grade: string) => void;
  onAskForTests: () => void;
}) {
  const again = (
    <Button size="sm" onPress={onGrade}>
      Grade again…
    </Button>
  );
  switch (state.at) {
    case "not-graded":
      return (
        <Button size="sm" onPress={onGrade}>
          {isNew ? "Grade…" : "Grade against the current version…"}
        </Button>
      );
    case "running":
      return (
        <Button size="sm" onPress={() => onShow(state.step.grade)}>
          Show it
        </Button>
      );
    case "nothing-compared":
      return again;
    case "only-evals":
      return null;
    case "no-tests":
      return state.refused !== null || isNew ? null : (
        <Button size="sm" onPress={onAskForTests}>
          Ask a chat to write them
        </Button>
      );
    case "graded":
      return (
        <>
          {!state.changed && (
            <Button size="sm" variant="ghost" inline onPress={() => onScorecard(state.row.grade)}>
              Scorecard
            </Button>
          )}
          {again}
        </>
      );
  }
}

/**
 * **The grade of the draft `id`, as its page holds it**: the app's word, read again when the
 * project's skills change and when a grade of it ends, with each step of a grade running over it.
 * `undefined` until it is read, and where it could not be: the footer then says only what Accept
 * does, as a grade is never a gate.
 */
export function useDraftGrade(
  plane: string,
  id: string,
  changed: number,
): SkillDraftGrade | undefined {
  const [again, setAgain] = useState(0);
  const [step, setStep] = useState<SkillGradeStep>();
  const { data } = useRead(() => commands.skillDraftGrade(plane, id), [plane, id, changed, again]);
  useEffect(() => {
    const listening = listen<SkillGradeStep>(SKILL_GRADE, (event) => {
      const now = event.payload;
      if (now.plane !== plane || now.draft !== id) return;
      if (now.ended === null) setStep(now);
      else {
        setStep(undefined);
        setAgain((n) => n + 1);
      }
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane, id]);
  // Anything that is no grade is no grade: the page, and Accept on it, stands without one.
  if (typeof data !== "object" || data === null || !("only_evals" in data)) return undefined;
  return step === undefined ? data : { ...data, running: step };
}
