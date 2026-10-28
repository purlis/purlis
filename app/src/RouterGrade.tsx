// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { commands, ROUTER_GRADE, type RouterGradePlan, type RouterGradeStep } from "./bindings";
import { counted } from "./dispatches";
import { Notice } from "./Notice";
import { tokensSaid } from "./SkillGrade";
import { Button, ButtonRow } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";

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

  const open = () => {
    setAsking(true);
    setSaid(undefined);
    void commands.routerGradePlan(plane, workspace, persona).then((answer) => {
      if (answer.status === "ok") {
        setPlan(answer.data);
        setRefused(undefined);
      } else setRefused(answer.error);
    });
  };

  const start = () => {
    void commands
      .routerGradeStart(plane, workspace, persona, plan?.consent ?? "")
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
        } else setSaid({ tone: "trouble", text: answer.error });
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
        <RouterGradePlanShown plan={plan} onStart={start} onCancel={() => setAsking(false)} />
      )}
    </div>
  );
}

/** The cost-first plan of a router's grade, and the press that starts it. */
function RouterGradePlanShown({
  plan,
  onStart,
  onCancel,
}: {
  plan: RouterGradePlan;
  onStart: () => void;
  onCancel: () => void;
}) {
  const nothing = plan.chats === 0;
  const tests = plan.skills.reduce((all, one) => all + one.tests, 0);
  const limit = `$${(plan.cost_cap_usd ?? 0).toFixed(2)}`;
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
            × {plan.chat_minutes} min. It stops once its chats have spent{" "}
            {tokensSaid(plan.token_cap ?? 0)}
            {plan.cap_set_here ? ", the cap you set on this machine" : ", the default cap"}, or cost{" "}
            {limit}. The chats run on {plan.profile ?? "the project's default profile"}
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
