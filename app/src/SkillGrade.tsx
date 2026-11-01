// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Grading a skill, as pages in the studio** (SK-16, #2779; decisions 22 and 43; #3688, the
 * amendment's S6): the plan, then the grade running. The scorecard and a saved chat are
 * `GradeScorecard.tsx`'s; the Grades tab that leads to them is `SkillGrades.tsx`'s.
 *
 * - **The plan** is a review page: its totals first (the chats, the most it may spend, the
 *   estimate, what it runs on), then the trigger tests it runs, where it runs and what it may
 *   spend, with Start fixed at the bottom saying what it may spend of whose account. Nothing runs
 *   until Start is pressed. **A plan that changed under the person** (Start refused on it, or
 *   the skill changed on disk) is never started: the page says so in the attention tone and Start
 *   waits until they read the new one.
 * - **A Grader extension** (#3236) is listed beside purlis's own runner where one is installed. The
 *   pick is in what Start consents to, and the plan names it plainly. Grading after each edit runs
 *   purlis's own alone.
 * - **A grade running** shows its chats done, what it has spent against its dollar limit and its
 *   tokens against its cap, with Stop fixed at the bottom. A grade's chats are drawn nowhere.
 */
import { useEffect, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  commands,
  SKILL_GRADE,
  type GradeGrader,
  type ProfileRow,
  type SkillGradeAfterEdit,
  type SkillGradePlan,
  type SkillGradeStep,
  type SkillLevel,
} from "./bindings";
import { counted } from "./dispatches";
import { NotRead, ReadingRows } from "./SkillPanels";
import { Notice, type NoticeAction } from "./Notice";
import { Button, ButtonRow } from "./ui/button/Button";
import { Checkbox } from "./ui/choice/Checkbox";
import { Tile, Totals } from "./ui/card/Totals";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { TextField } from "./ui/field/Field";
import { ProgressBar } from "./ui/meter/Meter";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { ReviewPage } from "./ui/review-page/ReviewPage";
import { Select } from "./ui/select/Select";

/** The project's default profile, or the profile's own model, in a picker. */
export const DEFAULT = "";

/** A number of tokens as the window says it: `200,000 tokens`. */
export function tokensSaid(tokens: number): string {
  const whole = Math.round(tokens);
  return counted(whole, "token", "tokens", whole.toLocaleString("en-US"));
}

/** How a grade that ended is said, in one line, with the chats that asked you something
 *  (SK-59): a grade's chat waits on nobody, so the grade says it, not a notice on a chat nobody
 *  can see. */
export function endedSaid(step: SkillGradeStep): string {
  const said = endSaid(step);
  if (step.asked === 0 || step.ended === "failed") return said;
  const them = step.asked === 1 ? "it" : "them";
  return `${said} ${counted(step.asked, "chat", "chats")} asked you something, and purlis ended ${them} there without counting ${them}: the grade's scorecard opens each.`;
}

/** **How a grade that ended is said**, before what its chats asked: a skill's grade's, and a
 *  router's (#4222), whose step says the same of its end. */
export function endSaid(
  step: Pick<SkillGradeStep, "ran" | "tokens" | "cap" | "ended" | "said">,
): string {
  const chats = counted(step.ran, "chat", "chats");
  const spent = step.tokens === null ? "" : `, having spent ${tokensSaid(step.tokens)}`;
  switch (step.ended) {
    case "whole":
      return step.said === null
        ? `Graded in ${chats}. Its scorecard says how each test went.`
        : `Graded in ${chats}. ${step.said}`;
    case "stopped":
      return `Stopped after ${chats}. ${step.said ?? "What ran is scored."}`;
    case "chats-did-not-start":
      return `The grade stopped: its chats did not start. ${step.said ?? ""}`.trim();
    case "over-cap":
      return `Stopped at its cap of ${tokensSaid(step.cap ?? 0)} after ${chats}${spent}. ${step.said ?? "What ran is scored."}`;
    case "over-limit":
      return `Stopped at its dollar limit after ${chats}${spent}. ${step.said ?? "What ran is scored."}`;
    case "spend-not-reported":
      return `Stopped after ${chats}: ${step.said ?? "a chat did not say what it spent, so the grade could not be held to its cap."}`;
    case "failed":
      return step.said ?? "A fault in purlis ended the grade before it was scored.";
    default:
      return "";
  }
}

/** **What a grade runs, by its fixtures** (SK-55): its trigger tests and its outcome evals, each
 *  counted, the words named in full where `full`. */
export function fixturesSaid(plan: SkillGradePlan, full = false): string {
  const tests = counted(
    plan.tests.length,
    full ? "trigger test" : "test",
    full ? "trigger tests" : "tests",
  );
  const on = plan.evals.filter((one) => one.on).length;
  const evals = counted(on, "outcome eval", "outcome evals");
  if (on === 0) return tests;
  if (plan.tests.length === 0) return evals;
  return `${tests} and ${evals}`;
}

/** What a grade should spend, from a grade before, in one sentence: never a figure that was not
 *  measured, and always which grade it is worked out from (#3273). A grade whose chats said
 *  nothing of what they spent was a grade still: its cost is not known. */
export function costSaid(
  plan: Pick<SkillGradePlan, "estimate"> & Partial<Pick<SkillGradePlan, "outcome_evals">>,
  /** Whose grade it is: a skill's, or a router's on its library (#3618). */
  of: "skill" | "router" = "skill",
): string {
  const estimate = plan.estimate;
  const turn =
    of === "skill"
      ? "Each chat is one turn, and ends once the skill starts."
      : "Each chat is one turn, and ends once the skill it tests starts.";
  if (estimate === null)
    return plan.outcome_evals !== undefined && plan.outcome_evals > 0
      ? "This skill's outcome evals have not been graded here before, so purlis cannot say yet what a grade spends. An outcome eval's chat runs its whole turn, and a judge chat reads what it left."
      : of === "skill"
        ? `This skill has not been graded here before, so purlis cannot say yet what a grade spends. ${turn}`
        : `purlis knows no grade of this router here to go by (one purlis stopped under before it scored a test says nothing of what a grade spends), so it cannot say yet what this one spends. ${turn}`;
  const on =
    estimate.profile === null
      ? ""
      : estimate.same_profile === false
        ? ` on another profile, ${estimate.profile}`
        : ` on ${estimate.profile}`;
  // The model its chats reported (SK-60): a grade on another model spends otherwise.
  const ranOn = [estimate.harness, estimate.model].filter((one) => one !== null).join(", ");
  const ran = `${on}${ranOn === "" ? "" : ` (${ranOn})`}, ${estimate.ts.slice(0, 10)}`;
  if (estimate.chats_then === 0)
    return `Its last grade${ran}, ran no chat, so purlis cannot say yet what a grade spends. ${turn}`;
  if (estimate.tokens === null && estimate.cost_usd === null)
    return `Its last grade${ran}, did not say what its chats spent, so purlis cannot say what this one spends. ${turn}`;
  const parts: string[] = [];
  if (estimate.tokens !== null) parts.push(`about ${tokensSaid(estimate.tokens)}`);
  if (estimate.cost_usd !== null) parts.push(`about $${estimate.cost_usd.toFixed(2)}`);
  // A grade purlis stopped under is worked out from what its chats spent before it stopped
  // (#3602), and the plan says it was interrupted.
  const which = estimate.interrupted ? "interrupted grade" : "grade";
  return `From its ${which}${ran}, of ${counted(estimate.chats_then, "chat", "chats")}, this one should spend ${parts.join(", ")}.`;
}

/** The dollar limit a grade is held to, and the price it was worked out at (#3469): the model's,
 *  or the dearest known where the model or its price is not. */
export function limitSaid(plan: Pick<SkillGradePlan, "cost_cap_usd" | "priced">): string {
  const limit = `$${(plan.cost_cap_usd ?? 0).toFixed(2)}`;
  const price = `$${(plan.priced.usd_per_million ?? 0).toFixed(2)} a million tokens`;
  if (plan.priced.of_the_model)
    return `${limit}, its tokens at ${plan.priced.model ?? "its model"}'s ${price}`;
  const why =
    plan.priced.model === null
      ? "as no model is named for its chats"
      : `as ${plan.priced.model}'s price is not known`;
  return `${limit}, its tokens at the dearest known price of ${price}, ${why}`;
}

/** What each edit graded under the opt-in runs and may spend, in one sentence (SK-58): the most
 *  chats and the dollar limit the person accepted, on the profile and model they picked. */
export function afterEditSaid(on: SkillGradeAfterEdit): string {
  const model = on.model === null ? "" : `, ${on.model}`;
  return `Each edit you save grades this skill on ${on.profile}${model}: up to ${counted(on.chats, "chat", "chats")}, and at most $${(on.limit_usd ?? 0).toFixed(2)} or ${tokensSaid(on.token_cap ?? 0)}, each time, paid by that profile's account.`;
}

/** **Why grading after each edit may run more chats than a grade now** (#4035): the previous
 *  version's arm its first saved edit brings, counted in what the person accepts. */
function previousArmSaid(name: string, plan: SkillGradePlan): string {
  const chats = `${counted(plan.after_edit_chats, "chat", "chats")}, not the ${plan.chats} a grade runs now`;
  return plan.previous_refused === null
    ? `Your first saved edit makes a version of ${name} to compare with, so each grade after an edit also runs that version: ${chats}.`
    : `Once its previous version can be compared, each grade after an edit also runs it: ${chats}.`;
}

/** Dollars as the window says them: `$1.92`. */
export function dollarsSaid(usd: number): string {
  return `$${usd.toFixed(2)}`;
}

/** What a plan's arms are, counted: with the skill, without it, and its previous version. */
function armsOf(plan: SkillGradePlan): number {
  return plan.previous === null ? 2 : 3;
}

/** **What changed in a plan**, in a clause: what the new one runs and may spend. */
export function planChangedSaid(
  now: Pick<SkillGradePlan, "chats" | "profile" | "model" | "cost_cap_usd"> & {
    /** The Grader it names (#3236), or why the one picked is gone: a skill's plan's. */
    grader?: GradeGrader | null;
    grader_refused?: string | null;
  },
): string {
  const on = [now.profile, now.model].filter((one) => one !== null).join(", ");
  const by = now.grader ? `, scored by ${graderSaid(now.grader)}` : "";
  const gone = now.grader_refused ? `. ${now.grader_refused}` : "";
  return `it now runs ${counted(now.chats, "chat", "chats")}${on === "" ? "" : ` on ${on}`}${by}, and may spend at most ${dollarsSaid(now.cost_cap_usd ?? 0)}${gone}`;
}

/** **Where a Grader can send what it is handed**, in a sentence: each host its manifest names. */
export function graderHostsSaid(hosts: readonly string[]): string {
  return hosts.length === 0
    ? "It reaches no host on the network."
    : `It can send what it is handed to ${hosts.join(", ")}, and to no other host.`;
}

/** **A Grader extension, as a plan names it**: its title, which it gives itself, and its id, which
 *  says whose program it is. */
export function graderSaid(grader: GradeGrader): string {
  return `${grader.title} (${grader.id})`;
}

/** Why the plan's main action can't be pressed now, or nothing where it can. */
function startRefused(
  name: string,
  plan: SkillGradePlan,
  newer: boolean,
  afterEdit: boolean,
): string | undefined {
  if (newer) return "The plan changed: read the new one first.";
  if (plan.tests.length === 0 && !plan.evals.some((one) => one.on))
    return plan.evals.length > 0
      ? `${name} has no trigger tests, so tick an outcome eval to run.`
      : `${name} has no trigger tests to run yet.`;
  if (plan.refused !== null) return `No grade can run on ${plan.profile ?? "this profile"}.`;
  if (!afterEdit && plan.running !== null) return `A grade of ${name} is running already.`;
  if (plan.grader_refused !== null) return plan.grader_refused;
  if (plan.consent === null) return "purlis could not say what this grade would run.";
  return undefined;
}

/**
 * **A grading page's heading**, and the line under it: in the page's flow, never held over what
 * scrolls under it (a review page's body scrolls, and the plan's text would run under it).
 */
export function GradingHeading({ name, description }: { name: string; description?: string }) {
  return (
    <div className="skill-grading-head">
      <h2 className="ui-text-strong">{name}</h2>
      {description !== undefined && <p className="honest">{description}</p>}
    </div>
  );
}

/**
 * **The grade plan** (S6): see the module. `afterEdit` makes it the plan that turns on grading
 * after each edit, whose main action turns that on instead of starting a grade. `changed` is
 * bumped when the project's skills change: the plan is read again, and one that changed is held
 * until the person reads it.
 */
export function GradePlanPage({
  plane,
  name,
  level,
  changed,
  afterEdit = false,
  draft,
  onStarted,
  onAfterEdit,
  onRunning,
  onCancel,
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  changed: number;
  afterEdit?: boolean;
  /** The draft of it this plan grades (#2780), by its id; `undefined` is the skill itself. */
  draft?: string;
  /** A grade started: its id. */
  onStarted: (grade: string) => void;
  /** Grading after each edit was turned on: what was accepted. */
  onAfterEdit: (on: SkillGradeAfterEdit) => void;
  /** A grade of this skill runs already: show it. */
  onRunning: (grade: string) => void;
  onCancel: () => void;
}) {
  const [plan, setPlan] = useState<SkillGradePlan>();
  // **A plan read since this one was drawn that consents to something else**: held, never drawn
  // under Start, until the person reads it.
  const [newer, setNewer] = useState<SkillGradePlan>();
  const [refused, setRefused] = useState<string>();
  const [said, setSaid] = useState<string>();
  const [profiles, setProfiles] = useState<readonly ProfileRow[]>([]);
  const [profile, setProfile] = useState(DEFAULT);
  // The model the chats run on (SK-60): the profile's own where none is picked.
  const [model, setModel] = useState(DEFAULT);
  // The Grader that scores it (#3236): purlis's own where none is picked.
  const [grader, setGrader] = useState(DEFAULT);
  const [again, setAgain] = useState(0);
  // The outcome evals the person ticked: none until they do, and the consent covers these.
  // Kept with the copy they were ticked on: another skill or level starts with none.
  const copy = JSON.stringify([plane, level, name]);
  const [tickedOn, setTickedOn] = useState<{ copy: string; evals: string[] }>({ copy, evals: [] });
  const ticked = tickedOn.copy === copy ? tickedOn.evals : [];
  const setTicked = (evals: string[]) => setTickedOn({ copy, evals });
  const picked = profile === DEFAULT ? null : profile;
  const pickedModel = model === DEFAULT ? null : model;
  const pickedGrader = grader === DEFAULT ? null : grader;
  // The level as a value: a page drawn again with an equal level reads nothing anew.
  const levelKey = JSON.stringify(level);
  const drawn = useRef<SkillGradePlan>(undefined);
  // The latest read: an older one's answer, for a pick since changed, is dropped.
  const latest = useRef(0);
  // Whether a read for a new pick is on its way: Start waits on what the pick consents to.
  const [reading, setReading] = useState(false);
  useEffect(() => {
    drawn.current = plan;
  });

  /** Reads the plan: drawn, or (`check`) held as newer where it consents to something else. */
  const read = (
    on: string | null,
    onModel: string | null,
    check = false,
    by: string | null = pickedGrader,
    evals: string[] = ticked,
  ) => {
    latest.current += 1;
    const asked = latest.current;
    const planned =
      draft === undefined
        ? commands.skillGradePlan(plane, level, name, on, onModel, evals, by)
        : commands.skillDraftGradePlan(plane, draft, on, onModel, evals, by);
    return void planned
      .then((answer) => {
        if (asked !== latest.current) return;
        setReading(false);
        if (answer.status !== "ok") {
          if (!check) setRefused(answer.error);
          return;
        }
        const was = drawn.current;
        if (check && was !== undefined && was.consent !== answer.data.consent)
          setNewer(answer.data);
        else {
          setPlan(answer.data);
          setNewer(undefined);
          setRefused(undefined);
        }
      })
      .catch((err: unknown) => {
        if (asked !== latest.current) return;
        setReading(false);
        if (!check) setRefused(String(err));
      });
  };

  useEffect(() => {
    read(picked, pickedModel);
    void commands
      .startOptions(plane)
      .then((answer) => {
        if (answer.status === "ok") setProfiles(answer.data.profiles);
      })
      .catch(() => undefined);
    // The plan is the page's as it opens, and again on Try again.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plane, levelKey, name, again]);

  // A change to the project's skills may change what a grade runs: read again, and held.
  const first = useRef(changed);
  useEffect(() => {
    if (changed === first.current) return;
    read(picked, pickedModel, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [changed]);

  if (refused !== undefined)
    return (
      <NotRead
        what={`what a grade of ${name} runs`}
        why={refused}
        onAgain={() => {
          setRefused(undefined);
          setAgain((n) => n + 1);
        }}
        testid="skill-grading-not-read"
      />
    );
  if (plan === undefined)
    return <ReadingRows what={`what a grade of ${name} runs`} testid="skill-grading-reading" />;

  const consent = plan.consent ?? "";
  const start = () =>
    void (
      draft === undefined
        ? commands.skillGradeStart(
            plane,
            level,
            name,
            picked,
            pickedModel,
            ticked,
            pickedGrader,
            consent,
          )
        : commands.skillDraftGradeStart(
            plane,
            draft,
            picked,
            pickedModel,
            ticked,
            pickedGrader,
            consent,
          )
    ).then((answer) => {
      if (answer.status === "ok") onStarted(answer.data);
      else {
        setSaid(answer.error);
        // The plan is read again after any refusal: one that changed since it was shown is
        // held, to be read and pressed again.
        read(picked, pickedModel, true);
      }
    });
  const turnOn = () =>
    void commands
      .skillGradeAfterEditOn(plane, level, name, picked, pickedModel, ticked, consent)
      .then((answer) => {
        if (answer.status === "ok") onAfterEdit(answer.data);
        else {
          setSaid(answer.error);
          read(picked, pickedModel, true);
        }
      });
  const why = reading
    ? "Reading the plan of the Grader you picked."
    : startRefused(name, plan, newer !== undefined, afterEdit);
  const limit = dollarsSaid(plan.cost_cap_usd ?? 0);
  const whose = plan.profile ?? "the profile";
  const nothing = plan.tests.length === 0 && plan.evals.length === 0;
  // Grading after each edit counts the arm a first saved edit brings (#4035): its chats, and
  // its estimate at the last grade's spend per chat.
  const chats = afterEdit ? plan.after_edit_chats : plan.chats;
  const arms = afterEdit && plan.after_edit_chats > plan.chats ? 3 : armsOf(plan);
  const scale = plan.chats > 0 ? chats / plan.chats : 1;
  const graded = plan.draft ? `the draft of ${name}` : name;
  return (
    <ReviewPage
      label={afterEdit ? `Grade ${name} after each edit` : `Grade ${graded}`}
      data-testid="skill-grade-plan"
      summary={
        afterEdit
          ? `Nothing runs now. Each edit you save then runs up to ${counted(plan.after_edit_chats, "chat", "chats")}, at most ${limit} each time, of ${whose}'s account.`
          : `Nothing runs until you press Start. It spends at most ${limit} of ${whose}'s account.${plan.grader === null ? "" : ` Scored by ${graderSaid(plan.grader)}, an extension you installed.`}`
      }
      onCancel={onCancel}
      main={{
        children: afterEdit
          ? "Turn on grading after each edit"
          : `Start grading · ${counted(plan.chats, "chat", "chats")}`,
        onPress: afterEdit ? turnOn : start,
        isDisabled: why !== undefined,
        tooltip: why,
        // It spends the person's money: pressed, never found by Return.
        byReturn: false,
      }}
    >
      <GradingHeading
        name={afterEdit ? `Grade ${name} after each edit` : `Grade ${graded}`}
        description={
          plan.draft
            ? `A grade runs the draft's trigger tests in chats with the draft, with the current version where there is one, and with neither, and shows what it may spend before anything runs. It stays on this machine.`
            : `A grade runs ${name}'s trigger tests in chats with it and without it, and shows what it may spend before anything runs.`
        }
      />
      {newer !== undefined && (
        <PlanChanged
          cause={`skill-grade-plan-changed:${name}`}
          newer={newer}
          testid="skill-grade-plan-changed"
          onShow={() => {
            setPlan(newer);
            setNewer(undefined);
            setSaid(undefined);
          }}
        />
      )}
      {said !== undefined && newer === undefined && (
        <Notice
          cause="skill-grade"
          tone="trouble"
          label="Not graded"
          onDismiss={() => setSaid(undefined)}
        >
          {said}
        </Notice>
      )}
      {plan.running !== null && !afterEdit && (
        <Notice
          cause={`skill-grade-running:${name}`}
          tone="news"
          label="A grade is running"
          fixes={[{ label: "Show it", onPress: () => onRunning(plan.running ?? "") }]}
        >
          {`A grade of ${name} is running: one runs at a time.`}
        </Notice>
      )}
      {!nothing && (
        <Totals aria-label={`What a grade of ${name} runs and spends`}>
          <Tile
            label="Chats"
            value={String(chats)}
            line={`${fixturesSaid(plan)} × ${plan.runs} × ${arms} arms${plan.judge_chats > 0 ? `, judges among them` : ""}`}
          />
          <Tile label="At most" value={limit} line={`or ${tokensSaid(plan.token_cap ?? 0)}`} />
          <EstimateTile estimate={plan.estimate} scale={scale} />
          <RunsOnTile plan={plan} />
        </Totals>
      )}
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
      <section aria-labelledby="skill-grade-runs">
        <PanelSectionTitle id="skill-grade-runs">What it runs</PanelSectionTitle>
        {nothing ? (
          <EmptyState
            variant="panel"
            headline={`No trigger tests or outcome evals for ${name} yet`}
            body="Trigger tests are the queries a skill should start on and the near misses it should leave alone; outcome evals are tasks it should do well, with what a judge checks. Both are kept with the skill. Add some to its files, and Grade runs them."
            testid="skill-grade-none"
          />
        ) : (
          <>
            <p data-testid="skill-grade-cost">
              Grading runs {counted(plan.chats, "chat", "chats")}, one turn each: each of the{" "}
              {fixturesSaid(plan, true)} {counted(plan.runs, "time", "times")} with{" "}
              {plan.draft ? "the draft" : "the skill"} and {counted(plan.runs, "time", "times")}{" "}
              without it
              {plan.previous === null
                ? ""
                : plan.draft
                  ? `, and ${counted(plan.runs, "time", "times")} with the current version, which it is compared with`
                  : `, and ${counted(plan.runs, "time", "times")} with its previous version, ${plan.previous.commit} (${plan.previous.subject}), which it is compared with`}
              {plan.judge_chats > 0
                ? `; and a judge chat for each outcome eval's chat, ${counted(plan.judge_chats, "judge chat", "judge chats")} in all, which reads what that chat left and changes nothing`
                : ""}
              , up to {counted(plan.chats, "chat", "chats")} × {plan.chat_minutes} min.{" "}
              {costSaid(plan)} It stops once its chats have spent {tokensSaid(plan.token_cap ?? 0)}
              {plan.cap_set_here ? ", the cap you set on this machine" : ", the default cap"}, or
              cost {limitSaid(plan)}. The chats run on the profile and model you pick
              {plan.sandbox === null ? "" : `, in the project's ${plan.sandbox}`}, with none of your
              own Allows and no tools but purlis's that only read, and its account pays for them as
              for any chat.
            </p>
            {plan.previous_refused !== null && (
              <p className="honest" data-testid="skill-grade-previous-refused">
                Its previous version is not compared: {plan.previous_refused}.
              </p>
            )}
            {plan.record_said !== null && (
              <p className="honest" role="note" data-testid="skill-grade-record">
                {plan.record_said}
              </p>
            )}
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
                Left out:{" "}
                {plan.left_out.map((left) => `Test ${left.at + 1}: ${left.why}`).join(" ")}
              </p>
            )}
          </>
        )}
        {plan.evals.length > 0 && (
          <ul aria-label={`Outcome evals of ${name}`} className="skill-grade-evals">
            {plan.evals.map((one) => (
              <li key={one.id} data-testid="skill-grade-eval">
                <Checkbox
                  isSelected={one.on}
                  onChange={(on) => {
                    const evals = on ? [...ticked, one.id] : ticked.filter((id) => id !== one.id);
                    setTicked(evals);
                    read(picked, pickedModel, false, pickedGrader, evals);
                  }}
                >
                  <code>{one.prompt}</code>
                </Checkbox>{" "}
                · {counted(one.chats, "more chat", "more chats")} ·{" "}
                {counted(one.expectations, "expectation", "expectations")} a judge checks
                {one.files.length > 0 && ` · with ${one.files.join(", ")}`}
                {one.hides && (
                  <span className="honest"> · holds a character no reader would see</span>
                )}
              </li>
            ))}
          </ul>
        )}
        {plan.outcomes_left_out.length > 0 && (
          <p className="honest" data-testid="skill-grade-evals-left-out">
            Left out:{" "}
            {plan.outcomes_left_out
              .map((left) => `Outcome eval ${left.id}: ${left.why}.`)
              .join(" ")}
          </p>
        )}
        {plan.outcomes_refused !== null && <p className="honest">{plan.outcomes_refused}</p>}
      </section>
      {!nothing && (
        <>
          <section aria-labelledby="skill-grade-where">
            <PanelSectionTitle id="skill-grade-where">Where it runs</PanelSectionTitle>
            <GradeOn
              profileLabel={`Grade ${name} on`}
              modelLabel={`Model ${name}'s grade runs on`}
              profiles={profiles}
              models={plan.models}
              profile={profile}
              model={model}
              onProfile={(to) => {
                // Another profile may be another program, whose models are its own.
                setProfile(to);
                setModel(DEFAULT);
                read(to === DEFAULT ? null : to, null);
              }}
              onModel={(to) => {
                setModel(to);
                read(picked, to === DEFAULT ? null : to);
              }}
            />
          </section>
          {afterEdit
            ? plan.graders.length > 0 && (
                <p className="honest" data-testid="skill-grade-after-edit-grader">
                  Each edit is graded by purlis&apos;s own runner. An extension&apos;s Grader is
                  picked for one grade at a time, on its plan.
                </p>
              )
            : (plan.graders.length > 0 || plan.grader_refused !== null) && (
                <section aria-labelledby="skill-grade-by">
                  <PanelSectionTitle id="skill-grade-by">Who scores it</PanelSectionTitle>
                  <Select
                    size="sm"
                    aria-label={`Who scores ${name}'s grade`}
                    value={grader}
                    options={[
                      { value: DEFAULT, label: "purlis's own runner" },
                      ...plan.graders.map((one) => ({ value: one.id, label: graderSaid(one) })),
                      // A pick no longer on offer stays what the picker says, until it is changed.
                      ...(plan.grader_refused !== null && pickedGrader !== null
                        ? [{ value: pickedGrader, label: pickedGrader }]
                        : []),
                    ]}
                    onValueChange={(to) => {
                      setGrader(to);
                      setReading(true);
                      read(picked, pickedModel, false, to === DEFAULT ? null : to);
                    }}
                  />
                  {plan.grader_refused !== null && (
                    <p className="honest" data-testid="skill-grade-grader-refused">
                      {plan.grader_refused}
                    </p>
                  )}
                  {plan.grader !== null && (
                    <p data-testid="skill-grade-grader">
                      {graderSaid(plan.grader)} is an extension you installed, not part of purlis.
                      It is handed the trigger tests above, asks purlis for the chats it wants, and
                      writes the scores this grade records. It is told each chat&apos;s id, which
                      skills it started, what it spent, how long it took and how it ended, not what
                      it said. {graderHostsSaid(plan.grader.hosts)} purlis keeps it from the model
                      services it knows, and holds it to this plan: up to{" "}
                      {counted(plan.chats, "chat", "chats")}, and the limits below. Asking past them
                      ends the grade.
                    </p>
                  )}
                </section>
              )}
          <section aria-labelledby="skill-grade-spend">
            <PanelSectionTitle id="skill-grade-spend">What it may spend</PanelSectionTitle>
            {afterEdit && (
              <p className="honest" data-testid="skill-grade-after-edit-offer">
                Each edit you save then runs this grade again, up to{" "}
                {counted(plan.after_edit_chats, "chat", "chats")} and at most {limitSaid(plan)}, on
                the profile and model you pick, paid by its account.
                {plan.after_edit_chats > plan.chats && ` ${previousArmSaid(name, plan)}`} If an edit
                makes a grade run more chats or spend more, it starts nothing and asks you again.
              </p>
            )}
            <TokenCap
              key={plan.token_cap ?? 0}
              cap={plan.token_cap ?? 0}
              onCap={(tokens) =>
                void (
                  draft === undefined
                    ? commands.skillGradeSetCap(
                        plane,
                        level,
                        name,
                        picked,
                        pickedModel,
                        ticked,
                        pickedGrader,
                        tokens,
                      )
                    : commands.skillDraftGradeSetCap(
                        plane,
                        draft,
                        picked,
                        pickedModel,
                        ticked,
                        pickedGrader,
                        tokens,
                      )
                ).then((answer) => {
                  if (answer.status === "ok") {
                    setPlan(answer.data);
                    setNewer(undefined);
                    setSaid(undefined);
                  } else setSaid(answer.error);
                })
              }
            />
          </section>
        </>
      )}
    </ReviewPage>
  );
}

/** **What a grade should spend, as a plan's totals say it**: the last grade's spend, `scale`d to
 *  the chats this one runs, or that it is not known. A skill's plan's and a router's (#4222). */
export function EstimateTile({
  estimate,
  scale = 1,
}: {
  estimate: SkillGradePlan["estimate"];
  scale?: number;
}) {
  return (
    <Tile
      label="Estimate"
      value={
        estimate?.cost_usd != null
          ? dollarsSaid(estimate.cost_usd * scale)
          : estimate?.tokens != null
            ? tokensSaid(Math.round(estimate.tokens * scale))
            : "Not known"
      }
      line={
        estimate === null
          ? "not graded here before"
          : estimate.interrupted
            ? "from its interrupted grade"
            : "from its last grade"
      }
    />
  );
}

/** **What a grade's chats run on, as a plan's totals say it**: the model and the profile. */
export function RunsOnTile({ plan }: { plan: Pick<SkillGradePlan, "model" | "profile"> }) {
  return (
    <Tile
      label="Runs on"
      value={plan.model ?? "Its own model"}
      line={plan.profile ?? "The project's default"}
    />
  );
}

/** **A plan that changed under the person**, in the attention tone, with the press that shows it
 *  (S6): Start waits on it. A skill's plan's and a router's (#4222). */
export function PlanChanged({
  cause,
  newer,
  testid,
  onShow,
}: {
  cause: string;
  newer: Parameters<typeof planChangedSaid>[0];
  testid: string;
  onShow: () => void;
}) {
  return (
    <Notice
      cause={cause}
      tone="attention"
      label="The plan changed"
      fixes={[{ label: "Show the new plan", onPress: onShow }]}
    >
      <span data-testid={testid}>
        {`The plan changed since you opened it: ${planChangedSaid(newer)}. Read it again before you start.`}
      </span>
    </Notice>
  );
}

/** **Where a grade's chats run, picked** (SK-60): the profile, and the model where its harness
 *  offers more than one. `DEFAULT` (`""`) picks the project's default, or the profile's own. */
export function GradeOn({
  profileLabel,
  modelLabel,
  profiles,
  models,
  profile,
  model,
  onProfile,
  onModel,
}: {
  profileLabel: string;
  modelLabel: string;
  profiles: readonly ProfileRow[];
  models: readonly string[];
  profile: string;
  model: string;
  onProfile: (to: string) => void;
  onModel: (to: string) => void;
}) {
  return (
    <ButtonRow>
      <Select
        size="sm"
        aria-label={profileLabel}
        value={profile}
        options={[
          { value: DEFAULT, label: "The project's default" },
          ...profiles.map((one) => ({ value: one.name, label: one.name })),
        ]}
        onValueChange={onProfile}
      />
      {models.length > 0 && (
        <Select
          size="sm"
          aria-label={modelLabel}
          value={model}
          options={[
            { value: DEFAULT, label: "The profile's own model" },
            ...models.map((one) => ({ value: one, label: one })),
          ]}
          onValueChange={onModel}
        />
      )}
    </ButtonRow>
  );
}

/** **How far a grade running is** (S6): its chats done, what it has spent against its dollar
 *  limit and its tokens against its cap, live. */
function GradeProgress({
  label,
  ran,
  of,
  tokens,
  cap,
  costUsd,
  limitUsd,
}: {
  label: string;
  ran: number;
  of: number;
  tokens: number | null;
  cap: number;
  costUsd: number | null;
  limitUsd: number | null;
}) {
  return (
    <>
      <Totals live aria-label={label}>
        <Tile
          label="Chats"
          value={`${ran} of ${of}`}
          said={`${ran} of ${counted(of, "chat", "chats")} done`}
        />
        <Tile
          label="Spent"
          value={costUsd !== null ? dollarsSaid(costUsd) : "Not said yet"}
          line={limitUsd === null ? undefined : `of at most ${dollarsSaid(limitUsd)}`}
        />
        <Tile
          label="Tokens"
          value={tokens !== null ? Math.round(tokens).toLocaleString("en-US") : "0"}
          line={`of ${tokensSaid(cap)}`}
        />
      </Totals>
      <ProgressBar
        label="Chats done"
        value={ran}
        maxValue={Math.max(of, 1)}
        valueLabel={`${ran} of ${of}`}
      />
      <p className="honest">
        You can leave this page: the grade keeps going, and its chats never show as needing you.
        Closing the app ends them.
      </p>
    </>
  );
}

/** The token cap a grade is held to, set on this machine before anything runs: a skill's grade's
 *  and a router's (#3618). */
export function TokenCap({ cap, onCap }: { cap: number; onCap: (tokens: number) => void }) {
  const [typed, setTyped] = useState(String(cap));
  const tokens = Number(typed.replace(/[,\s]/g, ""));
  return (
    <ButtonRow>
      <TextField
        size="sm"
        aria-label="Token cap"
        inputMode="numeric"
        value={typed}
        onChange={setTyped}
      />
      <Button
        size="sm"
        isDisabled={!Number.isInteger(tokens) || tokens === cap}
        onPress={() => onCap(tokens)}
      >
        Set the cap
      </Button>
    </ButtonRow>
  );
}

/** What every grade's step says, a skill's or a router's: what the running page draws. */
type RunningStepShown = Pick<
  SkillGradeStep,
  "plane" | "grade" | "ran" | "of" | "tokens" | "cost_usd" | "limit_usd" | "cap" | "ended" | "said"
>;

/**
 * **The page of a grade running** (S6; #4240): a skill's and a router's, given what differs: the
 * event its steps come on, how it is read, whether an end opens its scorecard, and the words.
 * Everything else is the one page: listening before reading, not read, gone, Stop, the end's
 * notice and how far it is.
 */
export function RunningGradePage<Step extends RunningStepShown>({
  whose,
  plane,
  grade,
  event,
  isOurs = () => true,
  read,
  of,
  opensItsScorecard,
  endWords,
  onEnded,
  onBack,
}: {
  /** Whose grade it is, in its page's test ids and its notices' causes. */
  whose: "skill" | "router";
  plane: string;
  grade: string;
  /** The event its steps are told on. */
  event: string;
  /** Whether a step of this plane and grade is this page's, where its id does not say all. */
  isOurs?: (step: Step) => boolean;
  /** The app's word on it: its latest step, or `null` where it runs no more. */
  read: () => Promise<{ status: "ok"; data: Step | null } | { status: "error"; error: string }>;
  /** What is graded, as "the grade of …" says it, from its step where one is told. */
  of: (step: Step | undefined) => string;
  /** Whether its end says all it has to on its scorecard, which then opens. */
  opensItsScorecard: (step: Step) => boolean;
  /** How the page says an end that stays on it. */
  endWords: (step: Step) => {
    tone: "news" | "trouble";
    fixes: readonly [NoticeAction, ...NoticeAction[]];
    said: ReactNode;
  };
  onEnded: () => void;
  /** Back to where it was started from: the grade keeps going. */
  onBack: () => void;
}) {
  const [step, setStep] = useState<Step>();
  // Whether the app said this grade runs, where it has: its latest step says the rest (#3928).
  const [runs, setRuns] = useState<boolean>();
  // Why the app's word on it could not be read, where it could not, and a press to read again.
  const [unread, setUnread] = useState<string>();
  const [again, setAgain] = useState(0);
  const [stopping, setStopping] = useState(false);
  const [stopRefused, setStopRefused] = useState<string>();
  // The latest of each, used as an event lands: the listener is not attached anew with them, so
  // no step is lost while it would be.
  const latest = useRef({ isOurs, read, opensItsScorecard, onEnded });
  useEffect(() => {
    latest.current = { isOurs, read, opensItsScorecard, onEnded };
  });

  // Listened to first, then the app's word on whether it runs and its latest step: an end told
  // before the listener was attached is never missed, as the read after it says it runs no more.
  useEffect(() => {
    let live = true;
    const listening = listen<Step>(event, (told) => {
      const now = told.payload;
      if (now.plane !== plane || now.grade !== grade || !latest.current.isOurs(now)) return;
      setStep(now);
      if (latest.current.opensItsScorecard(now)) latest.current.onEnded();
    }).catch(() => undefined);
    void listening
      .then(() => latest.current.read())
      .then((answer) => {
        if (!live) return;
        if (answer.status !== "ok") {
          setUnread(answer.error);
          return;
        }
        setUnread(undefined);
        const now = answer.data;
        setRuns(now !== null);
        // A step told since is newer than the one the read says.
        if (now !== null) setStep((told) => told ?? now);
        // Ended before this page listened: the event is gone, and its end is read here.
        if (now !== null && latest.current.opensItsScorecard(now)) latest.current.onEnded();
      })
      .catch((err: unknown) => {
        if (live) setUnread(String(err));
      });
    return () => {
      live = false;
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
  }, [event, plane, grade, again]);

  const graded = of(step);
  // A read that failed with no step told says so in place, never "Reading…" for good.
  if (step === undefined && unread !== undefined)
    return (
      <NotRead
        what={`the grade of ${graded}`}
        why={unread}
        onAgain={() => {
          setUnread(undefined);
          setAgain((n) => n + 1);
        }}
        testid={`${whose}-grading-not-read`}
      />
    );
  if (step === undefined && runs === undefined)
    return <ReadingRows what={`the grade of ${graded}`} testid={`${whose}-grading-reading`} />;
  if (step === undefined && runs === false)
    return (
      <EmptyState
        variant="panel"
        headline={`This grade of ${graded} is not running any more`}
        body="It ended while this page was away. Its scorecard says how."
        testid={`${whose}-grade-gone`}
        action={<Button onPress={onEnded}>Open its scorecard</Button>}
      />
    );
  const over = step !== undefined && step.ended !== null;
  const end = over && !opensItsScorecard(step) ? endWords(step) : undefined;
  return (
    <ReviewPage
      label={`Grading ${graded}`}
      data-testid={`${whose}-grade-running`}
      summary={over ? "It has ended." : "Stopping keeps what ran so far."}
      summaryReserve={["It has ended.", "Stopping keeps what ran so far."]}
      onCancel={onBack}
      cancel={{ children: "Leave it running" }}
      main={{
        children: "Stop",
        variant: "danger",
        isDisabled: stopping || over,
        onPress: () => {
          setStopping(true);
          setStopRefused(undefined);
          void commands
            .skillGradeStop(plane, grade)
            .then((answer) => {
              if (answer.status === "error") {
                setStopping(false);
                setStopRefused(answer.error);
              }
            })
            .catch((err: unknown) => {
              setStopping(false);
              setStopRefused(String(err));
            });
        },
      }}
    >
      <GradingHeading name={`Grading ${graded}`} />
      {stopRefused !== undefined && (
        <Notice
          cause={`${whose}-grade-stop:${grade}`}
          tone="trouble"
          label="Not stopped"
          onDismiss={() => setStopRefused(undefined)}
        >
          {stopRefused}
        </Notice>
      )}
      {end !== undefined && (
        <Notice
          cause={`${whose}-grade-ended:${grade}`}
          tone={end.tone}
          label="The grade ended"
          fixes={end.fixes}
        >
          {end.said}
        </Notice>
      )}
      <GradeProgress
        label={`How far the grade of ${graded} is`}
        ran={step?.ran ?? 0}
        of={step?.of ?? 0}
        tokens={step?.tokens ?? null}
        cap={step?.cap ?? 0}
        costUsd={step?.cost_usd ?? null}
        limitUsd={step?.limit_usd ?? null}
      />
    </ReviewPage>
  );
}

/**
 * **A skill's grade running** (S6): see the module. `onEnded` is told once it ends with nothing
 * more to say, so the studio opens its scorecard; an end with words to say stays here and says
 * them.
 */
export function GradeRunningPage({
  plane,
  name,
  level,
  grade,
  draft,
  onEnded,
  onBack,
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  grade: string;
  /** The draft of it the grade grades (#2780), by its id; `undefined` is the skill itself. */
  draft?: string;
  onEnded: () => void;
  /** Back to the Grades tab: the grade keeps going. */
  onBack: () => void;
}) {
  return (
    <RunningGradePage
      // Another grade is another page: nothing of this one's is drawn under its name. The level
      // as a value: an equal level reads nothing anew.
      key={JSON.stringify([plane, level, name, grade])}
      whose="skill"
      plane={plane}
      grade={grade}
      event={SKILL_GRADE}
      isOurs={(step) => step.skill === name && step.draft === (draft ?? null)}
      read={async () => {
        const answer =
          draft === undefined
            ? await commands.skillGrades(plane, level, name)
            : await commands.skillDraftGrade(plane, draft);
        if (answer.status !== "ok") return answer;
        const now = answer.data.running;
        return { status: "ok", data: now !== null && now.grade === grade ? now : null };
      }}
      of={() => (draft === undefined ? name : `the draft of ${name}`)}
      opensItsScorecard={(step) => step.ended !== null && step.said === null}
      // An end with words to say: something went wrong on the way (not recorded, chats that did
      // not start), so its scorecard may not be there to open.
      endWords={(step) => ({
        tone: "trouble",
        fixes: [
          { label: "Open its scorecard", onPress: onEnded },
          { label: "Back to its grades", onPress: onBack },
        ],
        said: endedSaid(step),
      })}
      onEnded={onEnded}
      onBack={onBack}
    />
  );
}
