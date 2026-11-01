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
  type ScoredEval,
  type ScoredEvalArm,
  type ScoredRuns,
  type ScoredTest,
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
export const ARM_SAID: Record<ScoredRuns["arm"], string> = {
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
export function against(one: number | null, other: number | null): string | undefined {
  if (one === null || other === null) return undefined;
  return `${points(one - other)} points`;
}

/** A chat a scorecard's row offers, with the arm and what it ran: `about` says so in words. */
export type KeptChat = { chat: ScoredChat; arm: ScoredArm; about: string };

/** Each chat of `tests` this machine kept, in the order their rows show them: a skill's
 *  scorecard's, or every skill's of a router's (#3617). */
export function keptChats(tests: readonly ScoredTest[]): KeptChat[] {
  return tests.flatMap((test) =>
    test.arms.flatMap((runs) =>
      runs.chats
        .filter((chat) => chat.kept)
        .map((chat) => ({ chat, arm: runs.arm, about: `trigger test “${test.query}”` })),
    ),
  );
}

/** Each chat of `evals` this machine kept, its judge's after it, in the order their rows show
 *  them (SK-55). */
export function keptEvalChats(evals: readonly ScoredEval[]): KeptChat[] {
  return evals.flatMap((one) =>
    one.arms.flatMap((arm) =>
      arm.results.flatMap((run) =>
        [
          { chat: run.chat, about: `outcome eval ${one.id}` },
          { chat: run.judge, about: `the judge of outcome eval ${one.id}` },
        ].flatMap(({ chat, about }) =>
          chat !== null && chat.kept ? [{ chat, arm: arm.arm, about }] : [],
        ),
      ),
    ),
  );
}

/** The arms a scorecard's rows have a column for, in the order its tests ran them. */
function armsOf(tests: readonly ScoredTest[]): ScoredArm[] {
  const arms: ScoredArm[] = [];
  for (const test of tests)
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
          so purlis ended each at its ask
          {it.evals.length > 0
            ? ": a trigger test's is not counted, and an outcome eval's fails that run. Open"
            : " and counts none of them: open"}{" "}
          its chat below to read what it asked.
        </p>
      )}
      {it.outcome !== "interrupted" && (it.tests.length > 0 || it.evals.length === 0) && (
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
      {it.outcome_arms !== null && it.outcome !== "interrupted" && (
        <Totals aria-label={`Outcome evals of the grade of ${name}`}>
          <Tile
            label="With it"
            value={pointsSaid(it.outcome_arms.with)}
            line="of expectations passed"
          />
          <Tile
            label="Without it"
            value={pointsSaid(it.outcome_arms.without)}
            line={against(it.outcome_arms.with, it.outcome_arms.without)}
          />
          {it.outcome_arms.previous !== null && (
            <Tile
              label="Previous version"
              value={pointsSaid(it.outcome_arms.previous)}
              line={against(it.outcome_arms.with, it.outcome_arms.previous)}
            />
          )}
        </Totals>
      )}
      {(it.tests.length > 0 || it.evals.length === 0) && (
        <section aria-labelledby="grade-scorecard-tests">
          <PanelSectionTitle id="grade-scorecard-tests">Each trigger test</PanelSectionTitle>
          <TriggerTestsTable tests={it.tests} label={`Trigger tests of ${name}`} onChat={onChat} />
        </section>
      )}
      {it.evals.length > 0 && (
        <section aria-labelledby="grade-scorecard-evals">
          <PanelSectionTitle id="grade-scorecard-evals">Each outcome eval</PanelSectionTitle>
          <OutcomeEvalsTable evals={it.evals} label={`Outcome evals of ${name}`} onChat={onChat} />
        </section>
      )}
    </section>
  );
}

/**
 * **A scorecard's table of trigger tests** (S6): one row per test, a column per arm with its runs
 * and verdict, and each of its chats one press away where this machine kept it. A skill's
 * scorecard draws one; a router's, one per skill (#3617).
 */
export function TriggerTestsTable({
  tests,
  label,
  onChat,
}: {
  tests: readonly ScoredTest[];
  label: string;
  onChat: (chat: string) => void;
}) {
  const arms = armsOf(tests);
  return (
    <Table aria-label={label} data-testid="grade-scorecard-tests">
      <TableHeader>
        <TableColumn isRowHeader>Trigger test</TableColumn>
        <TableColumn>Should</TableColumn>
        {arms.map((arm) => (
          <TableColumn key={arm}>{ARM_SAID[arm]}</TableColumn>
        ))}
      </TableHeader>
      <TableBody>
        {tests.map((test, at) => (
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
                  textValue={runs === undefined ? "Not run" : runsSaid(runs, test.should_trigger)}
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
  );
}

/** The arms a scorecard's outcome evals have a column for, in the order they ran them. */
function evalArmsOf(evals: readonly ScoredEval[]): ScoredArm[] {
  const arms: ScoredArm[] = [];
  for (const one of evals)
    for (const arm of one.arms) if (!arms.includes(arm.arm)) arms.push(arm.arm);
  return arms;
}

/** One outcome eval in one arm, in a sentence: what its judges found, and why any run failed. */
export function evalSaid(arm: ScoredEvalArm | undefined): string {
  if (arm === undefined || arm.points === null) return "Not judged";
  const failed = arm.results.flatMap((run) => (run.failed === null ? [] : [run.failed]));
  return [`${arm.points}% over ${counted(arm.runs, "run", "runs")}`, ...failed].join(". ");
}

/** One outcome eval in one arm, in its cell: its rate, why any run failed, and each run's chat and
 *  judge, one press away where this machine kept them. */
function EvalCell({
  arm,
  id,
  onChat,
}: {
  arm: ScoredEvalArm | undefined;
  id: string;
  onChat: (chat: string) => void;
}): ReactNode {
  if (arm === undefined) return <Badge>Not judged</Badge>;
  const button = (chat: ScoredChat, what: string) => (
    <Button
      key={`${what}-${chat.id}`}
      size="sm"
      variant="ghost"
      aria-label={`Open ${ARM_SAID[arm.arm].toLowerCase()}, ${what.toLowerCase()} ${chat.run}, of outcome eval ${id}`}
      isDisabled={!chat.kept}
      tooltip={
        chat.kept
          ? "Opens the chat as it showed when the grade ended it, read-only"
          : "Not kept on this machine: purlis keeps a grade's chats where the grade ran, for the skill's newest grades"
      }
      onPress={() => onChat(chat.id)}
    >
      {`${what} ${chat.run}`}
    </Button>
  );
  return (
    <span className="skill-grading-arm">
      {arm.points === null ? (
        <Badge>Not judged</Badge>
      ) : (
        `${arm.points}% over ${counted(arm.runs, "run", "runs")} `
      )}
      {arm.results.flatMap((run) => [
        ...(run.chat === null ? [] : [button(run.chat, "Chat")]),
        ...(run.judge === null ? [] : [button(run.judge, "Judge")]),
      ])}
      {arm.results.flatMap((run) =>
        run.failed === null
          ? []
          : [
              <span
                className="honest"
                key={`why-${run.run}`}
                data-testid="grade-scorecard-eval-why"
              >
                {`Run ${run.run}: ${run.failed}`}
              </span>,
            ],
      )}
    </span>
  );
}

/** What the runs of one expectation came to in one arm, in words. */
function checksSaid(arm: ScoredEvalArm, at: number): string {
  // A run with a score counts, its failure among them: that is what its arm's rate counts.
  const judged = arm.results.filter((run) => run.passed !== null);
  if (judged.length === 0) return "not judged";
  // A row too long for the record keeps each run's count and drops its verdicts.
  if (judged.some((run) => run.checks.length === 0 && run.failed === null))
    return "its verdicts were not kept";
  const passed = judged.filter((run) => run.checks[at]?.passed === true).length;
  return `${passed} of ${judged.length} passed`;
}

/**
 * **A scorecard's outcome evals** (SK-55, #3235): one row per eval, each arm's rate, its chats and
 * its judges, why any run failed, and under it each expectation with what each arm's judges found.
 */
export function OutcomeEvalsTable({
  evals,
  label,
  onChat,
}: {
  evals: readonly ScoredEval[];
  label: string;
  onChat: (chat: string) => void;
}) {
  const arms = evalArmsOf(evals);
  return (
    <>
      <Table aria-label={label} data-testid="grade-scorecard-evals">
        <TableHeader>
          <TableColumn isRowHeader>Outcome eval</TableColumn>
          {arms.map((arm) => (
            <TableColumn key={arm}>{ARM_SAID[arm]}</TableColumn>
          ))}
        </TableHeader>
        <TableBody>
          {evals.map((one) => (
            <TableRow key={one.id} id={one.id} textValue={`Outcome eval ${one.id}`}>
              <TableCell>{`Eval ${one.id}`}</TableCell>
              {arms.map((arm) => {
                const scored = one.arms.find((each) => each.arm === arm);
                return (
                  <TableCell key={arm} textValue={evalSaid(scored)}>
                    <EvalCell arm={scored} id={one.id} onChat={onChat} />
                  </TableCell>
                );
              })}
            </TableRow>
          ))}
        </TableBody>
      </Table>
      {evals.map((one) => (
        <ul aria-label={`Expectations of outcome eval ${one.id}`} key={one.id}>
          {one.expectations.map((text, at) => (
            <li key={at} data-testid="grade-scorecard-expectation">
              <code>{text}</code>
              {one.arms.map((arm) => ` · ${ARM_SAID[arm.arm]}: ${checksSaid(arm, at)}`).join("")}
              {one.arms.flatMap((arm) =>
                arm.results.flatMap((run) => {
                  const evidence = run.checks[at]?.evidence ?? "";
                  return evidence === ""
                    ? []
                    : [
                        <p className="honest" key={`${arm.arm}-${run.run}`}>
                          {`${ARM_SAID[arm.arm]}, run ${run.run}: ${evidence}`}
                        </p>,
                      ];
                }),
              )}
            </li>
          ))}
        </ul>
      ))}
    </>
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
  return (
    <KeptChatShown
      screen={screen}
      kept={read === undefined ? [] : [...keptChats(read.tests), ...keptEvalChats(read.evals)]}
      graded={read?.graded}
      chat={chat}
      whose={`${name}'s grade`}
      onChat={onChat}
      onScorecard={onScorecard}
    />
  );
}

/**
 * **A kept chat of a grade, drawn read-only** (SK-59, S6), with the previous and next of the
 * grade's kept chats: a skill's grade's or a router's (#3617). `kept` is every chat the grade's
 * scorecard offers, `graded` when it ran, where its scorecard was read; `whose` names the grade
 * in its heading ("deploy's grade").
 */
export function KeptChatShown({
  screen,
  kept,
  graded,
  chat,
  whose,
  onChat,
  onScorecard,
}: {
  screen: { data?: string; refused?: string };
  kept: readonly KeptChat[];
  graded: string | undefined;
  chat: string;
  whose: string;
  onChat: (chat: string) => void;
  onScorecard: () => void;
}) {
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
          here === undefined || graded === undefined
            ? `A chat of ${whose}`
            : `Chat ${here.chat.run} of the ${graded} grade`
        }
        description={
          here === undefined
            ? `What this chat of ${whose} showed when the grade ended it.`
            : `${ARM_SAID[here.arm]} · ${here.about}. What it showed when the grade ended it.`
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
          label={`A chat of ${whose}`}
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
