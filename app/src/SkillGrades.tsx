// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's Grades tab, and its grading pages** (#3688, the amendment's S6; extends decision
 * 43). Grading lives in the Skill studio's one tab, under the skill:
 *
 * - **The Grades tab** shows the latest grade as bars (with it, without it, its previous
 *   version), every grade with how it ended (Graded, Stopped, Stopped at its limit,
 *   Interrupted), and the grade-after-each-edit row, which says what each edit may spend before
 *   it is turned on. A skill with no trigger tests says what they are instead.
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
  Reading,
  afterEditSaid,
  dollarsSaid,
  endedSaid,
} from "./SkillGrade";
import { useRead } from "./SkillPanels";
import type { GradingAt } from "./skillStudio";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Meter, MeterGroup } from "./ui/meter/Meter";
import { PanelSectionTitle } from "./ui/panel/Panel";
import type { Crumb } from "./ui/panel/ViewBar";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./ui/table/Table";

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
      {row.arms.with === null && row.arms.without === null ? (
        <p className="honest">{`${OUTCOME_SAID[row.outcome].said}: nothing of ${name} was compared.`}</p>
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
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  changed: number;
  onGrading: GradingGo;
  /** Told when a grade of this skill ends, so the page reads what it shows again. */
  onGraded?: () => void;
}) {
  const [again, setAgain] = useState(0);
  const graded = useRef(onGraded);
  useEffect(() => {
    graded.current = onGraded;
  });
  const [said, setSaid] = useState<string>();
  // The level as a value: a page drawn again with an equal level reads nothing anew.
  const levelKey = JSON.stringify(level);
  const reread = [plane, name, levelKey, changed, again];
  const grades = useRead(() => commands.skillGrades(plane, level, name), reread);
  const plan = useRead(() => commands.skillGradePlan(plane, level, name, null, null), reread);
  const afterEdit = useRead(() => commands.skillGradeAfterEdit(plane, level, name), reread);

  // A grade of this skill that ends, or an edit grading after each edit did not grade, is read
  // again here: the app's word, never this tab's memory.
  useEffect(() => {
    const ended = listen<SkillGradeStep>(SKILL_GRADE, (event) => {
      const now = event.payload;
      if (now.plane === plane && now.skill === name && now.ended !== null) {
        setAgain((n) => n + 1);
        graded.current?.();
        if (now.said !== null) setSaid(endedSaid(now));
      }
    }).catch(() => undefined);
    const waits = listen<SkillGradeAfterEditStep>(SKILL_GRADE_AFTER_EDIT, (event) => {
      const now = event.payload;
      if (now.plane !== plane || now.skill !== name) return;
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
  const tests = Array.isArray(plan.data?.tests) ? plan.data.tests : undefined;
  const noTests = tests !== undefined && tests.length === 0 && plan.data?.tests_refused == null;
  const running = plan.data?.running ?? null;
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
          body={`A grade has nothing to run until it has some. Trigger tests are prompts, each with whether ${name} should start for it, kept with the skill in evals/triggers.json; a grade runs each in chats with ${name} and without it, and counts the ones each chat got right.`}
          testid="skill-grades-no-tests"
        />
      )}
      {grades.refused !== undefined ? (
        <EmptyState
          variant="panel"
          headline={`purlis could not read the grades of ${name}`}
          body={grades.refused}
          testid="skill-grades-not-read"
          action={<Button onPress={() => setAgain((n) => n + 1)}>Try again</Button>}
        />
      ) : grades.data === undefined ? (
        <Reading what={`the grades of ${name}`} />
      ) : rows.length === 0 ? (
        !noTests && (
          <EmptyState
            variant="panel"
            headline={`No grades of ${name} yet`}
            body="Grade runs its trigger tests in chats with it and without it, once you have seen what it may spend."
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
 * of that grade's chats, as the studio's place names it.
 */
export function GradingPage({
  plane,
  name,
  level,
  grading,
  changed,
  onGrading,
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  grading: GradingAt;
  changed: number;
  onGrading: GradingGo;
}) {
  switch (grading.at) {
    case "plan":
      return (
        <GradePlanPage
          plane={plane}
          name={name}
          level={level}
          changed={changed}
          afterEdit={grading.afterEdit === true}
          onStarted={(grade, plan) =>
            onGrading(
              {
                at: "running",
                grade,
                chats: plan.chats,
                ...(plan.cost_cap_usd === null ? {} : { limitUsd: plan.cost_cap_usd }),
              },
              "replace",
            )
          }
          onAfterEdit={() => onGrading(undefined, "replace")}
          onRunning={(grade) => onGrading({ at: "running", grade }, "go")}
          onCancel={() => onGrading(undefined, "replace")}
        />
      );
    case "running":
      return (
        <GradeRunningPage
          plane={plane}
          name={name}
          level={level}
          grade={grading.grade}
          started={grading}
          onEnded={() => onGrading({ at: "scorecard", grade: grading.grade }, "replace")}
          onBack={() => onGrading(undefined, "go")}
        />
      );
    case "scorecard":
      return (
        <GradeScorecardPage
          plane={plane}
          name={name}
          grade={grading.grade}
          onChat={(chat) => onGrading({ at: "chat", grade: grading.grade, chat }, "go")}
          onGradeAgain={() => onGrading({ at: "plan" }, "go")}
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
        />
      );
  }
}
