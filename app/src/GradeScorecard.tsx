// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type GradeScorecard, type ScoredRuns } from "./bindings";
import type { ViewRef } from "./tabs";
import { counted } from "./dispatches";
import { ReadOnlyTerminal } from "./ReadOnlyTerminal";
import { textSizes } from "./textSize";
import { Button } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";

/** The view name of a grade's scorecard (SK-59, #3239). */
export const GRADE_VIEW = "skill-grade";

/** The view name of one of a grade's chats, shown read-only (SK-59, #3239). */
export const GRADE_CHAT_VIEW = "skill-grade-chat";

/** The columns a grade's chat is kept at: the size it ran at, drawn nowhere (`skillgrade.rs`). */
const GRADE_CHAT_COLUMNS = 120;
const GRADE_CHAT_ROWS = 40;

/**
 * A grade's scorecard, keyed by its skill and its id: `deploy@01J…`. Neither a skill's name nor
 * a grade's id holds an `@`.
 */
export function gradeView(skill: string, grade: string): ViewRef {
  return { from: null, view: GRADE_VIEW, key: `${skill}@${grade}` };
}

/** One of a grade's chats, keyed by its skill, its grade and its id: `deploy@01J…@01K…`. */
export function gradeChatView(skill: string, grade: string, chat: string): ViewRef {
  return { from: null, view: GRADE_CHAT_VIEW, key: `${skill}@${grade}@${chat}` };
}

/** The grade a scorecard's tab is of, from its key; `undefined` for any other tab. */
export function gradeOfView(view: ViewRef): { skill: string; grade: string } | undefined {
  if (view.from !== null || view.view !== GRADE_VIEW) return undefined;
  const [skill, grade, ...rest] = view.key.split("@");
  return skill && grade && rest.length === 0 ? { skill, grade } : undefined;
}

/** The chat a grade's chat tab shows, from its key; `undefined` for any other tab. */
export function gradeChatOfView(
  view: ViewRef,
): { skill: string; grade: string; chat: string } | undefined {
  if (view.from !== null || view.view !== GRADE_CHAT_VIEW) return undefined;
  const [skill, grade, chat, ...rest] = view.key.split("@");
  return skill && grade && chat && rest.length === 0 ? { skill, grade, chat } : undefined;
}

/** What a grade's scorecard tab is called. */
export function gradeTitle(skill: string): string {
  return `Grade · ${skill}`;
}

/** What a grade's chat tab is called: its place in its test's runs, from 1. */
export function gradeChatTitle(skill: string, run: number): string {
  return `Grade chat ${run} · ${skill}`;
}

/** An arm as the scorecard says it. */
const ARM_SAID: Record<ScoredRuns["arm"], string> = {
  with: "With the skill",
  without: "Without it",
  previous: "Its previous version",
  router: "With the router",
  "built-in-router": "With the built-in router",
};

/** A number of points with its sign: `+25`, `-5`, `0`. */
function points(delta: number): string {
  return `${delta > 0 ? "+" : ""}${delta}`;
}

/** The difference a grade measured, after how it ended: against the skill's previous version
 *  where it compared one (SK-57), with the difference against no skill beside it, else against
 *  no skill. */
export function differenceSaid(card: GradeScorecard): string {
  if (card.delta === null) return ".";
  if (card.against === null)
    return `, the skill made a difference of ${points(card.delta)} points.`;
  const without =
    card.delta_without === null
      ? ""
      : `, and the skill ${points(card.delta_without)} against no skill`;
  return `, this version made a difference of ${points(card.delta)} points against its previous version ${card.against}${without}.`;
}

/** One arm of one test, in a sentence: what ran, what started the skill, and the asks. */
export function runsSaid(runs: ScoredRuns, shouldTrigger: boolean): string {
  const verdict = runs.pass === null ? "not run" : runs.pass ? "passed" : "failed";
  const started =
    runs.runs === 0
      ? "no chat ran"
      : `${runs.triggered} of ${counted(runs.runs, "chat", "chats")} started it`;
  const asked =
    runs.asked === 0
      ? ""
      : `; ${counted(runs.asked, "chat", "chats")} asked you something, and purlis ended ${runs.asked === 1 ? "it" : "them"} there without counting ${runs.asked === 1 ? "it" : "them"}`;
  return `${started} (${shouldTrigger ? "should start it" : "should leave it alone"}), ${verdict}${asked}`;
}

/**
 * **A grade's scorecard** (SK-59, #3239; spec #2762 decision 43): the grade as its row says it,
 * then each trigger test in each arm, with each of its chats one click away. A chat opens
 * read-only in a tab of its own, as it showed when the grade ended it; a chat this machine did
 * not keep (a teammate's grade, or one past the skill's newest three) says so.
 */
export function GradeScorecardTab({
  plane,
  skill,
  grade,
  onOpenView,
}: {
  plane: string;
  skill: string;
  grade: string;
  onOpenView?: (view: ViewRef, title: string) => void;
}) {
  const [card, setCard] = useState<GradeScorecard>();
  const [refused, setRefused] = useState<string>();
  useEffect(() => {
    let live = true;
    void commands.skillGradeScorecard(plane, skill, grade).then((answer) => {
      if (!live) return;
      if (answer.status === "ok") setCard(answer.data);
      else setRefused(answer.error);
    });
    return () => {
      live = false;
    };
  }, [plane, skill, grade]);
  if (refused !== undefined)
    return (
      <EmptyState
        headline={`purlis could not read this grade of ${skill}`}
        body={refused}
        testid="grade-scorecard-refused"
      />
    );
  if (card === undefined)
    return (
      <p className="pending" aria-busy="true">
        Reading the grade's scorecard…
      </p>
    );
  return (
    <section
      className="grade-scorecard"
      data-testid="grade-scorecard"
      aria-label={`Scorecard of ${skill}`}
    >
      <p data-testid="grade-scorecard-about">
        Graded {card.graded}
        {card.after_edit ? ", after an edit you saved" : ""}, version {card.version}, on{" "}
        {card.ran_on}. {card.ended}
        {differenceSaid(card)}
      </p>
      {card.asked > 0 && (
        <p className="honest" data-testid="grade-scorecard-asked">
          {counted(card.asked, "chat", "chats")} asked you something. A grade's chat waits on
          nobody, so purlis ended each at its ask and counts none of them: open its run below to
          read what it asked.
        </p>
      )}
      <ol aria-label={`Trigger tests of ${skill}`}>
        {card.tests.map((test, at) => (
          <li key={at} data-testid="grade-scorecard-test">
            <code>{test.query}</code>
            <ul>
              {test.arms.map((runs) => (
                <li key={runs.arm} data-testid={`grade-scorecard-${runs.arm}`}>
                  {ARM_SAID[runs.arm]}: {runsSaid(runs, test.should_trigger)}.{" "}
                  {runs.chats.map((chat) => (
                    <Button
                      key={chat.id}
                      size="sm"
                      variant="ghost"
                      isDisabled={!chat.kept || onOpenView === undefined}
                      tooltip={
                        chat.kept
                          ? "Opens the chat as it showed when the grade ended it, read-only"
                          : "Not kept on this machine: purlis keeps a grade's chats where the grade ran, for the skill's newest grades"
                      }
                      onPress={() =>
                        onOpenView?.(
                          gradeChatView(skill, grade, chat.id),
                          gradeChatTitle(skill, chat.run),
                        )
                      }
                    >
                      {`Open run ${chat.run}`}
                    </Button>
                  ))}
                </li>
              ))}
            </ul>
          </li>
        ))}
      </ol>
    </section>
  );
}

/**
 * **One of a grade's chats, read-only** (SK-59, #3239): what its terminal showed as the grade
 * ended it, its scrollback with it, drawn by the terminal's read-only mode (`ReadOnlyTerminal`,
 * #3676) from the theme's terminal tokens, read as text by a screen reader, and taking
 * no input. The chat itself is gone: this is what it showed.
 */
export function GradeChatTab({
  plane,
  skill,
  grade,
  chat,
}: {
  plane: string;
  skill: string;
  grade: string;
  chat: string;
}) {
  const [screen, setScreen] = useState<string>();
  const [refused, setRefused] = useState<string>();
  useEffect(() => {
    let live = true;
    void commands.skillGradeChat(plane, skill, grade, chat).then((answer) => {
      if (!live) return;
      if (answer.status === "ok") setScreen(answer.data);
      else setRefused(answer.error);
    });
    return () => {
      live = false;
    };
  }, [plane, skill, grade, chat]);
  if (refused !== undefined)
    return (
      <EmptyState
        headline="This grade's chat is not kept here"
        body={refused}
        testid="grade-chat-refused"
      />
    );
  if (screen === undefined)
    return (
      <p className="pending" aria-busy="true">
        Reading the grade's chat…
      </p>
    );
  return (
    <div className="grade-chat" data-testid="grade-chat">
      <p className="honest">What this chat of {skill}'s grade showed when the grade ended it.</p>
      <ReadOnlyTerminal
        screen={screen}
        label={`A grade's chat of ${skill}`}
        note="It takes no input and can't be resumed."
        cols={GRADE_CHAT_COLUMNS}
        rows={GRADE_CHAT_ROWS}
        fontSize={textSizes().terminal}
      />
    </div>
  );
}
