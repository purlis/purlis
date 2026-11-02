// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  commands,
  ROUTER_GRADE,
  SKILL_GRADE,
  type ProfileRow,
  type RouterGradePlan,
  type RouterGradeStep,
  type SkillGradeStep,
} from "./bindings";
import { counted } from "./dispatches";
import {
  against,
  KeptChatShown,
  keptChats,
  OUTCOME_SAID,
  pointsSaid,
  TriggerTestsTable,
} from "./GradeScorecard";
import { Notice } from "./Notice";
import {
  costSaid,
  DEFAULT,
  dollarsSaid,
  endSaid,
  EstimateTile,
  GradeOn,
  GradingHeading,
  limitSaid,
  PlanChanged,
  RunningGradePage,
  RunsOnTile,
  TokenCap,
  tokensSaid,
} from "./SkillGrade";
import { NotRead, ReadingRows, useRead } from "./SkillPanels";
import { Tile, Totals } from "./ui/card/Totals";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { ReviewPage } from "./ui/review-page/ReviewPage";

/** Where a router was chosen, as its grades are read: the studio's workspace and persona. */
type RouterScope = {
  plane: string;
  router: string;
  workspace: string | null;
  persona: string | null;
};

/** Whether `step` is of `router` as this scope chose it. */
function isOurs(
  step: RouterGradeStep | null,
  router: string,
  workspace: string | null,
  persona: string | null,
): boolean {
  return (
    step !== null &&
    step.router === router &&
    step.workspace === workspace &&
    step.persona === persona
  );
}

/**
 * **The router's grade running in this scope, told as it goes** (#4222): what Routing says of it,
 * with the way to its page, from wherever the person left it. `onEnded` is told at the end of any
 * router's grade in the project, so the studio reads the router's line again.
 */
export function useRouterGradeRunning(
  plane: string,
  workspace: string | null,
  persona: string | null,
  onEnded: () => void,
): RouterGradeStep | undefined {
  const [step, setStep] = useState<RouterGradeStep>();
  const told = useRef(onEnded);
  useEffect(() => {
    told.current = onEnded;
  });
  useEffect(() => {
    const listening = listen<RouterGradeStep>(ROUTER_GRADE, (event) => {
      const now = event.payload;
      if (now.plane !== plane) return;
      if (now.ended !== null) {
        setStep(undefined);
        told.current();
      } else if (now.workspace === workspace && now.persona === persona) setStep(now);
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane, workspace, persona]);
  return step;
}

/** Why the plan's Start can't be pressed now, or nothing where it can. */
function startRefused(router: string, plan: RouterGradePlan, newer: boolean): string | undefined {
  if (newer) return "The plan changed: read the new one first.";
  if (plan.chats === 0) return "No skill of this library has trigger tests to run yet.";
  if (plan.refused !== null) return `No grade can run on ${plan.profile ?? "this profile"}.`;
  if (plan.running !== null) return "A grade is running in this project already.";
  if (plan.consent === null) return `purlis could not say what a grade of ${router} would run.`;
  return undefined;
}

/**
 * **Grade the router, where it is chosen: its plan, a review page on Routing** (SK-15e, #3233;
 * #4222; ADR 0102 §8), as a skill's is (S6, `GradePlanPage`): its totals first (the chats, the
 * most it may spend, the estimate, what it runs on), then what it runs (each of the library's
 * trigger tests with the router in place and with the built-in router, the times it runs), where
 * it runs and what it may spend, with Start fixed at the foot saying what it may spend of whose
 * account. Nothing runs until Start is pressed, and Start consents to exactly what the plan
 * shows. **A plan that changed under the person** (Start refused on it, or the project's skills
 * changed) is never started: the page says so in the attention tone, and Start waits until they
 * read the new one.
 */
export function RouterGradePlanPage({
  plane,
  router,
  workspace,
  persona,
  changed,
  onStarted,
  onRunning,
  onCancel,
}: RouterScope & {
  /** Bumped when the project's skills change: the plan is read again, and held if it changed. */
  changed: number;
  /** A grade started: its id. */
  onStarted: (grade: string) => void;
  /** A grade of this router runs already: show it. */
  onRunning: (grade: string) => void;
  onCancel: () => void;
}) {
  const [plan, setPlan] = useState<RouterGradePlan>();
  // **A plan read since this one was drawn that consents to something else**: held, never drawn
  // under Start, until the person reads it.
  const [newer, setNewer] = useState<RouterGradePlan>();
  const [refused, setRefused] = useState<string>();
  const [said, setSaid] = useState<string>();
  const [profiles, setProfiles] = useState<readonly ProfileRow[]>([]);
  // Where its chats run (#3618): the project's default profile and the profile's own model
  // until the person picks others.
  const [profile, setProfile] = useState(DEFAULT);
  const [model, setModel] = useState(DEFAULT);
  // The grade running in the project read as this router's: what Show it opens.
  const [found, setFound] = useState<string>();
  const [again, setAgain] = useState(0);
  const picked = profile === DEFAULT ? null : profile;
  const pickedModel = model === DEFAULT ? null : model;
  const drawn = useRef<RouterGradePlan>(undefined);
  useEffect(() => {
    drawn.current = plan;
  });

  // The latest read asked for: an answer to an older one, landing after it, is never drawn, so
  // the plan shown is always of the profile and model picked.
  const asked = useRef(0);
  const settling = useRef(false);
  /** Reads the plan: drawn, or (`check`) held as newer where it consents to something else. */
  const read = (on: string | null, onModel: string | null, check = false) => {
    // A plain read under way answers with what is there now, so a check waits on nothing: it
    // never supersedes the read, nor mistakes the person's own pick for a change under them.
    if (check && settling.current) return;
    if (!check) settling.current = true;
    const ask = ++asked.current;
    const settled = () => {
      if (!check && ask === asked.current) settling.current = false;
    };
    void commands
      .routerGradePlan(plane, workspace, persona, on, onModel)
      .then((answer) => {
        if (ask !== asked.current) return;
        settled();
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
        settled();
        if (ask === asked.current && !check) setRefused(String(err));
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
  }, [plane, workspace, persona, again]);

  // A change to the project's skills may change what a grade runs: read again, and held.
  const first = useRef(changed);
  useEffect(() => {
    if (changed === first.current) return;
    read(picked, pickedModel, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [changed]);

  // A skill's grade that ends frees the project: read again, so Start is not held on it. A
  // router's end reaches `changed`, as the studio is told it.
  const readNow = useRef<() => void>(() => undefined);
  useEffect(() => {
    readNow.current = () => read(picked, pickedModel, true);
  });
  useEffect(() => {
    const listening = listen<SkillGradeStep>(SKILL_GRADE, (event) => {
      if (event.payload.plane === plane && event.payload.ended !== null) readNow.current();
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane]);

  // Whether the grade running in the project is this scope's of this router, so Show it can open
  // it: a scorecard is read by scope, and another scope's would not be found from here.
  const running = plan?.running ?? null;
  // Only the grade running now: one found before it is no grade to show.
  const ours = found !== undefined && found === running ? found : undefined;
  useEffect(() => {
    if (running === null) return;
    let live = true;
    void commands
      .routerGradeRunning(plane, running)
      .then((answer) => {
        if (live && answer.status === "ok" && isOurs(answer.data, router, workspace, persona))
          setFound(running);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [plane, router, workspace, persona, running]);

  if (refused !== undefined)
    return (
      <NotRead
        what={`what a grade of the router ${router} runs`}
        why={refused}
        onAgain={() => {
          setRefused(undefined);
          setAgain((n) => n + 1);
        }}
        testid="router-grade-refused"
      />
    );
  if (plan === undefined)
    return (
      <ReadingRows
        what={`what a grade of the router ${router} runs`}
        testid="router-grade-reading"
      />
    );

  const start = () =>
    void commands
      .routerGradeStart(plane, workspace, persona, picked, pickedModel, plan.consent ?? "")
      .then((answer) => {
        if (answer.status === "ok") onStarted(answer.data);
        else {
          setSaid(answer.error);
          // The plan is read again after any refusal: one that changed since it was shown is
          // held, to be read and pressed again.
          read(picked, pickedModel, true);
        }
      });
  const why = startRefused(router, plan, newer !== undefined);
  const limit = dollarsSaid(plan.cost_cap_usd ?? 0);
  const whose = plan.profile ?? "the profile";
  const nothing = plan.chats === 0;
  const tests = plan.skills.reduce((all, one) => all + one.tests, 0);
  return (
    <ReviewPage
      label={`Grade the router ${router}`}
      data-testid="router-grade-plan"
      summary={`Nothing runs until you press Start. It spends at most ${limit} of ${whose}'s account.`}
      onCancel={onCancel}
      main={{
        children: `Start grading · ${counted(plan.chats, "chat", "chats")}`,
        onPress: start,
        isDisabled: why !== undefined,
        tooltip: why,
        // It spends the person's money: pressed, never found by Return.
        byReturn: false,
      }}
    >
      <GradingHeading
        name={`Grade the router ${router}`}
        description="A grade runs the library's trigger tests in chats with this router in place and with the built-in router, and shows what it may spend before anything runs."
      />
      {newer !== undefined && (
        <PlanChanged
          cause={`router-grade-plan-changed:${router}`}
          newer={newer}
          testid="router-grade-plan-changed"
          onShow={() => {
            setPlan(newer);
            setNewer(undefined);
            setSaid(undefined);
          }}
        />
      )}
      {said !== undefined && newer === undefined && (
        <Notice
          cause="router-grade"
          tone="trouble"
          label="Not graded"
          onDismiss={() => setSaid(undefined)}
        >
          {said}
        </Notice>
      )}
      {plan.running !== null &&
        (ours === undefined ? (
          // Another's grade, a skill's: its page is under that skill, so none is offered here.
          <p className="honest" role="note" data-testid="router-grade-another-running">
            A grade is running in this project: one runs at a time.
          </p>
        ) : (
          <Notice
            cause={`router-grade-running:${router}`}
            tone="news"
            label="A grade is running"
            fixes={[{ label: "Show it", onPress: () => onRunning(ours) }]}
          >
            {`A grade of the router ${router} is running: one runs at a time.`}
          </Notice>
        ))}
      {!nothing && (
        <Totals aria-label={`What a grade of the router ${router} runs and spends`}>
          <Tile
            label="Chats"
            value={String(plan.chats)}
            line={`${counted(tests, "test", "tests")} × ${plan.runs} × 2 arms`}
          />
          <Tile label="At most" value={limit} line={`or ${tokensSaid(plan.token_cap ?? 0)}`} />
          <EstimateTile estimate={plan.estimate} />
          <RunsOnTile plan={plan} />
        </Totals>
      )}
      {plan.refused !== null && (
        <p className="honest" data-testid="router-grade-cannot-run">
          No grade can run on {plan.profile ?? "this profile"}: {plan.refused}
        </p>
      )}
      <section aria-labelledby="router-grade-runs">
        <PanelSectionTitle id="router-grade-runs">What it runs</PanelSectionTitle>
        {nothing ? (
          <EmptyState
            variant="panel"
            headline="No trigger tests in this library yet"
            body="A router is graded on the trigger tests of the skills it may hide: the queries a skill should start on and the near misses it should leave alone, kept with each skill. Add some, and Grade runs them."
            testid="router-grade-none"
          />
        ) : (
          <>
            <p data-testid="router-grade-cost">
              Grading runs {counted(plan.chats, "chat", "chats")}, one turn each: each of the{" "}
              {counted(tests, "trigger test", "trigger tests")} of{" "}
              {counted(plan.skills.length, "skill", "skills")} {counted(plan.runs, "time", "times")}{" "}
              with the router {plan.router} in place and {counted(plan.runs, "time", "times")} with
              the built-in router, each chat handed the library of{" "}
              {counted(plan.library, "skill", "skills")}, up to{" "}
              {counted(plan.chats, "chat", "chats")} × {plan.chat_minutes} min.{" "}
              {routerCostSaid(plan)} It stops once its chats have spent{" "}
              {tokensSaid(plan.token_cap ?? 0)}
              {plan.cap_set_here ? ", the cap you set on this machine" : ", the default cap"}, or
              cost {limitSaid(plan)}. The chats run on{" "}
              {plan.profile ?? "the project's default profile"}
              {plan.model === null ? "" : `, on ${plan.model}`}
              {plan.sandbox === null ? "" : `, in the project's ${plan.sandbox}`}, with none of your
              own Allows and no tools but purlis's that only read, and its account pays for them as
              for any chat. It is recorded{" "}
              {plan.commit === null
                ? "on a library with changes not committed"
                : `on the library at ${plan.commit.slice(0, 12)}`}
              .
            </p>
            {plan.pointer !== null && (
              <p data-testid="router-grade-pointer">
                {plan.pointer
                  ? `The router ${plan.router} reads prompts here: in its arm, each trigger test's prompt is handed to it, and its pointer reaches that prompt as in a real chat.`
                  : `The router ${plan.router} can read prompts, but that is off on Routing, so it is graded on its answer at each chat's start alone.`}
              </p>
            )}
            <ul aria-label="Skills the router is graded on">
              {plan.skills.map((one) => (
                <li key={one.skill} data-testid="router-grade-skill">
                  <code>{one.skill}</code> · {counted(one.tests, "trigger test", "trigger tests")}
                  {one.refused !== null && <span className="honest"> · {one.refused}</span>}
                  {one.left_out.length > 0 && (
                    <span className="honest">
                      {" "}
                      · left out:{" "}
                      {one.left_out.map((left) => `test ${left.at + 1}: ${left.why}`).join(" ")}
                    </span>
                  )}
                </li>
              ))}
            </ul>
          </>
        )}
      </section>
      {!nothing && (
        <>
          <section aria-labelledby="router-grade-where">
            <PanelSectionTitle id="router-grade-where">Where it runs</PanelSectionTitle>
            <GradeOn
              profileLabel={`Grade the router ${router} on`}
              modelLabel={`Model the router ${router}'s grade runs on`}
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
          <section aria-labelledby="router-grade-spend">
            <PanelSectionTitle id="router-grade-spend">What it may spend</PanelSectionTitle>
            <TokenCap
              key={plan.token_cap ?? 0}
              cap={plan.token_cap ?? 0}
              onCap={(tokens) => {
                const ask = ++asked.current;
                void commands
                  .routerGradeSetCap(plane, workspace, persona, picked, pickedModel, tokens)
                  .then((answer) => {
                    if (ask !== asked.current) return;
                    if (answer.status === "ok") {
                      setPlan(answer.data);
                      setNewer(undefined);
                      setSaid(undefined);
                    } else setSaid(answer.error);
                  });
              }}
            />
          </section>
        </>
      )}
    </ReviewPage>
  );
}

/** Whether an end says all it has to on its scorecard, which this scope can read. */
function opensItsScorecard(step: RouterGradeStep, chosen: string | null): boolean {
  return step.ended !== null && step.said === null && step.recorded && step.router === chosen;
}

/**
 * **A router's grade running, a page on Routing** (#4222), as a skill's is (S6,
 * `RunningGradePage`): its chats done, what it has spent against its dollar limit and its tokens
 * against its cap, with Stop fixed at the foot. A grade's chats are drawn nowhere. `onEnded` is
 * told once it ends with nothing more to say, so Routing opens its scorecard (#3617); an end with
 * words to say stays here and says them, as does one of a router no longer chosen here, whose
 * grades are not read from this scope. It stays drawn whichever router is chosen meanwhile: `router`
 * is the one chosen now, and the grade's own name comes from its step, so Stop is never lost.
 */
export function RouterGradeRunningPage({
  plane,
  router,
  grade,
  onEnded,
  onBack,
}: {
  plane: string;
  /** The router chosen in this scope now, where an extension's is. */
  router: string | null;
  grade: string;
  onEnded: () => void;
  /** Back to Routing: the grade keeps going. */
  onBack: () => void;
}) {
  return (
    <RunningGradePage
      // Another grade is another page: nothing of this one's is drawn under its name.
      key={`${plane}:${grade}`}
      whose="router"
      plane={plane}
      grade={grade}
      event={ROUTER_GRADE}
      read={() => commands.routerGradeRunning(plane, grade)}
      of={(step) => `the router ${step?.router ?? router ?? "chosen here before"}`}
      opensItsScorecard={(step) => opensItsScorecard(step, router)}
      // An end with words to say: something went wrong on the way, or the router it graded is no
      // longer chosen here, so its scorecard is offered only where it can open.
      endWords={(step) => ({
        tone: step.said === null ? "news" : "trouble",
        fixes:
          step.recorded && step.router === router
            ? [
                { label: "Open its scorecard", onPress: onEnded },
                { label: "Back to the list", onPress: onBack },
              ]
            : [{ label: "Back to the list", onPress: onBack }],
        said: (
          <span data-testid="router-grade-ended">
            {step.router === router
              ? endSaid(step)
              : `${endSaid(step)} It graded the router ${step.router}, no longer chosen here: choose it again to read its scorecard.`}
          </span>
        ),
      })}
      onEnded={onEnded}
      onBack={onBack}
      back="Back to the list"
    />
  );
}

/** **What a router's grade should spend, from its last grade** (#3618): a skill's plan's words
 *  (`costSaid`), and, where that grade ran on another library, that this one may spend otherwise. */
export function routerCostSaid(plan: RouterGradePlan): string {
  const said = costSaid(plan, "router");
  const { estimate } = plan;
  // Said only of a figure the plan gives.
  const figure =
    estimate !== null &&
    estimate.chats_then > 0 &&
    (estimate.tokens !== null || estimate.cost_usd !== null);
  return figure && plan.estimate_same_library === false
    ? `${said} That grade may have run on another library (its skills differ, or hold changes not committed), and each chat is handed the whole library, so this one may spend otherwise.`
    : said;
}

/**
 * **A router's grade's scorecard, a page on Routing** (#3617), as a skill's is (S6): how it ended,
 * its totals (each arm's pass rate over every skill, and what it spent), then each skill's trigger
 * tests in both arms, each chat one press away where this machine kept it.
 */
export function RouterGradeScorecardPage({
  plane,
  router,
  workspace,
  persona,
  grade,
  onChat,
}: RouterScope & { grade: string; onChat: (chat: string) => void }) {
  const [again, setAgain] = useState(0);
  // Its end, put away once read: Grade the router is on the Routing page's list.
  const [endPutAway, setEndPutAway] = useState(false);
  const card = useRead(
    () => commands.routerGradeScorecard(plane, workspace, persona, grade),
    [plane, workspace, persona, grade, again],
  );
  if (card.refused !== undefined)
    return (
      <NotRead
        what={`this grade of the router ${router}`}
        why={card.refused}
        onAgain={() => setAgain((n) => n + 1)}
        testid="router-grade-scorecard-refused"
      />
    );
  if (card.data === undefined)
    return <ReadingRows what="the grade's scorecard" testid="router-grade-scorecard-reading" />;
  const it = card.data;
  const end = OUTCOME_SAID[it.outcome];
  const spent = it.spent.cost_usd === null ? "" : ` for ${dollarsSaid(it.spent.cost_usd)}`;
  return (
    <section
      className="skill-studio-page"
      data-testid="router-grade-scorecard"
      aria-label={`Scorecard of the router ${router}`}
    >
      <GradingHeading name={`Grade of ${router} · ${it.graded}`} />
      {!endPutAway && (
        <Notice
          cause={`router-grade-end:${grade}`}
          tone={end.tone}
          label={end.said}
          onDismiss={() => setEndPutAway(true)}
        >
          <span data-testid="router-grade-scorecard-about">
            {`${end.said} ${it.graded}, on ${it.library}, on ${it.ran_on}${spent}. ${it.ended}.`}
          </span>
        </Notice>
      )}
      {it.asked > 0 && (
        <p className="honest" data-testid="router-grade-scorecard-asked">
          {counted(it.asked, "chat", "chats")} asked you something. A grade's chat waits on nobody,
          so purlis ended each at its ask and counts none of them: open its chat below to read what
          it asked.
        </p>
      )}
      {it.outcome !== "interrupted" && (
        <Totals aria-label={`Totals of the grade of ${router}`}>
          <Tile label="With the router" value={pointsSaid(it.arms.router)} line="of tests passed" />
          <Tile
            label="With the built-in router"
            value={pointsSaid(it.arms.built_in)}
            line={against(it.arms.router, it.arms.built_in)}
          />
          <Tile
            label="Spent"
            value={it.spent.cost_usd === null ? "Not said" : dollarsSaid(it.spent.cost_usd)}
            line={[
              it.spent.limit_usd === null ? null : `of ${dollarsSaid(it.spent.limit_usd)}`,
              counted(it.spent.chats, "chat", "chats"),
            ]
              .filter((one) => one !== null)
              .join(" · ")}
          />
        </Totals>
      )}
      {it.skills.length === 0 ? (
        <EmptyState
          variant="panel"
          headline="No trigger test was scored"
          body="This grade ended before any skill's trigger tests were scored. Grade the router again to score them."
          testid="router-grade-scorecard-none"
        />
      ) : (
        it.skills.map((one, at) => (
          <section key={one.skill} aria-labelledby={`router-grade-skill-${at}`}>
            <PanelSectionTitle id={`router-grade-skill-${at}`}>{one.skill}</PanelSectionTitle>
            <TriggerTestsTable
              tests={one.tests}
              label={`Trigger tests of ${one.skill}`}
              onChat={onChat}
            />
          </section>
        ))
      )}
    </section>
  );
}

/**
 * **One of a router's grade's chats, read-only** (#3617): what its terminal showed as the grade
 * ended it, with the previous and next of the grade's kept chats, as a skill's grade's chat is.
 */
export function RouterGradeChatPage({
  plane,
  router,
  workspace,
  persona,
  grade,
  chat,
  onChat,
  onScorecard,
}: RouterScope & {
  grade: string;
  chat: string;
  onChat: (chat: string) => void;
  onScorecard: () => void;
}) {
  const screen = useRead(
    () => commands.routerGradeChat(plane, workspace, persona, grade, chat),
    [plane, workspace, persona, grade, chat],
  );
  const card = useRead(
    () => commands.routerGradeScorecard(plane, workspace, persona, grade),
    [plane, workspace, persona, grade],
  );
  // Taken only in its own shape: a core that answers another leaves it unread.
  const read = Array.isArray(card.data?.skills) ? card.data : undefined;
  return (
    <KeptChatShown
      screen={screen}
      kept={read === undefined ? [] : keptChats(read.skills.flatMap((one) => one.tests))}
      graded={read?.graded}
      chat={chat}
      whose={`the grade of ${router}`}
      onChat={onChat}
      onScorecard={onScorecard}
    />
  );
}
