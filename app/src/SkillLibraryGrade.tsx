// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  commands,
  SKILL_GRADES,
  type BatchSkillStep,
  type GradeEnded,
  type GradeEstimate,
  type SkillGradesPlan,
  type SkillGradesStep,
  type SkillSetMember,
} from "./bindings";
import { counted } from "./dispatches";
import { Notice } from "./Notice";
import { dollarsSaid as dollars, GradingHeading } from "./SkillGrade";
import { NotRead, ReadingRows } from "./SkillPanels";
import { useRead } from "./SkillRead";
import { memberKey, type Stays } from "./SkillSetReview";
import { Badge } from "./ui/badge/Badge";
import { Tile, Totals } from "./ui/card/Totals";
import { TextField } from "./ui/field/Field";
import { List, ListRow } from "./ui/list/List";
import { RowBelow, RowLabel, RowTrail } from "./ui/list/Row";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { ReviewPage } from "./ui/review-page/ReviewPage";

/**
 * **Grade, for the skills chosen in the Library's Select mode** (#5356, the operator's ruling of
 * 2026-11-05): each a grade of its own, as a press of Grade on its Grades tab starts one, on the
 * project's default profile and model, its trigger tests alone.
 *
 * - **The estimate comes first** ({@link GradeSetReview}), in the selection page's place: what
 *   each skill's last grade spent, worked out for this one and added, with the skills it has
 *   nothing to go on for named, each with why; the skills left out, those with no tests among
 *   them, named with why; and the limit the person sets, which nothing runs without.
 * - **A few run at a time** ({@link GradesRunningPage}): each skill with where it is, what the
 *   grades spent together against the limit, and Stop. Once the limit is reached none starts and
 *   those running stop, which keeps what they ran: every grade that ended is on its skill's Grades
 *   tab. The core decides it all (`grades::batch`); this page says it.
 */

/** Dollars as the window says them, a figure the core could not say as `null` being none. */
const dollarsSaid = (usd: number | null): string => dollars(usd ?? 0);

/** **The estimate in a sentence**: what is known, added, and how many have nothing to go on. */
export function estimateSaid(
  plan: Pick<SkillGradesPlan, "known_usd" | "known" | "unknown">,
): string {
  if (plan.known === 0)
    return "purlis has nothing to go on: none of these has a last grade that said what it spent.";
  const about = `About ${dollarsSaid(plan.known_usd)}`;
  if (plan.unknown.length === 0) return `${about} in all, from each skill's last grade.`;
  const graded = plan.known === 1 ? "the one graded before" : `the ${plan.known} graded before`;
  const others = plan.unknown.length === 1 ? "the other one" : `the other ${plan.unknown.length}`;
  return `${about} for ${graded}. purlis has nothing to go on for ${others}.`;
}

/** Why a skill's grade has nothing to go on: no grade before, or one whose chats said nothing. */
function noEstimateWhy(estimate: GradeEstimate | null): string {
  return estimate === null
    ? "It has not been graded here before."
    : "Its last grade did not say what it spent.";
}

/** A limit as typed, in dollars, or `undefined` where it is no amount above nothing. */
export function limitTyped(typed: string): number | undefined {
  const usd = Number(typed.trim().replace(/^\$/, ""));
  return typed.trim() !== "" && Number.isFinite(usd) && usd > 0 ? usd : undefined;
}

/** The limit the field starts at: the estimate rounded up to a dollar, where every skill has
 *  one, and empty otherwise, so purlis never guesses a figure it has nothing to go on for. */
function limitOffered(plan: SkillGradesPlan): string {
  return plan.unknown.length === 0 && plan.known > 0
    ? String(Math.max(1, Math.ceil(plan.known_usd ?? 0)))
    : "";
}

/** One name of a review, with what is said of it. */
function NamedLine({ name, said, testid }: { name: string; said: string; testid: string }) {
  return (
    <p className="honest" data-testid={testid}>
      <code>{name}</code> {said}
    </p>
  );
}

/**
 * **The review of Grade**, in the selection page's place: see the module. `members` are the
 * skills it can grade; `stays` those it leaves as they are (a built-in), with why. Cancel goes
 * back to the selection; Start tells the batch's id.
 */
export function GradeSetReview({
  plane,
  members,
  stays,
  onCancel,
  onStarted,
  onRunning,
}: {
  plane: string;
  members: readonly SkillSetMember[];
  stays: readonly Stays[];
  onCancel: () => void;
  onStarted: (batch: string) => void;
  /** Several skills of this project are being graded already: show them. */
  onRunning: () => void;
}) {
  const { read, again } = useRead(members.map(memberKey).join("\u0001"), () =>
    commands.skillGradesPlan(
      plane,
      members.map((one) => [one.level, one.name]),
    ),
  );
  const plan = read?.data;
  const [typed, setTyped] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string>();
  const shown = typed ?? (plan === undefined ? "" : limitOffered(plan));
  const limit = limitTyped(shown);
  const skills = plan?.skills ?? [];
  const title = `Grade ${counted(skills.length || members.length, "skill", "skills")}?`;
  const unready =
    plan === undefined
      ? "Reading what it would spend."
      : (plan.refused ??
        plan.running ??
        (skills.length === 0
          ? "None of these has tests to grade."
          : limit === undefined
            ? "Set a spending limit."
            : undefined));
  const start = () => {
    if (plan === undefined || limit === undefined) return;
    setBusy(true);
    setRefused(undefined);
    void commands
      .skillGradesStart(
        plane,
        skills.map((one) => ({ level: one.level, name: one.name, consent: one.consent })),
        limit,
      )
      .then((answer) => {
        setBusy(false);
        if (answer.status === "ok") onStarted(answer.data);
        else setRefused(answer.error);
      })
      .catch((err: unknown) => {
        setBusy(false);
        setRefused(String(err));
      });
  };
  const atMost = limit === undefined ? undefined : dollarsSaid(limit);
  return (
    <ReviewPage
      label={title}
      data-testid="grades-review"
      summary={
        atMost === undefined
          ? "Nothing runs until you set a limit and press Start grading."
          : `Nothing runs until you press Start grading. It stops at ${atMost}, keeping the grades done by then.`
      }
      onCancel={onCancel}
      main={{
        children: "Start grading",
        isDisabled: busy || unready !== undefined,
        tooltip: unready,
        onPress: start,
      }}
    >
      <GradingHeading
        name={title}
        description={`They run ${plan?.at_once ?? 3} at a time${plan?.profile == null ? "" : ` on ${plan.profile}`}, each a grade of its own, as Grade on its Grades tab runs one, with its trigger tests. Each grade ends on its skill's Grades tab.`}
      />
      {read?.refused !== undefined && (
        <NotRead
          what="what grading these would spend"
          why={read.refused}
          onAgain={again}
          testid="grades-not-read"
        />
      )}
      {plan === undefined && read?.refused === undefined && (
        <ReadingRows what="what grading these would spend" testid="grades-reading" />
      )}
      {plan?.running != null && (
        <Notice
          cause="skill-grades-running"
          tone="news"
          label="Grading already"
          fixes={[{ label: "Show them", onPress: onRunning }]}
        >
          {plan.running}
        </Notice>
      )}
      {plan?.refused != null && (
        <p className="honest" data-testid="grades-refused">
          No grade can run on {plan.profile ?? "this profile"}: {plan.refused}
        </p>
      )}
      {refused !== undefined && (
        <Notice
          cause="skill-grades-start"
          tone="trouble"
          label="Not started"
          onDismiss={() => setRefused(undefined)}
        >
          {refused}
        </Notice>
      )}
      {plan !== undefined && (
        <>
          <section aria-labelledby="grades-cost" className="skill-review">
            <PanelSectionTitle id="grades-cost">What it should spend</PanelSectionTitle>
            <p data-testid="grades-estimate">
              {estimateSaid(plan)} Each grade is held to its own limit too, so together they can
              spend at most {dollarsSaid(plan.most_usd)}.
            </p>
            {plan.unknown.length > 0 && (
              <div data-testid="grades-unknown">
                {skills
                  .filter((one) => plan.unknown.includes(one.name))
                  .map((one) => (
                    <NamedLine
                      key={one.name}
                      name={one.name}
                      said={noEstimateWhy(one.estimate)}
                      testid={`grades-unknown-${one.name}`}
                    />
                  ))}
              </div>
            )}
            <TextField
              label="Spending limit"
              help="In dollars, for all of them together. Once their chats have spent it, no more start and those running stop."
              inputMode="numeric"
              value={shown}
              onChange={setTyped}
              isDisabled={busy}
              validate={(value) =>
                value.trim() === "" || limitTyped(value) !== undefined
                  ? undefined
                  : "A limit is an amount above $0, such as 5."
              }
            />
          </section>
          <section aria-labelledby="grades-going" className="skill-review">
            <PanelSectionTitle id="grades-going">{`Will be graded (${skills.length})`}</PanelSectionTitle>
            {skills.map((one) => (
              <NamedLine
                key={one.name}
                name={one.name}
                said={`${counted(one.chats, "chat", "chats")}${
                  one.estimate?.cost_usd == null
                    ? ""
                    : `, about ${dollarsSaid(one.estimate.cost_usd)}`
                }`}
                testid={`grades-going-${one.name}`}
              />
            ))}
          </section>
          {(plan.left_out.length > 0 || stays.length > 0) && (
            <section aria-labelledby="grades-left-out" className="skill-review">
              <PanelSectionTitle id="grades-left-out">Left out</PanelSectionTitle>
              {plan.left_out.map((one) => (
                <NamedLine
                  key={one.name}
                  name={one.name}
                  said={one.why}
                  testid={`grades-left-out-${one.name}`}
                />
              ))}
              {stays.map((one) => (
                <NamedLine
                  key={one.name}
                  name={one.name}
                  said={one.why}
                  testid={`grades-left-out-${one.name}`}
                />
              ))}
            </section>
          )}
        </>
      )}
    </ReviewPage>
  );
}

/** How a grade of a batch that ended is said, in a few words. */
const ENDED_SAID: Record<GradeEnded, string> = {
  whole: "Graded",
  stopped: "Stopped",
  "chats-did-not-start": "Its chats did not start",
  "over-cap": "Stopped at its token cap",
  "over-limit": "Stopped at its own limit",
  "spend-not-reported": "Its chats did not say what they spent",
  failed: "A fault in purlis ended it",
};

/** **Where one skill of a batch is**, in a line. */
export function skillStepSaid(one: BatchSkillStep): string {
  switch (one.state.state) {
    case "waiting":
      return "Waiting";
    case "running":
      return `Running: ${one.ran} of ${counted(one.of, "chat", "chats")}`;
    case "not-run":
      return `Not graded: ${one.state.why}`;
    case "ended": {
      const ended = one.ended ?? "whole";
      const said =
        ended === "stopped" ? `Stopped after ${one.ran} of ${one.of} chats` : ENDED_SAID[ended];
      return one.cost_usd === null ? said : `${said}, ${dollarsSaid(one.cost_usd)}`;
    }
  }
}

/** **How a batch ended**, in a sentence. */
function batchEndedSaid(step: SkillGradesStep): string {
  const ended = step.skills.filter((one) => one.state.state === "ended").length;
  const kept = `${counted(ended, "grade", "grades")} ended and ${ended === 1 ? "is" : "are"} on ${ended === 1 ? "its skill's" : "their skills'"} Grades tab.`;
  switch (step.ended) {
    case "limit":
      return `Stopped at your limit: ${dollarsSaid(step.spent_usd)} of ${dollarsSaid(step.limit_usd)}. ${kept}`;
    case "stopped":
      return `You stopped it at ${dollarsSaid(step.spent_usd)}. ${kept}`;
    case "spend-not-reported":
      return `A chat did not say what it spent, so your limit could not hold, and it stopped at ${dollarsSaid(step.spent_usd)}. ${kept}`;
    default:
      return `Done: ${dollarsSaid(step.spent_usd)} of ${dollarsSaid(step.limit_usd)}. ${kept}`;
  }
}

/**
 * **The grades of a batch running**: see the module. Listened to first, then read, so an end
 * told before the listener was attached is never missed. `onOpen` opens a skill's Grades tab.
 */
export function GradesRunningPage({
  plane,
  batch,
  onOpen,
  onBack,
}: {
  plane: string;
  batch: string;
  onOpen: (one: BatchSkillStep) => void;
  /** Back to the Library: the grades keep going. */
  onBack: () => void;
}) {
  const [step, setStep] = useState<SkillGradesStep>();
  const [gone, setGone] = useState(false);
  const [unread, setUnread] = useState<string>();
  const [again, setAgain] = useState(0);
  const [stopping, setStopping] = useState(false);
  const [stopRefused, setStopRefused] = useState<string>();
  const latest = useRef({ plane, batch });
  useLayoutEffect(() => {
    latest.current = { plane, batch };
  });
  useEffect(() => {
    let live = true;
    let toldSince = false;
    const listening = listen<SkillGradesStep>(SKILL_GRADES, (told) => {
      if (!live || told.payload.plane !== plane || told.payload.batch !== batch) return;
      toldSince = true;
      setStep(told.payload);
    }).catch(() => undefined);
    void listening
      .then(() => commands.skillGradesRunning(latest.current.plane))
      .then((answer) => {
        if (!live) return;
        if (answer.status !== "ok") {
          setUnread(answer.error);
          return;
        }
        setUnread(undefined);
        const now = answer.data?.batch === batch ? answer.data : null;
        if (now !== null && !toldSince) setStep(now);
        if (now === null && !toldSince) setGone(true);
      })
      .catch((err: unknown) => {
        if (live) setUnread(String(err));
      });
    return () => {
      live = false;
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
  }, [plane, batch, again]);

  if (step === undefined && unread !== undefined)
    return (
      <NotRead
        what="the grades running"
        why={unread}
        onAgain={() => {
          setUnread(undefined);
          setAgain((n) => n + 1);
        }}
        testid="grades-not-read"
      />
    );
  if (step === undefined && !gone)
    return <ReadingRows what="the grades running" testid="grades-running-reading" />;
  const over = step === undefined || step.ended !== null;
  const ended = step?.skills.filter((one) => one.state.state === "ended").length ?? 0;
  return (
    <ReviewPage
      label="Grading several skills"
      data-testid="grades-running"
      summary={over ? "It has ended." : "Stopping keeps the grades done so far."}
      summaryReserve={["It has ended.", "Stopping keeps the grades done so far."]}
      onCancel={onBack}
      cancel={{ children: over ? "Back to the Library" : "Leave them running" }}
      main={{
        children: "Stop",
        variant: "danger",
        isDisabled: stopping || over,
        onPress: () => {
          setStopping(true);
          setStopRefused(undefined);
          void commands
            .skillGradesStop(plane, batch)
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
      <GradingHeading
        name={`Grading ${counted(step?.skills.length ?? 0, "skill", "skills")}`}
        description="You can leave this page: the grades keep going, and their chats never show as needing you. Closing the app ends them."
      />
      {stopRefused !== undefined && (
        <Notice
          cause={`skill-grades-stop:${batch}`}
          tone="trouble"
          label="Not stopped"
          onDismiss={() => setStopRefused(undefined)}
        >
          {stopRefused}
        </Notice>
      )}
      {step === undefined ? (
        <p className="honest" data-testid="grades-gone">
          These grades are not running any more. Each that ended is on its skill's Grades tab.
        </p>
      ) : (
        <>
          {step.ended !== null && (
            <p className="honest" data-testid="grades-ended">
              {batchEndedSaid(step)}
            </p>
          )}
          <Totals live aria-label="How far the grades are">
            <Tile
              label="Spent"
              value={dollarsSaid(step.spent_usd)}
              line={`of at most ${dollarsSaid(step.limit_usd)}`}
            />
            <Tile
              label="Grades"
              value={`${ended} of ${step.skills.length}`}
              said={`${ended} of ${counted(step.skills.length, "grade", "grades")} ended`}
            />
          </Totals>
          <List
            aria-label="Each skill"
            data-testid="grades-skills"
            onAction={(key) => {
              const one = step.skills.find((skill) => skill.name === String(key));
              if (one !== undefined) onOpen(one);
            }}
          >
            {step.skills.map((one) => (
              <ListRow
                key={one.name}
                id={one.name}
                textValue={`${one.name}, ${skillStepSaid(one)}`}
                data-testid={`grades-skill-${one.name}`}
              >
                <RowLabel>{one.name}</RowLabel>
                <RowTrail>
                  <Badge>
                    {one.state.state === "running"
                      ? "Running"
                      : one.state.state === "ended"
                        ? "Ended"
                        : one.state.state === "waiting"
                          ? "Waiting"
                          : "Not graded"}
                  </Badge>
                </RowTrail>
                <RowBelow clamp={1}>{skillStepSaid(one)}</RowBelow>
              </ListRow>
            ))}
          </List>
        </>
      )}
    </ReviewPage>
  );
}
