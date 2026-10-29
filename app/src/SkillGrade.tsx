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
 * - **A grade running** shows its chats done, what it has spent against its dollar limit and its
 *   tokens against its cap, with Stop fixed at the bottom. A grade's chats are drawn nowhere.
 */
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  commands,
  SKILL_GRADE,
  type ProfileRow,
  type SkillGradeAfterEdit,
  type SkillGradePlan,
  type SkillGradeStep,
  type SkillLevel,
} from "./bindings";
import { counted } from "./dispatches";
import { Notice } from "./Notice";
import { Button, ButtonRow } from "./ui/button/Button";
import { Tile, Totals } from "./ui/card/Totals";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { TextField } from "./ui/field/Field";
import { ProgressBar } from "./ui/meter/Meter";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { ViewPageHeader } from "./ui/panel/ViewFrame";
import { ReviewPage } from "./ui/review-page/ReviewPage";
import { Select } from "./ui/select/Select";

/** The project's default profile, in the picker. */
const DEFAULT = "";

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

/** How a grade that ended is said, before what its chats asked. */
function endSaid(step: SkillGradeStep): string {
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

/** What a grade should spend, from a grade before, in one sentence: never a figure that was not
 *  measured, and always which grade it is worked out from (#3273). A grade whose chats said
 *  nothing of what they spent was a grade still: its cost is not known. */
export function costSaid(plan: SkillGradePlan): string {
  const estimate = plan.estimate;
  if (estimate === null)
    return "This skill has not been graded here before, so purlis cannot say yet what a grade spends. Each chat is one turn, and ends once the skill starts.";
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
    return `Its last grade${ran}, ran no chat, so purlis cannot say yet what a grade spends. Each chat is one turn, and ends once the skill starts.`;
  if (estimate.tokens === null && estimate.cost_usd === null)
    return `Its last grade${ran}, did not say what its chats spent, so purlis cannot say what this one spends. Each chat is one turn, and ends once the skill starts.`;
  const parts: string[] = [];
  if (estimate.tokens !== null) parts.push(`about ${tokensSaid(estimate.tokens)}`);
  if (estimate.cost_usd !== null) parts.push(`about $${estimate.cost_usd.toFixed(2)}`);
  return `From its grade${ran}, of ${counted(estimate.chats_then, "chat", "chats")}, this one should spend ${parts.join(", ")}.`;
}

/** The dollar limit a grade is held to, and the price it was worked out at (#3469): the model's,
 *  or the dearest known where the model or its price is not. */
export function limitSaid(plan: SkillGradePlan): string {
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

/** Dollars as the window says them: `$1.92`. */
export function dollarsSaid(usd: number): string {
  return `$${usd.toFixed(2)}`;
}

/** What a plan's arms are, counted: with the skill, without it, and its previous version. */
function armsOf(plan: SkillGradePlan): number {
  return plan.previous === null ? 2 : 3;
}

/** **What changed in a plan**, in a clause: what the new one runs and may spend. */
export function planChangedSaid(now: SkillGradePlan): string {
  const on = [now.profile, now.model].filter((one) => one !== null).join(", ");
  return `it now runs ${counted(now.chats, "chat", "chats")}${on === "" ? "" : ` on ${on}`}, and may spend at most ${dollarsSaid(now.cost_cap_usd ?? 0)}`;
}

/** Why the plan's main action can't be pressed now, or nothing where it can. */
function startRefused(
  name: string,
  plan: SkillGradePlan,
  newer: boolean,
  afterEdit: boolean,
): string | undefined {
  if (newer) return "The plan changed: read the new one first.";
  if (plan.tests.length === 0) return `${name} has no trigger tests to run yet.`;
  if (plan.refused !== null) return `No grade can run on ${plan.profile ?? "this profile"}.`;
  if (!afterEdit && plan.running !== null) return `A grade of ${name} is running already.`;
  if (plan.consent === null) return "purlis could not say what this grade would run.";
  return undefined;
}

/** A read that failed, in place, with Try again (S11). */
function NotRead({ what, why, onAgain }: { what: string; why: string; onAgain: () => void }) {
  return (
    <EmptyState
      variant="panel"
      headline={`purlis could not read ${what}`}
      body={why}
      testid="skill-grading-not-read"
      action={<Button onPress={onAgain}>Try again</Button>}
    />
  );
}

/** A read still on its way (S11). */
export function Reading({ what }: { what: string }) {
  return (
    <p className="pending" aria-busy="true" data-testid="skill-grading-reading">
      {`Reading ${what}…`}
    </p>
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
  /** A grade started: its id, and the plan it was started from. */
  onStarted: (grade: string, plan: SkillGradePlan) => void;
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
  const [again, setAgain] = useState(0);
  const picked = profile === DEFAULT ? null : profile;
  const pickedModel = model === DEFAULT ? null : model;
  // The level as a value: a page drawn again with an equal level reads nothing anew.
  const levelKey = JSON.stringify(level);
  const drawn = useRef<SkillGradePlan>(undefined);
  useEffect(() => {
    drawn.current = plan;
  });

  /** Reads the plan: drawn, or (`check`) held as newer where it consents to something else. */
  const read = (on: string | null, onModel: string | null, check = false) =>
    void commands
      .skillGradePlan(plane, level, name, on, onModel)
      .then((answer) => {
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
        if (!check) setRefused(String(err));
      });

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
      />
    );
  if (plan === undefined) return <Reading what={`what a grade of ${name} runs`} />;

  const consent = plan.consent ?? "";
  const start = () =>
    void commands
      .skillGradeStart(plane, level, name, picked, pickedModel, consent)
      .then((answer) => {
        if (answer.status === "ok") onStarted(answer.data, plan);
        else {
          setSaid(answer.error);
          // The plan is read again after any refusal: one that changed since it was shown is
          // held, to be read and pressed again.
          read(picked, pickedModel, true);
        }
      });
  const turnOn = () =>
    void commands
      .skillGradeAfterEditOn(plane, level, name, picked, pickedModel, consent)
      .then((answer) => {
        if (answer.status === "ok") onAfterEdit(answer.data);
        else {
          setSaid(answer.error);
          read(picked, pickedModel, true);
        }
      });
  const why = startRefused(name, plan, newer !== undefined, afterEdit);
  const limit = dollarsSaid(plan.cost_cap_usd ?? 0);
  const whose = plan.profile ?? "the profile";
  const nothing = plan.tests.length === 0;
  return (
    <ReviewPage
      label={afterEdit ? `Grade ${name} after each edit` : `Grade ${name}`}
      data-testid="skill-grade-plan"
      summary={
        afterEdit
          ? `Nothing runs now. Each edit you save then runs up to ${counted(plan.chats, "chat", "chats")}, at most ${limit} each time, of ${whose}'s account.`
          : `Nothing runs until you press Start. It spends at most ${limit} of ${whose}'s account.`
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
      <ViewPageHeader
        name={afterEdit ? `Grade ${name} after each edit` : `Grade ${name}`}
        description={`A grade runs ${name}'s trigger tests in chats with it and without it, and shows what it may spend before anything runs.`}
      />
      {newer !== undefined && (
        <Notice
          cause={`skill-grade-plan-changed:${name}`}
          tone="attention"
          label="The plan changed"
          fixes={[
            {
              label: "Show the new plan",
              onPress: () => {
                setPlan(newer);
                setNewer(undefined);
                setSaid(undefined);
              },
            },
          ]}
        >
          <span data-testid="skill-grade-plan-changed">
            {`The plan changed since you opened it: ${planChangedSaid(newer)}. Read it again before you start.`}
          </span>
        </Notice>
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
            value={String(plan.chats)}
            line={`${counted(plan.tests.length, "test", "tests")} × ${plan.runs} × ${armsOf(plan)} arms`}
          />
          <Tile label="At most" value={limit} line={`or ${tokensSaid(plan.token_cap ?? 0)}`} />
          <Tile
            label="Estimate"
            value={
              plan.estimate?.cost_usd != null
                ? dollarsSaid(plan.estimate.cost_usd)
                : plan.estimate?.tokens != null
                  ? tokensSaid(plan.estimate.tokens)
                  : "Not known"
            }
            line={plan.estimate === null ? "not graded here before" : "from its last grade"}
          />
          <Tile
            label="Runs on"
            value={plan.model ?? "Its own model"}
            line={plan.profile ?? "The project's default"}
          />
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
              {counted(plan.runs, "time", "times")} without it
              {plan.previous === null
                ? ""
                : `, and ${counted(plan.runs, "time", "times")} with its previous version, ${plan.previous.commit} (${plan.previous.subject}), which it is compared with`}
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
        {plan.outcome_evals > 0 && (
          <p className="honest">
            {counted(plan.outcome_evals, "outcome eval waits", "outcome evals wait")} for a later
            version of purlis: this grade runs the trigger tests alone.
          </p>
        )}
        {plan.outcomes_refused !== null && <p className="honest">{plan.outcomes_refused}</p>}
      </section>
      {!nothing && (
        <>
          <section aria-labelledby="skill-grade-where">
            <PanelSectionTitle id="skill-grade-where">Where it runs</PanelSectionTitle>
            <ButtonRow>
              <Select
                size="sm"
                aria-label={`Grade ${name} on`}
                value={profile}
                options={[
                  { value: DEFAULT, label: "The project's default" },
                  ...profiles.map((one) => ({ value: one.name, label: one.name })),
                ]}
                onValueChange={(to) => {
                  // Another profile may be another program, whose models are its own.
                  setProfile(to);
                  setModel(DEFAULT);
                  read(to === DEFAULT ? null : to, null);
                }}
              />
              {plan.models.length > 0 && (
                <Select
                  size="sm"
                  aria-label={`Model ${name}'s grade runs on`}
                  value={model}
                  options={[
                    { value: DEFAULT, label: "The profile's own model" },
                    ...plan.models.map((one) => ({ value: one, label: one })),
                  ]}
                  onValueChange={(to) => {
                    setModel(to);
                    read(picked, to === DEFAULT ? null : to);
                  }}
                />
              )}
            </ButtonRow>
          </section>
          <section aria-labelledby="skill-grade-spend">
            <PanelSectionTitle id="skill-grade-spend">What it may spend</PanelSectionTitle>
            {afterEdit && (
              <p className="honest" data-testid="skill-grade-after-edit-offer">
                Each edit you save then runs this grade again, up to{" "}
                {counted(plan.chats, "chat", "chats")} and at most {limitSaid(plan)}, on the profile
                and model you pick, paid by its account. If an edit makes a grade run more chats or
                spend more, it starts nothing and asks you again.
              </p>
            )}
            <TokenCap
              key={plan.token_cap ?? 0}
              cap={plan.token_cap ?? 0}
              onCap={(tokens) =>
                void commands
                  .skillGradeSetCap(plane, level, name, picked, pickedModel, tokens)
                  .then((answer) => {
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

/** The token cap a grade is held to, set on this machine before anything runs. */
function TokenCap({ cap, onCap }: { cap: number; onCap: (tokens: number) => void }) {
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

/**
 * **A grade running** (S6): see the module. `onEnded` is told once it ends with nothing more to
 * say, so the studio opens its scorecard; an end with words to say stays here and says them.
 */
export function GradeRunningPage({
  plane,
  name,
  level,
  grade,
  started,
  onEnded,
  onBack,
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  grade: string;
  /** What the plan it was started from said, where this page opened from Start: the chats and
   *  the dollar limit until its first step says them (the core's first is told before then). */
  started?: { chats?: number; limitUsd?: number };
  onEnded: () => void;
  /** Back to the Grades tab: the grade keeps going. */
  onBack: () => void;
}) {
  const [step, setStep] = useState<SkillGradeStep>();
  const [plan, setPlan] = useState<SkillGradePlan>();
  const [stopping, setStopping] = useState(false);
  const levelKey = JSON.stringify(level);
  const ended = useRef(onEnded);
  useEffect(() => {
    ended.current = onEnded;
  });

  const [stopRefused, setStopRefused] = useState<string>();
  // Listened to first, then the app's word on whether it runs: an end told before the listener
  // was attached is never missed, as the plan read after it says the grade runs no more.
  useEffect(() => {
    let live = true;
    const listening = listen<SkillGradeStep>(SKILL_GRADE, (event) => {
      const now = event.payload;
      if (now.plane !== plane || now.skill !== name || now.grade !== grade) return;
      setStep(now);
      if (now.ended !== null && now.said === null) ended.current();
    }).catch(() => undefined);
    void listening
      .then(() => commands.skillGradePlan(plane, level, name, null, null))
      .then((answer) => {
        if (live && answer.status === "ok") setPlan(answer.data);
      })
      .catch(() => undefined);
    return () => {
      live = false;
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
    // The level as a value: an equal level reads nothing anew.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plane, levelKey, name, grade]);

  if (step === undefined && plan === undefined) return <Reading what={`the grade of ${name}`} />;
  if (step === undefined && plan !== undefined && plan.running !== grade)
    return (
      <EmptyState
        variant="panel"
        headline={`This grade of ${name} is not running any more`}
        body="It ended while this page was away. Its scorecard says how."
        testid="skill-grade-gone"
        action={<Button onPress={onEnded}>Open its scorecard</Button>}
      />
    );
  const ran = step?.ran ?? 0;
  const of = step?.of ?? started?.chats ?? plan?.chats ?? 0;
  const cap = step?.cap ?? plan?.token_cap ?? 0;
  const limit = step?.limit_usd ?? started?.limitUsd ?? plan?.cost_cap_usd ?? null;
  const over = step !== undefined && step.ended !== null;
  return (
    <ReviewPage
      label={`Grading ${name}`}
      data-testid="skill-grade-running"
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
      <ViewPageHeader name={`Grading ${name}`} />
      {stopRefused !== undefined && (
        <Notice
          cause={`skill-grade-stop:${grade}`}
          tone="trouble"
          label="Not stopped"
          onDismiss={() => setStopRefused(undefined)}
        >
          {stopRefused}
        </Notice>
      )}
      {over && step.said !== null && (
        // An end with words to say: something went wrong on the way (not recorded, chats that
        // did not start), so its scorecard may not be there to open.
        <Notice
          cause={`skill-grade-ended:${grade}`}
          tone="trouble"
          label="The grade ended"
          fixes={[
            { label: "Open its scorecard", onPress: onEnded },
            { label: "Back to its grades", onPress: onBack },
          ]}
        >
          {endedSaid(step)}
        </Notice>
      )}
      <Totals live aria-label={`How far the grade of ${name} is`}>
        <Tile
          label="Chats"
          value={`${ran} of ${of}`}
          said={`${ran} of ${counted(of, "chat", "chats")} done`}
        />
        <Tile
          label="Spent"
          value={step?.cost_usd != null ? dollarsSaid(step.cost_usd) : "Not said yet"}
          line={limit === null ? undefined : `of at most ${dollarsSaid(limit)}`}
        />
        <Tile
          label="Tokens"
          value={step?.tokens != null ? Math.round(step.tokens).toLocaleString("en-US") : "0"}
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
    </ReviewPage>
  );
}
