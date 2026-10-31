// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  commands,
  ROUTER_GRADE,
  type ProfileRow,
  type RouterGradePlan,
  type RouterGradeStep,
} from "./bindings";
import { counted } from "./dispatches";
import {
  KeptChatShown,
  keptChats,
  OUTCOME_SAID,
  pointsSaid,
  TriggerTestsTable,
} from "./GradeScorecard";
import { Notice } from "./Notice";
import {
  costSaid,
  dollarsSaid,
  GradingHeading,
  limitSaid,
  TokenCap,
  tokensSaid,
} from "./SkillGrade";
import { NotRead, ReadingRows, useRead } from "./SkillPanels";
import { Button, ButtonRow } from "./ui/button/Button";
import { Tile, Totals } from "./ui/card/Totals";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { Select } from "./ui/select/Select";

/** The project's default profile, or the profile's own model, in a picker. */
const DEFAULT = "";

/** How a router's grade that ended is said, in one line. */
export function routerEndedSaid(step: RouterGradeStep): string {
  const chats = counted(step.ran, "chat", "chats");
  const scored = step.said === null ? "" : ` ${step.said}`;
  switch (step.ended) {
    case "whole":
      return `Graded in ${chats}.${scored}`;
    case "stopped":
      return `Stopped after ${chats}.${scored}`;
    case "chats-did-not-start":
      return `The grade stopped: its chats did not start. ${step.said ?? ""}`.trim();
    case "over-cap":
      return `Stopped at its cap of ${tokensSaid(step.cap ?? 0)} after ${chats}.${scored}`;
    case "over-limit":
      return `Stopped at its dollar limit after ${chats}.${scored}`;
    case "spend-not-reported":
      return `Stopped after ${chats}: a chat did not say what it spent, so the grade could not be held to its cap.`;
    case "failed":
      return step.said ?? "A fault in purlis ended the grade before it was scored.";
    default:
      return "";
  }
}

/**
 * **Grade the router, where it is chosen** (SK-15e, #3233; ADR 0102 §8): the cost first, then the
 * grade. The plan says what it runs (each of the library's trigger tests with the router in place
 * and with the built-in router, the times it runs), the cap and the dollar limit it is held to,
 * and whose account pays; nothing runs until the person presses Grade on it. A grade's chats are
 * drawn nowhere: this says how far it is, offers Stop, and says how it ended. Its result is the
 * router's line in the studio.
 */
export function RouterGrade({
  plane,
  router,
  workspace,
  persona,
  onGraded,
}: {
  plane: string;
  router: string;
  workspace: string | null;
  persona: string | null;
  /** Told when a grade of this router ended, so the studio reads its grade again. */
  onGraded: () => void;
}) {
  const [plan, setPlan] = useState<RouterGradePlan>();
  const [refused, setRefused] = useState<string>();
  const [asking, setAsking] = useState(false);
  const [step, setStep] = useState<RouterGradeStep>();
  const [said, setSaid] = useState<{ tone: "news" | "trouble"; text: string }>();
  // Where its chats run (#3618): the project's default profile and the profile's own model
  // until the person picks others.
  const [profiles, setProfiles] = useState<readonly ProfileRow[]>([]);
  const [profile, setProfile] = useState(DEFAULT);
  const [model, setModel] = useState(DEFAULT);
  const picked = profile === DEFAULT ? null : profile;
  const pickedModel = model === DEFAULT ? null : model;
  const running = step !== undefined && step.ended === null;
  // The latest to tell, read as an event lands: the listener is not drawn again with it, so no
  // step is lost while it would be.
  const told = useRef(onGraded);
  useEffect(() => {
    told.current = onGraded;
  }, [onGraded]);

  useEffect(() => {
    const listening = listen<RouterGradeStep>(ROUTER_GRADE, (event) => {
      const now = event.payload;
      if (now.plane !== plane || now.router !== router) return;
      setStep(now);
      if (now.ended !== null) {
        setSaid({
          tone: now.ended === "chats-did-not-start" || now.ended === "failed" ? "trouble" : "news",
          text: routerEndedSaid(now),
        });
        told.current();
      }
    }).catch(() => undefined);
    return () => void listening.then((stop) => stop?.()).catch(() => undefined);
  }, [plane, router]);

  /** Reads the plan of a grade on `on` and `onModel`. */
  const read = (on: string | null, onModel: string | null) =>
    void commands
      .routerGradePlan(plane, workspace, persona, on, onModel)
      .then((answer) => {
        if (answer.status === "ok") {
          setPlan(answer.data);
          setRefused(undefined);
        } else setRefused(answer.error);
      })
      .catch((err: unknown) => setRefused(String(err)));

  const open = () => {
    setAsking(true);
    setSaid(undefined);
    read(picked, pickedModel);
    void commands
      .startOptions(plane)
      .then((answer) => {
        if (answer.status === "ok") setProfiles(answer.data.profiles);
      })
      .catch(() => undefined);
  };

  const start = () => {
    void commands
      .routerGradeStart(plane, workspace, persona, picked, pickedModel, plan?.consent ?? "")
      .then((answer) => {
        if (answer.status === "ok") {
          setAsking(false);
          setStep({
            plane,
            router,
            grade: answer.data,
            ran: 0,
            of: plan?.chats ?? 0,
            tokens: null,
            cap: plan?.token_cap ?? 0,
            ended: null,
            said: null,
          });
        } else {
          setSaid({ tone: "trouble", text: answer.error });
          // A plan that changed since it was shown is read again, to be pressed again.
          read(picked, pickedModel);
        }
      });
  };

  return (
    <div data-testid="router-grade">
      {said !== undefined && (
        <Notice
          cause="router-grade"
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
          <p data-testid="router-grade-running">
            Grading the router {router}: {step.ran} of {counted(step.of, "chat", "chats")} done
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
        <Button size="sm" onPress={open}>
          Grade the router…
        </Button>
      ) : refused !== undefined ? (
        <EmptyState
          headline={`purlis could not plan a grade of the router ${router}`}
          body={refused}
          testid="router-grade-refused"
        />
      ) : plan === undefined ? (
        <p className="pending" aria-busy="true">
          Reading what a grade runs…
        </p>
      ) : (
        <RouterGradePlanShown
          plan={plan}
          profiles={profiles}
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
          onCap={(tokens) =>
            void commands
              .routerGradeSetCap(plane, workspace, persona, picked, pickedModel, tokens)
              .then((answer) => {
                if (answer.status === "ok") setPlan(answer.data);
                else setSaid({ tone: "trouble", text: answer.error });
              })
          }
          onStart={start}
          onCancel={() => setAsking(false)}
        />
      )}
    </div>
  );
}

/** **What a router's grade should spend, from its last grade** (#3618): a skill's plan's words
 *  (`costSaid`), and, where that grade ran on another library, that this one may spend otherwise. */
export function routerCostSaid(plan: RouterGradePlan): string {
  const said = costSaid(plan, "router");
  return plan.estimate_same_library === false
    ? `${said} That grade ran on another library, and each chat is handed the whole library, so this one may spend otherwise.`
    : said;
}

/** The cost-first plan of a router's grade, where its chats run and what it may spend, and the
 *  press that starts it. */
function RouterGradePlanShown({
  plan,
  profiles,
  profile,
  model,
  onProfile,
  onModel,
  onCap,
  onStart,
  onCancel,
}: {
  plan: RouterGradePlan;
  profiles: readonly ProfileRow[];
  /** The profile and model picked, `DEFAULT` for none. */
  profile: string;
  model: string;
  onProfile: (to: string) => void;
  onModel: (to: string) => void;
  onCap: (tokens: number) => void;
  onStart: () => void;
  onCancel: () => void;
}) {
  const nothing = plan.chats === 0;
  const tests = plan.skills.reduce((all, one) => all + one.tests, 0);
  return (
    <div data-testid="router-grade-plan">
      {plan.refused !== null && (
        <p className="honest" data-testid="router-grade-cannot-run">
          No grade can run on {plan.profile ?? "this profile"}: {plan.refused}
        </p>
      )}
      {nothing ? (
        <EmptyState
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
            {counted(plan.library, "skill", "skills")}, up to {counted(plan.chats, "chat", "chats")}{" "}
            × {plan.chat_minutes} min. {routerCostSaid(plan)} It stops once its chats have spent{" "}
            {tokensSaid(plan.token_cap ?? 0)}
            {plan.cap_set_here ? ", the cap you set on this machine" : ", the default cap"}, or cost{" "}
            {limitSaid(plan)}. The chats run on {plan.profile ?? "the project's default profile"}
            {plan.model === null ? "" : `, on ${plan.model}`}
            {plan.sandbox === null ? "" : `, in the project's ${plan.sandbox}`}, with none of your
            own Allows and no tools but purlis's that only read, and its account pays for them as
            for any chat. It is recorded{" "}
            {plan.commit === null
              ? "on a library with changes not committed"
              : `on the library at ${plan.commit.slice(0, 12)}`}
            .
          </p>
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
          <ButtonRow>
            <Select
              size="sm"
              aria-label={`Grade the router ${plan.router} on`}
              value={profile}
              options={[
                { value: DEFAULT, label: "The project's default" },
                ...profiles.map((one) => ({ value: one.name, label: one.name })),
              ]}
              onValueChange={onProfile}
            />
            {plan.models.length > 0 && (
              <Select
                size="sm"
                aria-label={`Model the router ${plan.router}'s grade runs on`}
                value={model}
                options={[
                  { value: DEFAULT, label: "The profile's own model" },
                  ...plan.models.map((one) => ({ value: one, label: one })),
                ]}
                onValueChange={onModel}
              />
            )}
          </ButtonRow>
          <TokenCap key={plan.token_cap ?? 0} cap={plan.token_cap ?? 0} onCap={onCap} />
        </>
      )}
      <ButtonRow>
        {!nothing && (
          <Button
            size="sm"
            variant="primary"
            isDisabled={plan.running !== null || plan.refused !== null || plan.consent === null}
            onPress={onStart}
          >
            Grade
          </Button>
        )}
        <Button size="sm" variant="ghost" onPress={onCancel}>
          Cancel
        </Button>
      </ButtonRow>
    </div>
  );
}

/** Where a router was chosen, as its grades are read: the studio's workspace and persona. */
type RouterScope = {
  plane: string;
  router: string;
  workspace: string | null;
  persona: string | null;
};

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
  const against =
    it.arms.router === null || it.arms.built_in === null
      ? undefined
      : `${it.arms.router - it.arms.built_in > 0 ? "+" : ""}${it.arms.router - it.arms.built_in} points`;
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
            {`${end.said} ${it.graded}, on ${it.library}, on ${it.ran_on}${spent}. ${it.said}`}
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
            line={against}
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
