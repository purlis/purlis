// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's Grades tab, and its grading pages** (#3688, the amendment's S6; extends decision
 * 43). Grading lives in the Skill studio's one tab, under the skill:
 *
 * - **The Grades tab** shows the skill's **checks** (ruling of 2026-11-02, `prototype/plain-checks`
 *   layout A): one heading, the latest grade in one sentence, and a card for each kind, **Does it
 *   get picked?** (its trigger tests) and **Does it do the job?** (its outcome evals), each with
 *   its technical name as a small label and its latest bars (with it, without it, its previous
 *   version). Under them every grade with how it ended (Graded, Stopped, Stopped at its limit,
 *   Interrupted), and the grade-after-each-edit row, which says what each edit may spend before
 *   it is turned on. A skill with no checks says what the two kinds are instead, and offers one
 *   chat to write both, back as the skill's one draft, which waits here to be accepted whole, or
 *   to write them yourself from a template (#3925).
 * - **Grade** opens the plan, a running grade shows its progress, and a grade's row opens its
 *   scorecard, from which each kept chat opens read-only: pages in the studio's history
 *   ({@link GradingAt}), with their own crumbs, never tabs of their own.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { listen, untilStopped } from "./here";
import {
  commands,
  SKILL_GRADE,
  SKILL_GRADE_AFTER_EDIT,
  type GradeRow,
  type SkillGradeAfterEditStep,
  type SkillDraft,
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
import { askAChat, sameLevel } from "./skillChanges";
import { RunningJob, useRunningJob } from "./studioChats";
import { askSkillChat } from "./skillDraftAsks";
import type { GradingAt } from "./skillStudio";
import { Badge } from "./ui/badge/Badge";
import { Card, CardTitle } from "./ui/card/Card";
import { counted } from "./dispatches";
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

/** A skill's outcome evals file, in its folder: skill-creator's, which a grade runs too. */
export const OUTCOME_EVALS_FILE = "evals/evals.json";

/** The two kinds of a skill's checks (ruling of 2026-11-02). */
type CheckKind = "picked" | "job";

/**
 * **What each kind of check is called** (ruling of 2026-11-02): its plain question first, and its
 * technical name and file as a small label under it, for the person who reads the files.
 */
export const CHECK_KINDS: Record<
  CheckKind,
  { question: string; technical: string; what: (name: string) => string }
> = {
  picked: {
    question: "Does it get picked?",
    technical: `trigger tests · ${TRIGGER_TESTS_FILE}`,
    what: (name) => `Things a person might type, each with whether ${name} should start for it.`,
  },
  job: {
    question: "Does it do the job?",
    technical: `outcome evals · ${OUTCOME_EVALS_FILE}`,
    what: () => "Tasks, each with what a good result has. Another chat checks each result.",
  },
};

/** How many checks of `kind` there are, as a card says it. */
function checksSaid(kind: CheckKind, count: number): string {
  return kind === "picked" ? counted(count, "prompt", "prompts") : counted(count, "task", "tasks");
}

/**
 * **The latest grade in one sentence** (the ruling's B, above the kinds): what each kind it
 * compared scored with the skill and without it. `undefined` where it compared neither.
 */
export function answerSaid(name: string, row: GradeRow): string | undefined {
  const picked =
    row.arms.with !== null && row.arms.without !== null
      ? { with: row.arms.with, without: row.arms.without }
      : null;
  const job =
    row.outcome_arms !== null && row.outcome_arms.with !== null && row.outcome_arms.without !== null
      ? { with: row.outcome_arms.with, without: row.outcome_arms.without }
      : null;
  if (picked !== null && job !== null)
    return `With ${name}, chats picked it right ${picked.with}% of the time and met ${job.with}% of what the job asks; without it, ${picked.without}% and ${job.without}%.`;
  if (picked !== null)
    return `With ${name}, chats picked it right ${picked.with}% of the time; without it, ${picked.without}%.`;
  if (job !== null)
    return `With ${name}, chats met ${job.with}% of what the job asks; without it, ${job.without}%.`;
  return undefined;
}

/** One kind's bars of a grade: with it, without it, and its previous version where it ran. */
function KindBars({
  kind,
  name,
  arms,
}: {
  kind: CheckKind;
  name: string;
  arms: { with: number | null; without: number | null; previous: number | null };
}) {
  return (
    <MeterGroup aria-label={`${CHECK_KINDS[kind].question} The latest grade of ${name}`}>
      <Meter label="With it" value={arms.with ?? 0} valueLabel={pointsSaid(arms.with)} />
      <Meter
        label="Without it"
        variant="baseline"
        value={arms.without ?? 0}
        valueLabel={pointsSaid(arms.without)}
      />
      {arms.previous !== null && (
        <Meter
          label="Previous version"
          variant="previous"
          value={arms.previous}
          valueLabel={pointsSaid(arms.previous)}
        />
      )}
    </MeterGroup>
  );
}

/** What a draft's checks file holds, as the draft's card lists it: each check's first line. */
function draftChecks(kind: CheckKind, text: string): string[] | undefined {
  try {
    const read: unknown = JSON.parse(text);
    const list = kind === "picked" ? read : (read as { evals?: unknown } | null)?.evals;
    if (!Array.isArray(list)) return undefined;
    // Each check has its words where a grade reads them, or the file is not checks it can run.
    const said = list.map(
      (one: unknown) =>
        (one as Record<string, unknown> | null)?.[kind === "picked" ? "query" : "prompt"],
    );
    return said.every((one) => typeof one === "string" && one !== "")
      ? (said as string[])
      : undefined;
  } catch {
    return undefined;
  }
}

/** **A draft that writes only a skill's checks**: every file of it under `evals/`. */
export function isChecksDraft(draft: SkillDraft): boolean {
  return draft.files.length > 0 && draft.files.every((one) => one.path.startsWith("evals/"));
}

/**
 * **The checks a chat wrote, waiting** (ruling of 2026-11-02): both kinds in the skill's one
 * draft, each kind's checks listed under its question, accepted or discarded whole.
 */
function ChecksDraft({
  plane,
  name,
  draft,
  onSaid,
  onDone,
}: {
  plane: string;
  name: string;
  draft: SkillDraft;
  onSaid: (said: string) => void;
  onDone: () => void;
}) {
  const [busy, setBusy] = useState(false);
  // Read once a draft: a tab drawn again (each step of a grade) parses nothing anew.
  const kinds = useMemo(
    () =>
      (["picked", "job"] as const).flatMap((kind) => {
        const path = kind === "picked" ? TRIGGER_TESTS_FILE : OUTCOME_EVALS_FILE;
        const file = draft.files.find((one) => one.path === path);
        if (file === undefined) return [];
        return [{ kind, path, checks: draftChecks(kind, file.after) }];
      }),
    [draft.files],
  );
  // A file a grade could not run is said, and Accept waits: accepted, it would only be refused.
  const unread = kinds.find((one) => one.checks === undefined);
  const answer = (act: () => Promise<{ status: "ok" } | { status: "error"; error: string }>) => {
    setBusy(true);
    void act()
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
      .then((answered) => {
        setBusy(false);
        if (answered.status === "error") onSaid(answered.error);
        else onDone();
      });
  };
  const by = draft.chat === null ? "A chat" : `Chat ${draft.chat}`;
  return (
    <Card
      as="section"
      variant="ask"
      aria-labelledby="skill-checks-draft"
      data-testid="skill-checks-draft"
    >
      <CardTitle as="h3" id="skill-checks-draft">
        {`${by} wrote checks for ${name}`}
      </CardTitle>
      {draft.stale && <Badge variant="waiting">Out of date</Badge>}
      <div className="skill-checks-kinds">
        {kinds.map(({ kind, path, checks }) => (
          <div key={kind} className="skill-checks-drafted">
            <p className="skill-checks-drafted-head">
              {checks === undefined
                ? `${CHECK_KINDS[kind].question} · ${path} can't be read as checks, so a grade couldn't run it.`
                : `${CHECK_KINDS[kind].question} · ${checksSaid(kind, checks.length)}`}
            </p>
            {checks !== undefined && (
              <ul className="skill-checks-list">
                {checks.map((one, at) => (
                  <li key={at}>{one}</li>
                ))}
              </ul>
            )}
          </div>
        ))}
      </div>
      <p className="honest">
        {draft.stale
          ? "A file it writes changed since, so it can't be accepted. Ask the chat to redo them."
          : "Nothing is kept until you accept it. Accept takes both kinds together."}
      </p>
      <ButtonRow>
        {!draft.stale && (
          <Button
            variant="primary"
            isDisabled={busy || unread !== undefined}
            tooltip={
              unread === undefined
                ? undefined
                : `${unread.path} can't be read as checks. Discard this draft and ask a chat again.`
            }
            onPress={() => answer(() => commands.skillDraftAccept(plane, draft.id))}
          >
            Accept checks
          </Button>
        )}
        {/* A redo is for a draft whose files changed under it, as on Drafts (S9). */}
        {draft.stale && (
          <Button
            isDisabled={busy}
            onPress={() =>
              askSkillChat({
                plane,
                kind: "redo",
                id: draft.id,
                name: draft.name,
                level: draft.level,
                chat: draft.chat_open ? draft.chat : null,
              })
            }
          >
            {draft.chat_open
              ? "Ask the chat to redo them"
              : askAChat(draft.level, "redo them", plane)}
          </Button>
        )}
        <Button
          variant="ghost"
          isDisabled={busy}
          onPress={() => answer(() => commands.skillDraftReject(plane, draft.id))}
        >
          Discard
        </Button>
      </ButtonRow>
    </Card>
  );
}

/**
 * **A skill's Grades tab** (S6; plain checks, ruling of 2026-11-02): see the module. `changed`
 * is bumped when the project's skills change, and a grade that ends reads it again.
 */
export function GradesTab({
  plane,
  name,
  level,
  changed,
  onGrading,
  onGraded,
  testsFile,
  evalsFile,
  draft,
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  changed: number;
  onGrading: GradingGo;
  /** Told when a grade of this skill ends, so the page reads what it shows again. */
  onGraded?: () => void;
  /** Its trigger tests' file, opened to write in, where the page offers it. */
  testsFile?: TestsFile;
  /** Its outcome evals' file, opened to edit, where the page offers it. */
  evalsFile?: TestsFile;
  /** A chat's draft of this skill that waits for review: shown here where it writes only checks. */
  draft?: SkillDraft;
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
    const ended = untilStopped(
      listen<SkillGradeStep>(SKILL_GRADE, (event) => {
        const now = event.payload;
        // A draft's grade is the draft's page's: it moves nothing on the skill's own tab (#2780).
        if (now.draft !== null || !isOurs(now)) return;
        // A grade that starts while the tab is open (one after an edit) is read, so it is shown.
        if (now.ended === null) {
          if (runningNow.current !== now.grade) setAgain((n) => n + 1);
          return;
        }
        setAgain((n) => n + 1);
        graded.current?.();
        if (now.said !== null) setSaid(endedSaid(now));
      }),
    );
    const waits = untilStopped(
      listen<SkillGradeAfterEditStep>(SKILL_GRADE_AFTER_EDIT, (event) => {
        const now = event.payload;
        if (!isOurs(now)) return;
        setSaid(now.said);
        setAgain((n) => n + 1);
      }),
    );
    return () => {
      ended();
      waits();
    };
  }, [plane, name]);

  // Each answer is taken only in its own shape: a core that answers another leaves it unread.
  const rows = Array.isArray(grades.data?.grades) ? grades.data.grades : [];
  // The job's columns are a grade's that ran outcome evals: a list with none draws without them.
  const anyJob = rows.some((row) => row.outcome_arms !== null);
  const tests = typeof grades.data?.tests === "number" ? grades.data.tests : undefined;
  // A skill with outcome evals alone is graded too (SK-55).
  const evals = typeof grades.data?.evals === "number" ? grades.data.evals : 0;
  const testsRefused = grades.data?.tests_refused ?? null;
  const noChecks = tests === 0 && evals === 0 && testsRefused === null;
  const running = grades.data?.running?.grade ?? null;
  const waiting = draft !== undefined && isChecksDraft(draft) ? draft : undefined;
  // **A studio chat writing its checks** (#5185, B's Grades card): the card shows the running job
  // in place of its button, until its draft waits.
  const drafted = draft === undefined ? [] : [draft];
  const writingTests = useRunningJob(name, level, ["trigger-tests", "checks"], drafted);
  const writingEvals = useRunningJob(name, level, ["outcome-evals", "checks"], drafted);
  const writingChecks = useRunningJob(
    name,
    level,
    ["checks", "trigger-tests", "outcome-evals"],
    drafted,
  );
  // Why a file can't be written or opened here, where it can't.
  const heldOf = (file: TestsFile | undefined) =>
    file === undefined
      ? "This skill's files can't be opened here."
      : "reason" in file
        ? file.reason
        : undefined;
  const held = heldOf(testsFile);
  useEffect(() => {
    runningNow.current = running;
  }, [running]);
  const on =
    typeof afterEdit.data === "object" && afterEdit.data !== null && "profile" in afterEdit.data
      ? afterEdit.data
      : null;
  const latest = rows[0];
  const answer = latest === undefined ? undefined : answerSaid(name, latest);

  // Write them yourself: the core's template for the trigger tests, written as a change to the
  // skill; its file then opens to write in. A refusal is said here, and opens nothing.
  const byHand = () => {
    if (testsFile === undefined || !("open" in testsFile)) return;
    const open = testsFile.open;
    setWriting(true);
    void commands
      .skillTriggerTestsStart(plane, level, name)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
      .then((answered) => {
        setWriting(false);
        if (answered.status === "error") {
          setSaid(answered.error);
          return;
        }
        setAgain((n) => n + 1);
        open();
      });
  };

  /** One kind's card: its question, its technical name, its bars and its way to add or edit. */
  const kindCard = (kind: CheckKind) => {
    const count = kind === "picked" ? (tests ?? 0) : evals;
    const any = count > 0 || (kind === "picked" && testsRefused !== null);
    const arms =
      latest === undefined
        ? null
        : kind === "picked"
          ? latest.arms.with === null && latest.arms.without === null
            ? null
            : latest.arms
          : latest.outcome_arms;
    const file = kind === "picked" ? testsFile : evalsFile;
    // Its own file says whether a chat's write to it would land in the skill.
    const fileHeld = heldOf(file);
    const titleId = `skill-checks-${kind}`;
    const job = kind === "picked" ? writingTests : writingEvals;
    return (
      <Card as="section" aria-labelledby={titleId} data-testid={titleId}>
        <div className="skill-checks-head">
          <CardTitle as="h3" id={titleId}>
            {CHECK_KINDS[kind].question}
          </CardTitle>
          <span className="skill-checks-technical">{CHECK_KINDS[kind].technical}</span>
        </div>
        {!any ? (
          <>
            <p className="honest">{`None yet. ${CHECK_KINDS[kind].what(name)}`}</p>
            {job !== undefined ? (
              <RunningJob plane={plane} chat={job} />
            ) : (
              <ButtonRow>
                <Button
                  size="sm"
                  isDisabled={fileHeld !== undefined}
                  tooltip={fileHeld}
                  onPress={() =>
                    askSkillChat({
                      plane,
                      kind: kind === "picked" ? "triggers" : "evals",
                      level,
                      name,
                    })
                  }
                >
                  {askAChat(level, "write these", plane)}
                </Button>
              </ButtonRow>
            )}
          </>
        ) : (
          <>
            <p className="honest">{CHECK_KINDS[kind].what(name)}</p>
            {kind === "picked" && testsRefused !== null ? (
              <p className="honest" role="note">
                {testsRefused}
              </p>
            ) : arms !== null ? (
              <KindBars kind={kind} name={name} arms={arms} />
            ) : (
              <p className="honest">
                {latest === undefined ? "Not graded yet." : "Not in the latest grade."}
              </p>
            )}
            <ButtonRow>
              {kind !== "picked" || testsRefused === null ? (
                <span className="skill-checks-count">{checksSaid(kind, count)}</span>
              ) : null}
              <Button
                size="sm"
                variant="link"
                isDisabled={fileHeld !== undefined}
                tooltip={fileHeld}
                onPress={() => {
                  if (file !== undefined && "open" in file) file.open();
                }}
              >
                Edit these checks
              </Button>
            </ButtonRow>
          </>
        )}
      </Card>
    );
  };

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
      {waiting !== undefined && (
        <ChecksDraft
          plane={plane}
          name={name}
          draft={waiting}
          onSaid={setSaid}
          onDone={() => setAgain((n) => n + 1)}
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
      ) : noChecks ? (
        waiting === undefined && (
          <EmptyState
            variant="panel"
            headline={`${name} has no checks yet`}
            body={
              <>
                {`Checks tell you whether ${name} helps. A grade runs each one in chats with ${name} and without it, and shows the difference.`}
                <ul className="skill-checks-kinds-said">
                  {(["picked", "job"] as const).map((kind) => (
                    <li key={kind}>
                      <strong>{CHECK_KINDS[kind].question}</strong> {CHECK_KINDS[kind].what(name)}
                    </li>
                  ))}
                </ul>
              </>
            }
            testid="skill-checks-none"
            action={
              writingChecks !== undefined ? (
                <RunningJob plane={plane} chat={writingChecks} />
              ) : (
                <ButtonRow>
                  {/* One chat on the project's default profile, the core's prompt typed and not
                    sent, writes both kinds; its write comes back as the skill's one draft. */}
                  <Button
                    variant="primary"
                    isDisabled={held !== undefined}
                    tooltip={held}
                    onPress={() => askSkillChat({ plane, kind: "checks", level, name })}
                  >
                    {askAChat(level, "write checks", plane)}
                  </Button>
                  <Button
                    variant="link"
                    isDisabled={held !== undefined || writing}
                    tooltip={held}
                    onPress={byHand}
                  >
                    Write them yourself
                  </Button>
                </ButtonRow>
              )
            }
          />
        )
      ) : (
        <>
          <section
            className="skill-checks"
            aria-labelledby="skill-checks-title"
            data-testid="skill-checks"
          >
            <div className="skill-checks-title">
              <PanelSectionTitle id="skill-checks-title">Checks</PanelSectionTitle>
              <ButtonRow>
                {latest !== undefined && (
                  <span className="honest" data-testid="skill-checks-latest">
                    {`Latest grade · ${latest.graded} · ${latest.ran_on}${latest.spent.cost_usd === null ? "" : ` · ${dollarsSaid(latest.spent.cost_usd)}`}`}
                  </span>
                )}
                <Button
                  size="sm"
                  variant={latest === undefined ? "primary" : "secondary"}
                  onPress={() => onGrading({ at: "plan" }, "go")}
                >
                  {latest === undefined ? `Grade ${name}…` : "Grade again…"}
                </Button>
              </ButtonRow>
            </div>
            {latest === undefined ? (
              <p className="honest">
                {`A grade runs every check in chats with ${name} and without it. You see what it may spend before it starts.`}
              </p>
            ) : answer !== undefined ? (
              <p className="skill-checks-answer" data-testid="skill-checks-answer">
                {answer}
              </p>
            ) : (
              <p className="honest">{`${OUTCOME_SAID[latest.outcome].said}: nothing of ${name} was compared.`}</p>
            )}
            <div className="skill-checks-kinds">
              {kindCard("picked")}
              {kindCard("job")}
            </div>
          </section>
        </>
      )}
      {/* Every grade, while it has any: its checks gone, its scorecards stay in reach. */}
      {grades.refused === undefined && grades.data !== undefined && rows.length > 0 && (
        <section aria-labelledby="skill-grades-title" className="skill-checks">
          <PanelSectionTitle id="skill-grades-title">Grades</PanelSectionTitle>
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
              <TableColumn align="end">Picked, with it</TableColumn>
              <TableColumn align="end">Picked, without it</TableColumn>
              <TableColumn align="end">Previous</TableColumn>
              {anyJob && <TableColumn align="end">Does the job, with it</TableColumn>}
              {anyJob && <TableColumn align="end">Does the job, without it</TableColumn>}
              <TableColumn align="end">Spent</TableColumn>
              <TableColumn>Ended</TableColumn>
              <TableColumn hideName>Scorecard</TableColumn>
            </TableHeader>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.grade} id={row.grade} textValue={row.graded}>
                  <TableCell>
                    {[
                      row.graded,
                      row.after_edit ? "after an edit" : null,
                      row.spent.resumed ? "resumed" : null,
                    ]
                      .filter((one) => one !== null)
                      .join(" · ")}
                  </TableCell>
                  <TableCell>{row.version}</TableCell>
                  <TableCell>{row.ran_on}</TableCell>
                  <TableCell align="end">{pointsSaid(row.arms.with)}</TableCell>
                  <TableCell align="end">{pointsSaid(row.arms.without)}</TableCell>
                  <TableCell align="end">
                    {row.arms.previous === null ? "—" : pointsSaid(row.arms.previous)}
                  </TableCell>
                  {anyJob && (
                    <TableCell align="end">
                      {row.outcome_arms === null ? "—" : pointsSaid(row.outcome_arms.with)}
                    </TableCell>
                  )}
                  {anyJob && (
                    <TableCell align="end">
                      {row.outcome_arms === null ? "—" : pointsSaid(row.outcome_arms.without)}
                    </TableCell>
                  )}
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
        </section>
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
              isDisabled={noChecks}
              tooltip={noChecks ? `${name} has no checks to grade yet.` : undefined}
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
                  void commands.skillGradeAfterEditOff(plane, level, name).then((answered) => {
                    if (answered.status === "ok") setAgain((n) => n + 1);
                    else setSaid(answered.error);
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
  draft,
}: {
  plane: string;
  name: string;
  level: SkillLevel | null;
  grading: GradingAt;
  changed: number;
  onGrading: GradingGo;
  onGradedAt?: (level: SkillLevel) => void;
  /** The draft of it these pages grade (#2780), by its id; `undefined` is the skill itself. */
  draft?: string;
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
          draft={draft}
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
          draft={draft}
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
          onResumed={(resumed) => onGrading({ at: "running", grade: resumed }, "go")}
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
