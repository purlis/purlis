// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's Grades tab, and its grading pages** (#3688, the amendment's S6; extends decision
 * 43). Grading lives in the Skill studio's one tab, under the skill:
 *
 * - **The Grades tab** shows the latest grade as bars (with it, without it, its previous
 *   version), every grade with how it ended (Graded, Stopped, Stopped at its limit,
 *   Interrupted), and the grade-after-each-edit row, which says what each edit may spend before
 *   it is turned on. A skill with no trigger tests says what they are instead, and offers to
 *   write them, from a template, or to ask a chat to (#3925).
 * - **Grade** opens the plan, a running grade shows its progress, and a grade's row opens its
 *   scorecard, from which each kept chat opens read-only: pages in the studio's history
 *   ({@link GradingAt}), with their own crumbs, never tabs of their own.
 */
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  commands,
  SKILL_GRADE,
  SKILL_GRADE_AFTER_EDIT,
  type GradeRow,
  type SkillGradeAfterEditStep,
  type SkillGradeStep,
  type SkillLevel,
} from "./bindings";
import { GradeChatPage, GradeScorecardPage, OUTCOME_SAID, pointsSaid } from "./GradeScorecard";
import { Notice } from "./Notice";
import {
  GradePlanPage,
  GradeRunningPage,
  afterEditSaid,
  dollarsSaid,
  endedSaid,
} from "./SkillGrade";
import { NotRead, ReadingRows, useRead } from "./SkillPanels";
import { sameLevel } from "./skillChanges";
import { askSkillChat } from "./skillDraftAsks";
import type { GradingAt } from "./skillStudio";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Meter, MeterGroup } from "./ui/meter/Meter";
import { PanelSectionTitle } from "./ui/panel/Panel";
import type { Crumb } from "./ui/panel/ViewBar";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./ui/table/Table";

/** A skill's trigger tests file, in its folder: skill-creator's, which a grade runs. */
export const TRIGGER_TESTS_FILE = "evals/triggers.json";

/** **Where a skill's trigger tests file is opened to write in**, by the skill's page, or why it
 *  can't be (a skill installed from a source, a file outside the project): then neither Write
 *  trigger tests nor Ask a chat to write them is offered, as neither would land in the skill. */
export type TestsFile = { open: () => void } | { reason: string };

/** How a grading page is gone to: a step of the studio's history, or said again in its place
 *  (a grade started from its plan, a scorecard once its grade ended). `undefined` is the skill's
 *  Grades tab. */
export type GradingGo = (to: GradingAt | undefined, how: "go" | "replace" | "back") => void;

/** **The crumbs a grading page adds after its skill's Grades tab.** */
export function gradingPath(grading: GradingAt): Crumb[] {
  switch (grading.at) {
    case "plan":
      return [{ id: "grading", label: grading.afterEdit ? "Grade after each edit" : "New grade" }];
    case "running":
      return [{ id: "grading", label: "Grading…" }];
    case "scorecard":
      return [{ id: "grading", label: "Scorecard" }];
    case "chat":
      return [
        { id: "scorecard", label: "Scorecard" },
        { id: "grading", label: "Saved chat" },
      ];
  }
}

/** Where a grading crumb goes: a chat's crumb back to its scorecard; any other, nowhere new. */
export function gradingCrumbTo(grading: GradingAt, crumb: string): GradingAt | undefined {
  return crumb === "scorecard" && grading.at === "chat"
    ? { at: "scorecard", grade: grading.grade }
    : undefined;
}

/** What a grade's row opens its scorecard by: what ran, for a grade that compared nothing. */
function openSaid(row: GradeRow): string {
  return row.outcome === "interrupted" ? "What ran" : "Scorecard";
}

/** What a grade spent, as its row says it. */
function spentSaid(row: GradeRow): string {
  return row.spent.cost_usd === null ? "Not said" : dollarsSaid(row.spent.cost_usd);
}

/** **The latest grade, as bars** (S6): with it, without it, and its previous version. */
function Latest({ name, row }: { name: string; row: GradeRow }) {
  const spent = row.spent.cost_usd === null ? "" : ` · ${dollarsSaid(row.spent.cost_usd)}`;
  return (
    <section aria-labelledby="skill-grades-latest" data-testid="skill-grades-latest">
      <PanelSectionTitle id="skill-grades-latest">
        {`Latest · ${row.graded} · ${row.ran_on}${spent}`}
      </PanelSectionTitle>
      {row.outcome_arms !== null && (
        <MeterGroup aria-label={`Outcome evals of the latest grade of ${name}`}>
          <Meter
            label="Outcome evals with it"
            value={row.outcome_arms.with ?? 0}
            valueLabel={pointsSaid(row.outcome_arms.with)}
          />
          <Meter
            label="Outcome evals without it"
            variant="baseline"
            value={row.outcome_arms.without ?? 0}
            valueLabel={pointsSaid(row.outcome_arms.without)}
          />
          {row.outcome_arms.previous !== null && (
            <Meter
              label="Outcome evals, previous version"
              variant="previous"
              value={row.outcome_arms.previous}
              valueLabel={pointsSaid(row.outcome_arms.previous)}
            />
          )}
        </MeterGroup>
      )}
      {row.arms.with === null && row.arms.without === null ? (
        row.outcome_arms === null && (
          <p className="honest">{`${OUTCOME_SAID[row.outcome].said}: nothing of ${name} was compared.`}</p>
        )
      ) : (
        <MeterGroup aria-label={`The latest grade of ${name}`}>
          <Meter
            label="With it"
            value={row.arms.with ?? 0}
            valueLabel={pointsSaid(row.arms.with)}
          />
          <Meter
            label="Without it"
            variant="baseline"
            value={row.arms.without ?? 0}
            valueLabel={pointsSaid(row.arms.without)}
          />
          {row.arms.previous !== null && (
            <Meter
              label="Previous version"
              variant="previous"
              value={row.arms.previous}
              valueLabel={pointsSaid(row.arms.previous)}
            />
          )}
        </MeterGroup>
      )}
    </section>
  );
}

/**
 * **A skill's Grades tab** (S6): see the module. `changed` is bumped when the project's skills
 * change, and a grade that ends reads it again.
 */
export function GradesTab({
  plane,
  name,
  level,
  changed,
  onGrading,
  onGraded,
  testsFile,
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  changed: number;
  onGrading: GradingGo;
  /** Told when a grade of this skill ends, so the page reads what it shows again. */
  onGraded?: () => void;
  /** Write trigger tests, where the page offers it; none says nothing more. */
  testsFile?: TestsFile;
}) {
  const [again, setAgain] = useState(0);
  const graded = useRef(onGraded);
  useEffect(() => {
    graded.current = onGraded;
  });
  const [said, setSaid] = useState<string>();
  const [writing, setWriting] = useState(false);
  // The grade the last read said runs: a step of another one reads the tab again.
  const runningNow = useRef<string | null>(null);
  // The level as a value: a page drawn again with an equal level reads nothing anew.
  const levelKey = JSON.stringify(level);
  const reread = [plane, name, levelKey, changed, again];
  // One read says the grades, whether there are trigger tests and the grade running (#3928).
  const grades = useRead(() => commands.skillGrades(plane, level, name), reread);
  const afterEdit = useRead(() => commands.skillGradeAfterEdit(plane, level, name), reread);

  // A grade of this skill that ends, or an edit grading after each edit did not grade, is read
  // again here: the app's word, never this tab's memory. A same-named copy at another level is
  // another skill, and a step of its moves nothing here (#3978).
  const ours = useRef(level);
  useEffect(() => {
    ours.current = level;
  });
  useEffect(() => {
    const isOurs = (now: { plane: string; skill: string; level: SkillLevel }) =>
      now.plane === plane && now.skill === name && sameLevel(now.level, ours.current);
    const ended = listen<SkillGradeStep>(SKILL_GRADE, (event) => {
      const now = event.payload;
      if (!isOurs(now)) return;
      // A grade that starts while the tab is open (one after an edit) is read, so it is shown.
      if (now.ended === null) {
        if (runningNow.current !== now.grade) setAgain((n) => n + 1);
        return;
      }
      setAgain((n) => n + 1);
      graded.current?.();
      if (now.said !== null) setSaid(endedSaid(now));
    }).catch(() => undefined);
    const waits = listen<SkillGradeAfterEditStep>(SKILL_GRADE_AFTER_EDIT, (event) => {
      const now = event.payload;
      if (!isOurs(now)) return;
      setSaid(now.said);
      setAgain((n) => n + 1);
    }).catch(() => undefined);
    return () => {
      void ended.then((stop) => stop?.()).catch(() => undefined);
      void waits.then((stop) => stop?.()).catch(() => undefined);
    };
  }, [plane, name]);

  // Each answer is taken only in its own shape: a core that answers another leaves it unread.
  const rows = Array.isArray(grades.data?.grades) ? grades.data.grades : [];
  const tests = typeof grades.data?.tests === "number" ? grades.data.tests : undefined;
  // A skill with outcome evals alone is graded too (SK-55).
  const evals = typeof grades.data?.evals === "number" ? grades.data.evals : 0;
  const noTests = tests === 0 && evals === 0 && grades.data?.tests_refused == null;
  const running = grades.data?.running?.grade ?? null;
  // Why trigger tests can't be written here, where they can't.
  const held =
    testsFile === undefined
      ? "This skill's files can't be opened here."
      : "reason" in testsFile
        ? testsFile.reason
        : undefined;
  useEffect(() => {
    runningNow.current = running;
  }, [running]);
  const on =
    typeof afterEdit.data === "object" && afterEdit.data !== null && "profile" in afterEdit.data
      ? afterEdit.data
      : null;
  return (
    <div className="skill-grades" data-testid="skill-grades">
      {said !== undefined && (
        <Notice
          cause="skill-grade"
          tone="trouble"
          label="Grading"
          onDismiss={() => setSaid(undefined)}
        >
          {said}
        </Notice>
      )}
      {running !== null && (
        <Notice
          cause={`skill-grade-running:${name}`}
          tone="news"
          label="A grade is running"
          fixes={[
            { label: "Show it", onPress: () => onGrading({ at: "running", grade: running }, "go") },
          ]}
        >
          <span data-testid="skill-grades-running">{`A grade of ${name} is running.`}</span>
        </Notice>
      )}
      {noTests && (
        <EmptyState
          variant="panel"
          headline={`${name} has no trigger tests yet`}
          body={`A grade has nothing to run until it has some. Trigger tests are prompts, each with whether ${name} should start for it, kept with the skill in ${TRIGGER_TESTS_FILE}; a grade runs each in chats with ${name} and without it, and counts the ones each chat got right.`}
          testid="skill-grades-no-tests"
          action={
            <ButtonRow>
              <Button
                variant="primary"
                isDisabled={held !== undefined || writing}
                tooltip={held}
                onPress={() => {
                  if (testsFile === undefined || !("open" in testsFile)) return;
                  const open = testsFile.open;
                  setWriting(true);
                  // The template is the core's, written as a change to the skill; its file then
                  // opens to write in. A refusal is said here, and opens nothing.
                  void commands
                    .skillTriggerTestsStart(plane, level, name)
                    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
                    .then((answer) => {
                      setWriting(false);
                      if (answer.status === "error") {
                        setSaid(answer.error);
                        return;
                      }
                      setAgain((n) => n + 1);
                      open();
                    });
                }}
              >
                Write trigger tests
              </Button>
              {/* A chat on the project's default profile, the core's prompt typed and not sent:
                  as Ask a chat to draft a skill opens one, and its write comes back as a draft. */}
              <Button
                isDisabled={held !== undefined}
                tooltip={held}
                onPress={() => askSkillChat({ plane, kind: "triggers", level, name })}
              >
                Ask a chat to write them
              </Button>
            </ButtonRow>
          }
        />
      )}
      {grades.refused !== undefined ? (
        <NotRead
          what={`the grades of ${name}`}
          why={grades.refused}
          onAgain={() => setAgain((n) => n + 1)}
          testid="skill-grades-not-read"
        />
      ) : grades.data === undefined ? (
        <ReadingRows what={`the grades of ${name}`} testid="skill-grading-reading" />
      ) : rows.length === 0 ? (
        !noTests && (
          <EmptyState
            variant="panel"
            headline={`No grades of ${name} yet`}
            body="Grade runs its trigger tests and outcome evals in chats with it and without it, once you have seen what it may spend."
            testid="skill-grades-empty"
            action={
              <Button onPress={() => onGrading({ at: "plan" }, "go")}>{`Grade ${name}…`}</Button>
            }
          />
        )
      ) : (
        <>
          <Latest name={name} row={rows[0]} />
          {grades.data.said !== null && (
            <p className="honest" role="note" data-testid="skill-grades-record">
              {grades.data.said}
            </p>
          )}
          <Table aria-label={`Grades of ${name}`} data-testid="skill-grades-list">
            <TableHeader>
              <TableColumn isRowHeader>Graded</TableColumn>
              <TableColumn>Version</TableColumn>
              <TableColumn>Ran on</TableColumn>
              <TableColumn align="end">With it</TableColumn>
              <TableColumn align="end">Without it</TableColumn>
              <TableColumn align="end">Previous</TableColumn>
              <TableColumn align="end">Outcome evals</TableColumn>
              <TableColumn align="end">Spent</TableColumn>
              <TableColumn>Ended</TableColumn>
              <TableColumn hideName>Scorecard</TableColumn>
            </TableHeader>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.grade} id={row.grade} textValue={row.graded}>
                  <TableCell>
                    {row.after_edit ? `${row.graded} · after an edit` : row.graded}
                  </TableCell>
                  <TableCell>{row.version}</TableCell>
                  <TableCell>{row.ran_on}</TableCell>
                  <TableCell align="end">{pointsSaid(row.arms.with)}</TableCell>
                  <TableCell align="end">{pointsSaid(row.arms.without)}</TableCell>
                  <TableCell align="end">
                    {row.arms.previous === null ? "—" : pointsSaid(row.arms.previous)}
                  </TableCell>
                  <TableCell align="end">
                    {row.outcome_arms === null ? "—" : pointsSaid(row.outcome_arms.with)}
                  </TableCell>
                  <TableCell align="end">{spentSaid(row)}</TableCell>
                  <TableCell textValue={row.ended}>
                    <Badge variant={OUTCOME_SAID[row.outcome].badge}>
                      {row.outcome === "graded" ? OUTCOME_SAID.graded.said : row.ended}
                    </Badge>
                  </TableCell>
                  <TableCell textValue="Scorecard">
                    {/* Its name starts with the words it shows, so a voice user's words find it. */}
                    <Button
                      size="sm"
                      aria-label={`${openSaid(row)} of the grade of ${row.graded}`}
                      onPress={() => onGrading({ at: "scorecard", grade: row.grade }, "go")}
                    >
                      {openSaid(row)}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </>
      )}
      <section
        aria-labelledby="skill-grades-outcome-evals"
        data-testid="skill-grades-outcome-evals"
      >
        <PanelSectionTitle id="skill-grades-outcome-evals">Outcome evals</PanelSectionTitle>
        <ButtonRow>
          <p>
            Tasks with statements a judge checks, kept with the skill. What a chat writes comes back
            as a draft for you to accept.
          </p>
          {/* A chat on the project's default profile, the core's prompt typed and not sent. */}
          <Button
            size="sm"
            isDisabled={held !== undefined}
            tooltip={held}
            onPress={() => askSkillChat({ plane, kind: "evals", level, name })}
          >
            Ask a chat to write outcome evals
          </Button>
        </ButtonRow>
      </section>
      <section aria-labelledby="skill-grades-after-edit" data-testid="skill-grades-after-edit">
        <PanelSectionTitle id="skill-grades-after-edit">Grade after each edit</PanelSectionTitle>
        <ButtonRow>
          <p data-testid="skill-grade-after-edit">
            {on === null
              ? "Off. When it's on, each edit you save is graded once, within a limit you accept first."
              : on.waits === null
                ? afterEditSaid(on)
                : `It waits for you: ${on.waits}`}
          </p>
          {on === null ? (
            <Button
              size="sm"
              isDisabled={noTests}
              tooltip={noTests ? `${name} has no trigger tests to grade yet.` : undefined}
              onPress={() => onGrading({ at: "plan", afterEdit: true }, "go")}
            >
              Turn on…
            </Button>
          ) : (
            <>
              {/* Waiting for a fresh yes: its new plan is read and accepted on its page. */}
              {on.waits !== null && (
                <Button size="sm" onPress={() => onGrading({ at: "plan", afterEdit: true }, "go")}>
                  Read the new plan…
                </Button>
              )}
              <Button
                size="sm"
                variant="ghost"
                onPress={() =>
                  void commands.skillGradeAfterEditOff(plane, level, name).then((answer) => {
                    if (answer.status === "ok") setAgain((n) => n + 1);
                    else setSaid(answer.error);
                  })
                }
              >
                Turn off
              </Button>
            </>
          )}
        </ButtonRow>
      </section>
    </div>
  );
}

/**
 * **A grading page under a skill** (S6): its plan, its grade running, a grade's scorecard or one
 * of that grade's chats, as the studio's place names it. `level` is the copy's where the studio
 * can grade it: a scorecard and a chat read by the grade alone and draw without it (a retired
 * tab comes back at the project's root), and say the level the grade was of (`onGradedAt`).
 * Each way back goes back where the step behind is the page it names, else says it in place
 * (#3978).
 */
export function GradingPage({
  plane,
  name,
  level,
  grading,
  changed,
  onGrading,
  onGradedAt,
}: {
  plane: string;
  name: string;
  level: SkillLevel | null;
  grading: GradingAt;
  changed: number;
  onGrading: GradingGo;
  onGradedAt?: (level: SkillLevel) => void;
}) {
  switch (grading.at) {
    case "plan":
      return level === null ? null : (
        <GradePlanPage
          plane={plane}
          name={name}
          level={level}
          changed={changed}
          afterEdit={grading.afterEdit === true}
          onStarted={(grade) => onGrading({ at: "running", grade }, "replace")}
          onAfterEdit={() => onGrading(undefined, "replace")}
          onRunning={(grade) => onGrading({ at: "running", grade }, "go")}
          onCancel={() => onGrading(undefined, "replace")}
        />
      );
    case "running":
      return level === null ? null : (
        <GradeRunningPage
          plane={plane}
          name={name}
          level={level}
          grade={grading.grade}
          onEnded={() => onGrading({ at: "scorecard", grade: grading.grade }, "replace")}
          // Leave it running, and its end's Back to its grades: the Grades tab, no new step.
          onBack={() => onGrading(undefined, "back")}
        />
      );
    case "scorecard":
      return (
        <GradeScorecardPage
          plane={plane}
          name={name}
          grade={grading.grade}
          onChat={(chat) => onGrading({ at: "chat", grade: grading.grade, chat }, "go")}
          onGradeAgain={level === null ? undefined : () => onGrading({ at: "plan" }, "go")}
          onGradedAt={onGradedAt}
        />
      );
    case "chat":
      return (
        <GradeChatPage
          plane={plane}
          name={name}
          grade={grading.grade}
          chat={grading.chat}
          onChat={(chat) => onGrading({ at: "chat", grade: grading.grade, chat }, "replace")}
          onScorecard={() => onGrading({ at: "scorecard", grade: grading.grade }, "back")}
          onGradedAt={onGradedAt}
        />
      );
  }
}
