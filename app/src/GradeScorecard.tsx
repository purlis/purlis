// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A grade's scorecard and its saved chats, as pages in the studio** (SK-59, #3239; #3688, the
 * amendment's S6; spec #2762 decision 43). They replace the scorecard's and the grade chat's own
 * tabs: an older launch's tabs of theirs come back as these pages (`studioOfSkillTab`).
 *
 * - **The scorecard** opens with how the grade ended (a banner, in the tone of its end), then its
 *   totals (each arm's pass rate and what it spent), then one row per trigger test with each
 *   arm's runs and verdict, and each of its chats one press away where this machine kept it.
 * - **A saved chat** is what its terminal showed as the grade ended it, drawn read-only
 *   (`ReadOnlyTerminal`, #3676), with the previous and next of the grade's kept chats.
 */
import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  commands,
  type GradeOutcome,
  type GradeScorecard,
  type ScoredArm,
  type ScoredChat,
  type ScoredRuns,
  type SkillLevel,
} from "./bindings";
import { counted } from "./dispatches";
import { Notice } from "./Notice";
import { ReadOnlyTerminal } from "./ReadOnlyTerminal";
import { dollarsSaid, GradingHeading } from "./SkillGrade";
import { NotRead, ReadingRows, useRead } from "./SkillPanels";
import { textSizes } from "./textSize";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { Tile, Totals } from "./ui/card/Totals";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./ui/table/Table";

/** The columns a grade's chat is kept at: the size it ran at, drawn nowhere (`skillgrade.rs`). */
const GRADE_CHAT_COLUMNS = 120;
const GRADE_CHAT_ROWS = 40;

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

/** **How a grade ended, as its mark and its banner say it** (S6), and the tone of each. */
export const OUTCOME_SAID: Record<
  GradeOutcome,
  { said: string; tone: "news" | "attention" | "trouble"; badge: "ok" | "waiting" | "neutral" }
> = {
  graded: { said: "Graded", tone: "news", badge: "ok" },
  stopped: { said: "Stopped", tone: "attention", badge: "waiting" },
  "at-limit": { said: "Stopped at its limit", tone: "attention", badge: "waiting" },
  interrupted: { said: "Interrupted", tone: "trouble", badge: "neutral" },
};

/** A pass rate in points as a tile and a bar say it: `86%`. */
export function pointsSaid(points: number | null): string {
  return points === null ? "Not run" : `${points}%`;
}

/** The difference between two arms' rates, as a tile's line says it: `+18 points`. */
function against(one: number | null, other: number | null): string | undefined {
  if (one === null || other === null) return undefined;
  return `${points(one - other)} points`;
}

/** Each chat of `card` this machine kept, in the order its rows show them. */
function keptChats(card: GradeScorecard): {
  chat: ScoredChat;
  arm: ScoredArm;
  query: string;
}[] {
  return card.tests.flatMap((test) =>
    test.arms.flatMap((runs) =>
      runs.chats
        .filter((chat) => chat.kept)
        .map((chat) => ({ chat, arm: runs.arm, query: test.query })),
    ),
  );
}

/** The arms a scorecard's rows have a column for, in the order its tests ran them. */
function armsOf(card: GradeScorecard): ScoredArm[] {
  const arms: ScoredArm[] = [];
  for (const test of card.tests)
    for (const runs of test.arms) if (!arms.includes(runs.arm)) arms.push(runs.arm);
  return arms;
}

/** One test in one arm, in its cell: its runs, its verdict and its chats. */
function ArmCell({
  runs,
  query,
  onChat,
}: {
  runs: ScoredRuns | undefined;
  query: string;
  onChat: (chat: string) => void;
}): ReactNode {
  if (runs === undefined || runs.pass === null) return <Badge>Not run</Badge>;
  return (
    <span className="skill-grading-arm">
      {`${runs.triggered} of ${runs.runs} `}
      <Badge variant={runs.pass ? "ok" : "waiting"}>{runs.pass ? "Passed" : "Failed"}</Badge>
      {runs.chats.map((chat) => (
        <Button
          key={chat.id}
          size="sm"
          variant="ghost"
          aria-label={`Open ${ARM_SAID[runs.arm].toLowerCase()}, chat ${chat.run}, of “${query}”`}
          isDisabled={!chat.kept}
          tooltip={
            chat.kept
              ? "Opens the chat as it showed when the grade ended it, read-only"
              : "Not kept on this machine: purlis keeps a grade's chats where the grade ran, for the skill's newest grades"
          }
          onPress={() => onChat(chat.id)}
        >
          {`Chat ${chat.run}`}
        </Button>
      ))}
    </span>
  );
}

/** Tells `onGradedAt` the level of the copy `card` was of, once it is read (#3978). */
function useGradedAt(
  card: GradeScorecard | undefined,
  onGradedAt: ((level: SkillLevel) => void) | undefined,
) {
  const told = useRef(onGradedAt);
  useEffect(() => {
    told.current = onGradedAt;
  });
  const level = card?.level ?? null;
  const levelKey = level === null ? "" : JSON.stringify(level);
  useEffect(() => {
    if (level !== null) told.current?.(level);
    // The level as a value: an equal one read again tells nothing anew.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [levelKey]);
}

/**
 * **A grade's scorecard page** (S6): see the module. `onChat` opens one of its kept chats;
 * `onGradeAgain` opens a new plan, where this copy can be graded from here. `onGradedAt` is told
 * the level of the copy the grade was of, once read (#3978).
 */
export function GradeScorecardPage({
  plane,
  name,
  grade,
  onChat,
  onGradeAgain,
  onGradedAt,
}: {
  plane: string;
  name: string;
  grade: string;
  onChat: (chat: string) => void;
  onGradeAgain?: () => void;
  onGradedAt?: (level: SkillLevel) => void;
}) {
  const [again, setAgain] = useState(0);
  const [endPutAway, setEndPutAway] = useState(false);
  const card = useRead(
    () => commands.skillGradeScorecard(plane, name, grade),
    [plane, name, grade, again],
  );
  useGradedAt(card.data, onGradedAt);
  if (card.refused !== undefined)
    return (
      <NotRead
        what={`this grade of ${name}`}
        why={card.refused}
        onAgain={() => setAgain((n) => n + 1)}
        testid="grade-scorecard-refused"
      />
    );
  if (card.data === undefined)
    return <ReadingRows what="the grade's scorecard" testid="skill-grading-reading" />;
  const it = card.data;
  const end = OUTCOME_SAID[it.outcome];
  const arms = armsOf(it);
  const spent = it.spent.cost_usd === null ? "" : ` for ${dollarsSaid(it.spent.cost_usd)}`;
  const about = (
    <span data-testid="grade-scorecard-about">
      {`${end.said} ${it.graded}${it.after_edit ? ", after an edit you saved" : ""}, version ${it.version}, on ${it.ran_on}${spent}. ${it.ended}${differenceSaid(it)}`}
    </span>
  );
  return (
    <section
      className="skill-studio-page"
      data-testid="grade-scorecard"
      aria-label={`Scorecard of ${name}`}
    >
      <GradingHeading name={`Grade of ${name} · ${it.graded}`} />
      {onGradeAgain !== undefined ? (
        <Notice
          cause={`skill-grade-end:${grade}`}
          tone={end.tone}
          label={end.said}
          fixes={[{ label: "Grade again", onPress: onGradeAgain }]}
        >
          {about}
        </Notice>
      ) : (
        // A copy this studio cannot grade from here (one deleted since): its end, put away
        // once read.
        !endPutAway && (
          <Notice
            cause={`skill-grade-end:${grade}`}
            tone={end.tone}
            label={end.said}
            onDismiss={() => setEndPutAway(true)}
          >
            {about}
          </Notice>
        )
      )}
      {it.asked > 0 && (
        <p className="honest" data-testid="grade-scorecard-asked">
          {counted(it.asked, "chat", "chats")} asked you something. A grade's chat waits on nobody,
          so purlis ended each at its ask and counts none of them: open its chat below to read what
          it asked.
        </p>
      )}
      {it.outcome !== "interrupted" && (
        <Totals aria-label={`Totals of the grade of ${name}`}>
          <Tile label="With it" value={pointsSaid(it.arms.with)} line="of tests passed" />
          <Tile
            label="Without it"
            value={pointsSaid(it.arms.without)}
            line={against(it.arms.with, it.arms.without)}
          />
          {it.arms.previous !== null && (
            <Tile
              label="Previous version"
              value={pointsSaid(it.arms.previous)}
              line={against(it.arms.with, it.arms.previous)}
            />
          )}
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
      <section aria-labelledby="grade-scorecard-tests">
        <PanelSectionTitle id="grade-scorecard-tests">Each trigger test</PanelSectionTitle>
        <Table aria-label={`Trigger tests of ${name}`} data-testid="grade-scorecard-tests">
          <TableHeader>
            <TableColumn isRowHeader>Trigger test</TableColumn>
            <TableColumn>Should</TableColumn>
            {arms.map((arm) => (
              <TableColumn key={arm}>{ARM_SAID[arm]}</TableColumn>
            ))}
          </TableHeader>
          <TableBody>
            {it.tests.map((test, at) => (
              <TableRow key={at} id={String(at)} textValue={test.query}>
                <TableCell>
                  <code>{test.query}</code>
                </TableCell>
                <TableCell>{test.should_trigger ? "Start it" : "Leave it alone"}</TableCell>
                {arms.map((arm) => {
                  const runs = test.arms.find((one) => one.arm === arm);
                  return (
                    <TableCell
                      key={arm}
                      textValue={
                        runs === undefined ? "Not run" : runsSaid(runs, test.should_trigger)
                      }
                    >
                      <span data-testid={`grade-scorecard-${arm}`}>
                        <ArmCell runs={runs} query={test.query} onChat={onChat} />
                      </span>
                    </TableCell>
                  );
                })}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </section>
    </section>
  );
}

/**
 * **One of a grade's chats, read-only** (SK-59, S6): what its terminal showed as the grade ended
 * it, its scrollback with it, read as text by a screen reader and taking no input. The chat
 * itself is gone: this is what it showed.
 */
export function GradeChatPage({
  plane,
  name,
  grade,
  chat,
  onChat,
  onScorecard,
  onGradedAt,
}: {
  plane: string;
  name: string;
  grade: string;
  chat: string;
  onChat: (chat: string) => void;
  onScorecard: () => void;
  onGradedAt?: (level: SkillLevel) => void;
}) {
  const screen = useRead(
    () => commands.skillGradeChat(plane, name, grade, chat),
    [plane, name, grade, chat],
  );
  const card = useRead(
    () => commands.skillGradeScorecard(plane, name, grade),
    [plane, name, grade],
  );
  useGradedAt(card.data, onGradedAt);
  // Taken only in its own shape: a core that answers another leaves it unread.
  const read = Array.isArray(card.data?.tests) ? card.data : undefined;
  const kept = read === undefined ? [] : keptChats(read);
  const at = kept.findIndex((one) => one.chat.id === chat);
  const here = kept[at];
  const before = kept[at - 1];
  const after = at === -1 ? undefined : kept[at + 1];
  const ways = (
    <ButtonRow>
      <Button isDisabled={before === undefined} onPress={() => before && onChat(before.chat.id)}>
        Previous chat
      </Button>
      <Button isDisabled={after === undefined} onPress={() => after && onChat(after.chat.id)}>
        Next chat
      </Button>
      <Button onPress={onScorecard}>Back to the scorecard</Button>
    </ButtonRow>
  );
  return (
    <section className="skill-studio-page" data-testid="grade-chat">
      <GradingHeading
        name={
          here === undefined || read === undefined
            ? `A chat of ${name}'s grade`
            : `Chat ${here.chat.run} of the ${read.graded} grade`
        }
        description={
          here === undefined
            ? `What this chat of ${name}'s grade showed when the grade ended it.`
            : `${ARM_SAID[here.arm]} · trigger test “${here.query}”. What it showed when the grade ended it.`
        }
      />
      {screen.refused !== undefined ? (
        <EmptyState
          variant="panel"
          headline="This grade's chat is not kept here"
          body={screen.refused}
          testid="grade-chat-refused"
        />
      ) : screen.data === undefined ? (
        <ReadingRows what="the grade's chat" testid="skill-grading-reading" />
      ) : (
        <ReadOnlyTerminal
          screen={screen.data}
          label={`A grade's chat of ${name}`}
          note="It takes no input and can't be resumed."
          cols={GRADE_CHAT_COLUMNS}
          rows={GRADE_CHAT_ROWS}
          fontSize={textSizes().terminal}
        />
      )}
      {ways}
    </section>
  );
}
