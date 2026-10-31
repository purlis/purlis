// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Fragment, useEffect, useState } from "react";
import { LoaderCircle, Send } from "lucide-react";
import { ChatAsk } from "./ChatAsk";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Notice } from "./Notice";
import { PersonaMark } from "./PersonaMark";
import { Select } from "./ui/select/Select";
import { useTaskBranchActs, type BranchTold } from "./taskBranchActs";
import {
  commands,
  type DispatchRow,
  type Dispatches,
  type NotStartedRow,
  type PlaneId,
  type WorktreeLoss,
} from "./bindings";
import {
  askersOf,
  branchSaid,
  counted,
  discardSays,
  DISPATCH_WHENS,
  dispatchesIn,
  losesNothing,
  lostSaid,
  nestedSaid,
  ownSaid,
  notStartedSaid,
  NO_PERSONA,
  NO_PERSONA_SAID,
  NO_WORKSPACE,
  NO_WORKSPACE_SAID,
  personasOf,
  saidAt,
  shownDispatches,
  waitsOnMemory,
  workspacesOf,
  worktreeSaid,
  type DispatchFilter,
  type DispatchWhen,
} from "./dispatches";
import { useWhileShown } from "./whileShown";
import { Button } from "./ui/button/Button";
import { Twist } from "./ui/disclosure/Disclosure";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./ui/table/Table";

/** How often the list is read again while a dispatch in it is still running, and the window is
 *  shown. */
const WHILE_RUNNING_MS = 5000;

/**
 * **A project's dispatches, in a tab of their own** (#1452): the running ones and the past ones
 * this machine still keeps, newest first, narrowed by persona, by the chat that asked, by the
 * workspace it worked in and by when it started.
 *
 * **It is also the project's Past tasks** (#1510): a task whose chat has closed is listed with
 * its report as long as the store keeps its record, which is this machine's own and never a
 * file git carries. Opened from a workspace's strip, it starts narrowed to that workspace;
 * Every workspace widens it.
 *
 * A row's task is its one control. It opens the persona chat while that chat is still open, else
 * the session record the chat wrote, and is plain text when neither is there. The brief and the
 * report are a row's own words and can be long, so they are drawn under the row on a press and
 * not in the table, with what the report says changed where it said.
 *
 * A dispatch that has not ended and whose chat is not open (`not open`) is not running: it has
 * no duration, and the list is not read again on a timer for it.
 *
 * **Cost is what the chat's harness reported**, relayed by the chat's status line into the app's
 * own folder, where no sandboxed chat can write (#1457), so the column says *Cost (reported)*. A
 * row whose harness reports none says so in words, never as a zero.
 *
 * **A dispatch that was given a branch of its own lists it under *Where*** (#1453), with how
 * it stands. Nothing merges a task's branch for it, so it is listed until it is merged or the
 * person discards its folder. **Discard** asks first: the core reads what the folder holds, and
 * the question names every uncommitted file and every ignored path of the task's that would
 * go, and says what becomes of the branch. The branch purlis cut loses no commit by it: one
 * that holds work stays. Commits made in the folder on no branch are the one thing a discard
 * loses, and the question names them as lost. The paths the person was shown go back with the
 * answer, and the core removes nothing where the folder holds other paths by then. A refusal (a chat is still open in it) stands as a Notice.
 * **Merge…** beside it, for a task that has ended, asks the Changes tab's own question
 * (`taskBranchActs.tsx`, #1534): what the merge would do, read from the core now, and nothing
 * merged until it is answered; a merge the core would refuse is said instead.
 * **Review changes** beside it opens the task's Changes tab (#1534), with what its branch
 * changed and the person's Merge: the same tab its finished row in the chats list opens, so a
 * row cleared there still reaches it here. A branch whose folder is gone (discarded, removed by
 * other hands, merged while git kept it) is reached the same way, where the tab says what is
 * left of it and offers Delete branch where git finds it merged (#1472).
 *
 * The window says this of a branch and its folder, never of a worktree (ADR 0072 §4).
 */
export function DispatchesTab({
  plane,
  workspace,
  changed,
  onShowChat,
  onOpenRecord,
  onChanges,
}: {
  plane: PlaneId;
  /** The workspace whose strip the tab was opened on, which it starts narrowed to; `undefined`
   *  outside every workspace. */
  workspace?: string;
  /** Bumped when the project changes on disk: the tab reads again. */
  changed: number;
  /** Bring the chat in `session` to the front. */
  onShowChat: (session: number) => void;
  /** Open the session record at `path`, called `title`. */
  onOpenRecord: (path: string, title: string) => void;
  /** Open the Changes tab of the task of dispatch `id`, called `task`; none is offered
   *  without it. */
  onChanges?: (id: string, task: string) => void;
}) {
  const [said, setSaid] = useState<{ read?: Dispatches; trouble?: string }>();
  const [filter, setFilter] = useState<DispatchFilter>(() => dispatchesIn(workspace));
  const [read, setRead] = useState<string>();
  const [again, setAgain] = useState(0);
  /** The discard being asked about: the row, what the core says would be lost, and its refusal
   *  of the last answer. */
  const [discarding, setDiscarding] = useState<{
    row: DispatchRow;
    loss: WorktreeLoss;
    trouble?: string;
  }>();
  /** Why the core would not ask about a row's worktree at all. */
  const [kept, setKept] = useState<{ id: string; task: string; why: string }>();
  const [busy, setBusy] = useState(false);
  /** What the last Merge… came to: merged, or why not, in the core's words. */
  const [merged, setMerged] = useState<{ task: string } & BranchTold>();
  const branchActs = useTaskBranchActs({
    plane,
    told: (said) => setMerged((was) => (was === undefined ? undefined : { ...was, ...said })),
    changed: () => setAgain((was) => was + 1),
  });
  const askToMerge = (row: DispatchRow) => {
    setKept(undefined);
    setMerged({ task: row.task, tone: "news", says: "" });
    void branchActs.askToMerge(row.id);
  };
  const askToDiscard = async (row: DispatchRow) => {
    setKept(undefined);
    const answer = await commands
      .dispatchWorktreeLoss(plane, row.id)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    if (answer.status === "error") {
      // Verbatim: the sentence says what stands in the way and what to do about it.
      setKept({ id: row.id, task: row.task, why: answer.error });
      setAgain((was) => was + 1);
      return;
    }
    setDiscarding({ row, loss: answer.data });
  };
  const discard = async () => {
    if (discarding === undefined) return;
    setBusy(true);
    const answer = await commands
      .dispatchWorktreeDiscard(plane, discarding.row.id, discarding.loss)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setBusy(false);
    if (answer.status === "error") {
      setDiscarding({ ...discarding, trouble: answer.error });
      return;
    }
    setDiscarding(undefined);
    setAgain((was) => was + 1);
  };
  useEffect(() => {
    let gone = false;
    void commands
      .dispatches(plane)
      .then((answer) => {
        if (gone) return;
        setSaid(answer.status === "error" ? { trouble: answer.error } : { read: answer.data });
      })
      .catch((err: unknown) => {
        if (!gone) setSaid({ trouble: String(err) });
      });
    return () => {
      gone = true;
    };
  }, [plane, changed, again]);
  // A running dispatch's time, its needs-you count and its end are the app's own writes, which
  // no change in the project's files announces: read again while one is running. So is a
  // dispatch waiting on memory starting by itself (#1467).
  const running =
    (said?.read?.rows.some((row) => row.outcome === "running") ?? false) ||
    (said?.read?.not_started ?? []).some(waitsOnMemory);
  // On the window's one beat (`whileShown.ts`): not while the window is hidden, and once the
  // moment it is shown again, so what started or ended meanwhile is drawn at once.
  useWhileShown(WHILE_RUNNING_MS, () => setAgain((was) => was + 1), running);

  if (said === undefined) {
    return (
      <p className="pending" aria-busy="true">
        <LoaderCircle className="node-icon spinning" />
        Reading the dispatches…
      </p>
    );
  }
  if (said.trouble !== undefined) {
    return (
      <Notice
        cause="dispatches-unread"
        tone="trouble"
        fixes={[{ label: "Read again", onPress: () => setAgain((was) => was + 1) }]}
      >
        {`purlis could not read this project's dispatches: ${said.trouble}`}
      </Notice>
    );
  }
  const rows = said.read?.rows ?? [];
  const undrawn = said.read?.undrawn ?? 0;
  /* Counted and never shown: a record holding characters that draw as nothing or turn the
     words around them, or more text than purlis ever stores, is not one the app wrote. */
  const refused = undrawn > 0 && (
    <p className="none" data-testid="dispatches-undrawn">
      {undrawn === 1
        ? "1 record purlis will not draw: it holds text purlis refuses to put on the screen."
        : `${undrawn} records purlis will not draw: they hold text purlis refuses to put on the screen.`}
    </p>
  );
  const notStarted = said.read?.not_started ?? [];
  const waiting = notStarted.length > 0 && <NotStarted rows={notStarted} />;
  if (rows.length === 0) {
    if (refused)
      return (
        <>
          {waiting}
          {refused}
        </>
      );
    if (waiting) return waiting;
    return (
      <EmptyState
        mark={Send}
        headline="No dispatches yet"
        body="When a chat hands work to another chat, it is listed here with its brief and its report."
        testid="dispatches-empty"
      />
    );
  }
  const shown = shownDispatches(rows, filter);
  return (
    <>
      {waiting}
      <div className="vault-tools dispatches-tools">
        <Select
          className="search-scope"
          aria-label="Filter by persona"
          value={filter.persona}
          options={[
            { value: "", label: "Every persona" },
            ...personasOf(rows).map((persona) => ({
              value: persona,
              label: persona === NO_PERSONA ? NO_PERSONA_SAID : persona,
            })),
          ]}
          onValueChange={(to) => setFilter({ ...filter, persona: to })}
        />
        <Select
          className="search-scope"
          aria-label="Filter by asking chat"
          value={filter.asker}
          options={[
            { value: "", label: "Every asking chat" },
            ...askersOf(rows).map((asker) => ({ value: asker.key, label: asker.name })),
          ]}
          onValueChange={(to) => setFilter({ ...filter, asker: to })}
        />
        <Select
          className="search-scope"
          aria-label="Filter by workspace"
          value={filter.workspace}
          options={[
            { value: "", label: "Every workspace" },
            ...workspacesOf(rows, filter.workspace).map((place) => ({
              value: place,
              label: place === NO_WORKSPACE ? NO_WORKSPACE_SAID : place,
            })),
          ]}
          onValueChange={(to) => setFilter({ ...filter, workspace: to })}
        />
        <Select
          className="search-scope"
          aria-label="Filter by date"
          value={filter.when}
          options={DISPATCH_WHENS.map(({ when, said }) => ({ value: when, label: said }))}
          onValueChange={(to) => setFilter({ ...filter, when: to as DispatchWhen })}
        />
      </div>
      {shown.length === 0 ? (
        // Not the empty state: the project has dispatches, and the filters keep none of them.
        <p className="none">No dispatch matches these filters.</p>
      ) : (
        <Table aria-label="Dispatches" className="dispatches">
          <TableHeader>
            <TableColumn isRowHeader>Task</TableColumn>
            <TableColumn>Persona</TableColumn>
            <TableColumn>Asked by</TableColumn>
            <TableColumn>Where</TableColumn>
            <TableColumn>Outcome</TableColumn>
            <TableColumn>Duration</TableColumn>
            <TableColumn>Needed you</TableColumn>
            {/* What the chat's harness reported, not a figure purlis measured. */}
            <TableColumn>Cost (reported)</TableColumn>
          </TableHeader>
          <TableBody>
            {shown.map((row) => (
              <Fragment key={row.id}>
                <TableRow
                  id={row.id}
                  textValue={row.task}
                  data-testid={`dispatch-${row.id}`}
                  data-outcome={row.outcome}
                >
                  <TableCell>
                    <span className="dispatch-task">
                      <Button
                        size="sm"
                        variant="ghost"
                        aria-expanded={read === row.id}
                        aria-label={
                          read === row.id
                            ? `Hide the brief and report of ${row.task}`
                            : `Show the brief and report of ${row.task}`
                        }
                        onPress={() => setRead(read === row.id ? undefined : row.id)}
                        className="dispatch-more"
                      >
                        <Twist open={read === row.id} />
                      </Button>
                      <Task row={row} onShowChat={onShowChat} onOpenRecord={onOpenRecord} />
                    </span>
                  </TableCell>
                  <TableCell>
                    {row.persona === null ? (
                      NO_PERSONA_SAID
                    ) : (
                      <span className="dispatch-persona">
                        <PersonaMark persona={row.persona} />
                        {row.persona}
                      </span>
                    )}
                  </TableCell>
                  <TableCell>{row.by_person ? `you, from ${row.asker}` : row.asker}</TableCell>
                  <TableCell>
                    {/* The place's words carry its whole folder; the cell's controls are their
                        own Tab stops. */}
                    <Tooltip tip={row.folder ?? undefined}>
                      <span>{row.place}</span>
                    </Tooltip>
                    {row.worktree !== null && (
                      <span className="dispatch-worktree" data-standing={row.worktree.standing}>
                        {worktreeSaid(row.worktree)}
                        {row.worktree.standing !== "merged" && onChanges !== undefined && (
                          <Button
                            size="sm"
                            aria-label={`Review changes of ${row.task}`}
                            tooltip={
                              row.worktree.standing === "kept"
                                ? "Opens what its own branch changed, with Merge and Discard."
                                : "Opens what is left of its own branch, with Delete branch where it is merged."
                            }
                            onPress={() => onChanges(row.id, row.task)}
                            className="dispatch-changes"
                          >
                            Review changes
                          </Button>
                        )}
                        {/* Its folder is there and the task has ended: the Changes tab's
                            Merge, asked the same way (#1534). */}
                        {row.worktree.discard && row.ended !== null && (
                          <Button
                            size="sm"
                            aria-label={`Merge the branch of ${row.task}`}
                            onPress={() => askToMerge(row)}
                            className="dispatch-discard"
                          >
                            Merge…
                          </Button>
                        )}
                        {row.worktree.discard && (
                          <Button
                            size="sm"
                            aria-label={`Discard the branch folder of ${row.task}`}
                            onPress={() => void askToDiscard(row)}
                            className="dispatch-discard"
                          >
                            Discard
                          </Button>
                        )}
                      </span>
                    )}
                  </TableCell>
                  <TableCell>{row.outcome}</TableCell>
                  <TableCell>{row.duration}</TableCell>
                  <TableCell>{row.needed_you}</TableCell>
                  <TableCell>
                    <Tooltip tip={row.tokens ?? undefined}>
                      <span>{row.cost ?? "not reported"}</span>
                    </Tooltip>
                  </TableCell>
                </TableRow>
                {read === row.id && (
                  <TableRow
                    id={`${row.id}:read`}
                    textValue={`The brief and report of ${row.task}`}
                    className="dispatch-read"
                  >
                    <TableCell colSpan={8}>
                      <p className="dispatch-facts">
                        {[
                          row.mode === "task" ? "Task" : "Handoff",
                          `started ${saidAt(row.started)}`,
                          row.ended !== null
                            ? `ended ${saidAt(row.ended)}`
                            : row.outcome === "running"
                              ? "still running"
                              : "not ended, and its chat is not open",
                          `asked by ${row.asker}`,
                          row.asker_persona === null ? NO_PERSONA_SAID : `as ${row.asker_persona}`,
                          // The messages the two chats sent each other after the brief.
                          ...(row.messages === 0
                            ? []
                            : [row.messages === 1 ? "1 message" : `${row.messages} messages`]),
                        ].join(" · ")}
                      </p>
                      <h2>Brief</h2>
                      <pre>{row.brief}</pre>
                      <h2>Report</h2>
                      {row.report === null ? (
                        <p className="none">
                          {row.ended === null ? "Not reported yet." : "It ended with none."}
                        </p>
                      ) : (
                        <pre>{row.report}</pre>
                      )}
                      {/* The persona chat's own words for what it changed, as it reported
                          them: said as its claim, never as a fact purlis checked. */}
                      {row.changed !== null && (
                        <>
                          <h2>What it says changed</h2>
                          <pre>{row.changed}</pre>
                        </>
                      )}
                    </TableCell>
                  </TableRow>
                )}
              </Fragment>
            ))}
          </TableBody>
        </Table>
      )}
      {refused}
      {kept !== undefined && (
        <Notice
          cause={`dispatch-worktree-kept:${kept.id}`}
          tone="trouble"
          label={`The branch folder of ${kept.task} was not discarded`}
          onDismiss={() => setKept(undefined)}
        >
          {kept.why}
        </Notice>
      )}
      {merged !== undefined && merged.says !== "" && (
        <Notice
          cause={`dispatch-merged:${merged.task}`}
          tone={merged.tone}
          label={merged.tone === "trouble" ? "Nothing was merged" : "Merged"}
          onDismiss={() => setMerged(undefined)}
        >
          {merged.says}
        </Notice>
      )}
      {branchActs.asking}
      {discarding !== undefined && (
        <ChatAsk
          title="Discard this branch's folder?"
          says={discardSays(discarding.loss)}
          answer="Discard folder"
          trouble={discarding.trouble}
          busy={busy}
          onAnswer={() => void discard()}
          onCancel={() => setDiscarding(undefined)}
        >
          <Loss loss={discarding.loss} />
        </ChatAsk>
      )}
    </>
  );
}

/** The dispatches that never started, and so have no record (#1456): held on the person's
 *  answer, or kept blocked by them; waiting on this machine's memory, or given up on it
 *  (#1467). The app lists them from memory, and says so. */
function NotStarted({ rows }: { rows: NotStartedRow[] }) {
  return (
    <section className="dispatches-not-started" aria-label="Dispatches that did not start">
      <h2>Not started</h2>
      <ul>
        {rows.map((row, at) => (
          <li key={at} data-testid="dispatch-not-started" data-state={row.state}>
            {notStartedSaid(row)}
          </li>
        ))}
      </ul>
      <p className="none">
        Listed while the app runs: none of these is kept once it is started again.
      </p>
    </section>
  );
}

/** What goes with a discarded folder, as the core read it: every uncommitted file by its path,
 *  every ignored path of the task's own, the commits made on no branch (the one thing a discard
 *  loses for good), what git keeps for the folder alone (#1472), then what becomes of the
 *  branch. */
export function Loss({ loss }: { loss: WorktreeLoss }) {
  const lost = lostSaid(loss);
  const nested = nestedSaid(loss);
  const own = ownSaid(loss);
  return (
    <div className="dispatch-loss" data-testid="discard-loses">
      {losesNothing(loss) && (
        <p className="honest" data-testid="discard-loses-nothing">
          The folder holds no uncommitted file and no ignored one, so nothing in it would be lost.
        </p>
      )}
      {loss.changes.length > 0 && (
        <>
          <p className="honest">
            {`${counted(loss.changes.length, "uncommitted file", "uncommitted files")} would be lost:`}
          </p>
          <pre>{loss.changes.join("\n")}</pre>
        </>
      )}
      {nested !== undefined && (
        <>
          <p className="honest" data-testid="discard-loses-repositories">
            {nested}
          </p>
          <pre>{loss.nested.join("\n")}</pre>
        </>
      )}
      {loss.ignored.length > 0 && (
        <>
          <p className="honest">
            {`${counted(loss.ignored.length, "ignored path goes", "ignored paths go")} with it:`}
          </p>
          <pre>{loss.ignored.join("\n")}</pre>
        </>
      )}
      {lost !== undefined && (
        <>
          <p className="honest" data-testid="discard-loses-commits">
            {lost}
          </p>
          <pre>
            {loss.lost.join("\n")}
            {loss.unmerged > loss.lost.length
              ? `\n… and ${loss.unmerged - loss.lost.length} more`
              : ""}
          </pre>
        </>
      )}
      {own !== undefined && (
        <>
          <p className="honest" data-testid="discard-loses-own">
            {own}
          </p>
          {loss.own && loss.own.length > 0 && <pre>{loss.own.join("\n")}</pre>}
        </>
      )}
      <p className="honest" data-testid="discard-branch">
        {branchSaid(loss)}
      </p>
    </div>
  );
}

/** A row's task: what opens its chat, else its session record, else its name alone. */
function Task({
  row,
  onShowChat,
  onOpenRecord,
}: {
  row: DispatchRow;
  onShowChat: (session: number) => void;
  onOpenRecord: (path: string, title: string) => void;
}) {
  const { open_session: session, session_record: record } = row;
  if (session !== null) {
    return (
      <Button size="sm" variant="link" tooltip="Show its chat" onPress={() => onShowChat(session)}>
        {row.task}
      </Button>
    );
  }
  if (record !== null) {
    return (
      <Button
        size="sm"
        variant="link"
        tooltip="Open its session record"
        onPress={() => onOpenRecord(record, row.task)}
      >
        {row.task}
      </Button>
    );
  }
  return (
    <Tooltip tip="Its chat is closed and wrote no session record">
      <span className="dispatch-gone">{row.task}</span>
    </Tooltip>
  );
}
