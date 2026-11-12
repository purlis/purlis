// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo, useContext, useState, type ReactNode } from "react";
import { SquareTerminal } from "lucide-react";
import type { FinishedTask } from "./bindings";
import type { Catalogued, Offer } from "./actions";
import { catalogueMenu } from "./Menus";
import { briefTitle, useOpenBrief } from "./Brief";
import { firstLine, foldedOf, qualifierOf, shownOf, whyNoReopen } from "./finished";
import { PersonaMark } from "./PersonaMark";
import { StateShown } from "./StateShown";
import { useTokensOnHover } from "./tasksUsed";
import { WaitingTaskWaysContext } from "./waitingTasks";
import { Button } from "./ui/button/Button";
import { Menu, MenuItem } from "./ui/menu/Menu";
import { RowBelow, RowLabel, RowLead, RowTrail } from "./ui/list/Row";
import { TreeRow } from "./ui/tree/Tree";

/**
 * **A chat's finished tasks, rows under its row in the Chats tree** (#1485, #3198).
 *
 * A task ends at its report, and its row stays here as a finished entry. The ones that came
 * out done, and the ones the asking chat cancelled, fold into one row, **Finished (n)**, whose
 * rows they are, with **Clear finished** at its end and in its menu. Every other end is a row of
 * its own until it is cleared, so a failure is never behind a count.
 *
 * **A task that did not start is one of those rows** (#1497): failed, with the reason shown
 * under it at once, as text. It is never a banner across the window. One the chat that asked
 * tried more than once is one row, which says how often.
 *
 * **A task a launch could not start again is drawn here too, and is not ended** (`waits`): it
 * is still recorded and is tried again at the next launch. Its row offers Try to start again,
 * Review and approve… where its profile waits on that, and End task, which is the only thing
 * that ends it. It has no Reopen and no Clear.
 *
 * **A finished task cannot be typed into**: its program has ended. Pressing its row, or Enter on
 * it, shows its report under its line, **as text**: every word of it is a text node, so nothing
 * a task wrote is ever read as markup; and the report is then the row's description. **Brief**
 * shows what it was sent, from the same record (#1494). **Reopen** resumes its conversation as
 * an ordinary chat with a tab; it is then no longer a task, and the chat that asked is told
 * nothing.
 *
 * **Changes** opens what the task changed, and no other task's, in a tab of its own (#1511,
 * V100-66); **Review changes** for a task that worked on its own branch, whose tab offers the
 * person's Merge and Discard. The report's line about what changed opens the same tab.
 *
 * **A row has a menu** (#1534): for a task on its own branch, the catalogue's Merge… and, under
 * the line, Discard branch…, which ask the Changes tab's own questions.
 *
 * **Rows of the tree** (#3198): the arrows reach them as they reach the chats, and → goes on from
 * a row into its buttons, as a row's controls are reached in the tree family's grid.
 */
export function finishedRows({
  session,
  tasks,
  onClear,
  onReopen,
  onLook,
  onChanges,
  offers,
  onPress,
}: {
  /** The chat that asked for them, by its number: what their "Finished (n)" row is keyed by. */
  session: number;
  tasks: readonly FinishedTask[];
  /** Takes these rows away, and forgets what their tasks said (#1520): their records stay. */
  onClear: (ids: string[]) => void;
  /** Reopens one as an ordinary chat; answers why not, where it could not. */
  onReopen: (task: FinishedTask) => Promise<string | undefined>;
  /** A row was opened to read its report: a task that failed has then been looked at, and
   *  its needs-you item goes (#1491). */
  onLook?: (task: FinishedTask) => void;
  /** Opens what a task changed, in a tab of its own; no Changes is offered without it. */
  onChanges?: (task: FinishedTask) => void;
  /** The catalogue a row's menu is drawn from (#1534); no menu without it. */
  offers?: Catalogued;
  onPress?: (offer: Offer) => void;
}): ReactNode[] {
  if (tasks.length === 0) return [];
  const { alone, folded } = foldedOnce(tasks);
  return [
    ...alone.map((task) => (
      <FinishedRow
        key={task.id}
        task={task}
        onClear={onClear}
        onReopen={onReopen}
        onLook={onLook}
        onChanges={onChanges}
        offers={offers}
        onPress={onPress}
      />
    )),
    ...(folded.length === 0
      ? []
      : [
          <FinishedFold
            key={finishedLineId(session)}
            session={session}
            folded={folded}
            onClear={onClear}
            onReopen={onReopen}
            onChanges={onChanges}
            offers={offers}
            onPress={onPress}
          />,
        ]),
  ];
}

/** Each chat's tasks as they fold, worked out once for each list of them: the rows drawn for it
 *  are then the same rows from one draw of the Chats list to the next (SC-3). */
const FOLDS = new WeakMap<readonly FinishedTask[], ReturnType<typeof foldedOf>>();

function foldedOnce(tasks: readonly FinishedTask[]): ReturnType<typeof foldedOf> {
  const known = FOLDS.get(tasks);
  if (known !== undefined) return known;
  const worked = foldedOf(tasks);
  FOLDS.set(tasks, worked);
  return worked;
}

/**
 * **The "Finished (n)" row** (#1485, #3198): the done and cancelled tasks of one chat, as its
 * rows, folded until it is opened (`finishedLineId`, which the tree says is open), with Clear
 * finished at its end and in its menu.
 */
const FinishedFold = memo(function FinishedFold({
  session,
  folded,
  onClear,
  onReopen,
  onChanges,
  offers,
  onPress,
}: {
  session: number;
  folded: readonly FinishedTask[];
  onClear: (ids: string[]) => void;
  onReopen: (task: FinishedTask) => Promise<string | undefined>;
  onChanges?: (task: FinishedTask) => void;
  offers?: Catalogued;
  onPress?: (offer: Offer) => void;
}) {
  const clear = () => onClear(folded.map((task) => task.id));
  return (
    <TreeRow
      id={finishedLineId(session)}
      textValue={`Finished (${folded.length})`}
      className="finished-count"
      // Clear finished, in its menu as at its end.
      menu={() => (
        <Menu aria-label="Finished tasks" onAction={clear}>
          <MenuItem id="clear" textValue="Clear finished">
            Clear finished
          </MenuItem>
        </Menu>
      )}
      content={
        <>
          <RowLabel className="session ui-text-secondary">Finished ({folded.length})</RowLabel>
          <RowTrail>
            <Button
              size="sm"
              excludeFromTabOrder
              tooltip="Takes these rows away, and forgets what these tasks and this chat said to each other. Each brief and report stays on its dispatch record."
              onPress={clear}
              className="finished-clear"
            >
              Clear finished
            </Button>
          </RowTrail>
        </>
      }
    >
      {folded.map((task) => (
        <FinishedRow
          key={task.id}
          task={task}
          onReopen={onReopen}
          onChanges={onChanges}
          offers={offers}
          onPress={onPress}
        />
      ))}
    </TreeRow>
  );
});

/** The key of chat `session`'s "Finished (n)" row: what the tree says is open by. */
export function finishedLineId(session: number): string {
  return `${FINISHED_LINE}${session}`;
}

/** Whether `key` is a "Finished (n)" row's ({@link finishedLineId}). */
export function isFinishedLineId(key: unknown): key is string {
  return typeof key === "string" && key.startsWith(FINISHED_LINE);
}

const FINISHED_LINE = "finished:";

/** One finished task: its name and how it ended, its report on a press, and Reopen. A row
 *  that stands alone has a Clear of its own; a folded one is cleared with its fold. */
function FinishedRow({
  task,
  onClear,
  onReopen,
  onLook,
  onChanges,
  offers,
  onPress,
}: {
  task: FinishedTask;
  onClear?: (ids: string[]) => void;
  onReopen: (task: FinishedTask) => Promise<string | undefined>;
  onLook?: (task: FinishedTask) => void;
  onChanges?: (task: FinishedTask) => void;
  offers?: Catalogued;
  onPress?: (offer: Offer) => void;
}) {
  // A task that did not start says why at once (#1497): its report is purlis's one sentence,
  // and the reason is the whole of what there is to know about it.
  const [shown, setShown] = useState(task.did_not_start);
  const [refused, setRefused] = useState<string>();
  const [busy, setBusy] = useState(false);
  /** Whether End task was pressed once and waits for the second press. */
  const [ending, setEnding] = useState(false);
  const ways = useContext(WaitingTaskWaysContext);
  const waits = task.waits;
  const state = shownOf(task);
  const more = qualifierOf(task);
  /** Opens the brief it was sent (`Brief.tsx`), where the window around this row has one. */
  const openBrief = useOpenBrief();
  /** Why Reopen does nothing, where it does nothing: said to a screen reader on the button,
   *  which stays in the Tab order, and in the opened row. A disabled button takes no focus,
   *  so its reason would reach nobody on a keyboard. */
  const cannot = whyNoReopen(task);
  // What it used, kept when it ended (#1500): on its name's hover, read as the pointer comes
  // on. A task still waiting to start has no record to read.
  const used = useTokensOnHover({ finished: task.id });
  /** How it ended, whole: the row draws only its mark (#1687). */
  const ended = [
    state?.word,
    more,
    task.attempts > 1 ? `tried ${task.attempts} times` : undefined,
  ].filter((one) => one !== undefined);
  const hover = [ended.join(" · "), firstLine(task.report), waits === null ? (used.said ?? "") : ""]
    .filter((one) => one !== "")
    .join("\n");
  const doing = (what: Promise<string | undefined>) => {
    setBusy(true);
    setRefused(undefined);
    void what.then(setRefused).finally(() => {
      setBusy(false);
      setEnding(false);
    });
  };
  const reopen = () => {
    if (cannot !== undefined || busy) return;
    setBusy(true);
    setRefused(undefined);
    void onReopen(task)
      .then(setRefused)
      .finally(() => setBusy(false));
  };
  /** Whether anything stands under its line: the report, a refusal, the core's note. */
  const noted = task.reopens && task.not_reopened !== null;
  return (
    <TreeRow
      id={`task:${task.id}`}
      textValue={task.name}
      className="finished-task"
      data-how={task.how}
      data-task-id={task.id}
      // How it ended, and while it is shown, its report: what a reader hears after its name.
      description={[ended.join(", "), shown ? task.report : ""]
        .filter((one) => one !== "")
        .join(". ")}
      // The row's one hint: its whole name, cut short in a narrow sidebar (#1499), then how it
      // ended, its report's first line and what it used.
      tip={[task.name, hover].filter((one) => one !== "").join("\n")}
      onHoverStart={waits === null ? used.onPointerEnter : undefined}
      onHoverEnd={waits === null ? used.onPointerLeave : undefined}
      // Its menu (#1534): the catalogue's rows for this task, where it has any.
      menu={
        offers === undefined || onPress === undefined
          ? undefined
          : catalogueMenu({ on: "finished", id: task.id }, offers, onPress)
      }
      onAction={() => {
        // Opened to be read: looked at.
        if (!shown) onLook?.(task);
        setShown((was) => !was);
      }}
      content={
        <>
          {/* How it ended, as a chat's row says a state (#1484): **its mark** (#1687), the
              words the row's description and whole in its tooltip. */}
          <RowLead>{state !== undefined && <StateShown shown={state} markOnly />}</RowLead>
          <RowLabel className="session ui-text-secondary">{task.name}</RowLabel>
          <RowTrail>
            {/* Its persona's mark at the line's end, as a chat's row has it (DS-11). */}
            {task.persona === null ? (
              <SquareTerminal className="node-icon" aria-hidden="true" />
            ) : (
              <PersonaMark persona={task.persona} />
            )}
            {openBrief !== undefined && (
              <Button
                size="sm"
                excludeFromTabOrder
                aria-label={briefTitle(task.name)}
                aria-haspopup="dialog"
                tooltip="What this task was sent, as it was sent."
                onPress={() => openBrief({ dispatch: task.id, name: task.name })}
                className="finished-brief"
              >
                Brief
              </Button>
            )}
            {/* Still a task (`waits`): the ways out a chat that did not start has, and never
                Reopen or Clear, which are for one that has ended. */}
            {waits !== null && ways !== null && (
              <>
                <Button
                  size="sm"
                  excludeFromTabOrder
                  isDisabled={busy}
                  aria-label={`Try to start ${task.name} again`}
                  tooltip="Starts every chat of this project that did not come back, this one among them. It stays a task of the chat that asked."
                  onPress={() => doing(ways.retry(task).then(() => undefined))}
                  className="finished-reopen"
                >
                  Try to start again
                </Button>
                {waits.approval !== null && (
                  <Button
                    size="sm"
                    excludeFromTabOrder
                    isDisabled={busy}
                    aria-label={`Review and approve what ${task.name} would run`}
                    onPress={() => waits.approval !== null && ways.approve(task, waits.approval)}
                    className="finished-reopen"
                  >
                    Review and approve…
                  </Button>
                )}
                {ending ? (
                  <>
                    <Button
                      size="sm"
                      excludeFromTabOrder
                      isDisabled={busy}
                      aria-label={`End ${task.name} now`}
                      onPress={() => doing(ways.end(task))}
                      className="finished-clear"
                    >
                      End it
                    </Button>
                    <Button
                      size="sm"
                      excludeFromTabOrder
                      isDisabled={busy}
                      aria-label={`Keep ${task.name}`}
                      onPress={() => setEnding(false)}
                      className="finished-clear"
                    >
                      Keep
                    </Button>
                  </>
                ) : (
                  <Button
                    size="sm"
                    excludeFromTabOrder
                    isDisabled={busy}
                    aria-label={`End task ${task.name}`}
                    tooltip="Ends the task: the chat that asked is told it failed and why, and it is not tried again. Its conversation can still be reopened as an ordinary chat."
                    onPress={() => setEnding(true)}
                    className="finished-clear"
                  >
                    End task
                  </Button>
                )}
              </>
            )}
            {/* Reopen and Clear are for a task that has ended. */}
            {waits === null && (
              <Button
                size="sm"
                excludeFromTabOrder
                aria-disabled={cannot !== undefined || busy || undefined}
                aria-label={`Reopen ${task.name}`}
                aria-description={cannot}
                tooltip={
                  cannot ??
                  "Resumes its conversation as an ordinary chat with a tab. It is no longer a task: it sends no report, and the chat that asked is not told."
                }
                onPress={reopen}
                className="finished-reopen"
              >
                Reopen
              </Button>
            )}
            {waits === null && onChanges !== undefined && (
              <Button
                size="sm"
                excludeFromTabOrder
                aria-label={`${task.branch === null ? "Changes" : "Review changes"} of ${task.name}`}
                tooltip={
                  task.branch === null
                    ? "Opens the files this task's edit tools wrote that are still uncommitted. Edits made by a shell command, and what it committed, are not listed."
                    : "Opens what its own branch changed, with Merge and Discard."
                }
                onPress={() => onChanges(task)}
                className="finished-changes"
              >
                {task.branch === null ? "Changes" : "Review changes"}
              </Button>
            )}
            {waits === null && onClear !== undefined && (
              <Button
                size="sm"
                excludeFromTabOrder
                aria-label={`Clear ${task.name}`}
                tooltip="Takes this row away, and forgets what this task and its chat said to each other. Its brief and report stay on its dispatch record."
                onPress={() => onClear([task.id])}
                className="finished-clear"
              >
                Clear
              </Button>
            )}
          </RowTrail>
          {(refused !== undefined || noted || shown) && (
            <RowBelow className="finished-below">
              {refused !== undefined && (
                <span className="trouble" role="alert">
                  {refused}
                </span>
              )}
              {/* The last Reopen started a chat that ended at once: the core's sentence, kept
                  on the row until the next try. */}
              {noted && (
                <span className="finished-note ui-text-secondary">{task.not_reopened}</span>
              )}
              {shown && (
                <span
                  className="finished-report ui-text-body"
                  role="region"
                  aria-label={`Report of ${task.name}`}
                >
                  {/* Text nodes, every one: a report is a chat's words, and is never markup. */}
                  <span className="report-text">{task.report}</span>
                  {/* The task's own words for what it changed, as text; the line opens what
                      purlis found it changed (#1511). */}
                  {task.changed !== null &&
                    (onChanges === undefined ? (
                      <span className="report-text report-changed">Changed: {task.changed}</span>
                    ) : (
                      <span className="report-text report-changed">
                        <Button
                          size="sm"
                          variant="ghost"
                          excludeFromTabOrder
                          tooltip="Opens what purlis can tell this task changed"
                          onPress={() => onChanges(task)}
                          className="report-changed-link"
                        >
                          Changed:
                        </Button>{" "}
                        {task.changed}
                      </span>
                    ))}
                  {waits !== null && (
                    <span className="report-text">
                      It is still recorded, and will be tried again at the next launch.
                    </span>
                  )}
                  <span className="report-where ui-text-secondary">
                    {task.place}
                    {task.branch !== null && ` · own branch ${task.branch}`}
                  </span>
                  {cannot !== undefined && (
                    <span className="report-where ui-text-secondary">{cannot}</span>
                  )}
                </span>
              )}
            </RowBelow>
          )}
        </>
      }
    />
  );
}
