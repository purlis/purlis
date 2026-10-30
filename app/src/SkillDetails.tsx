// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState, type ReactNode } from "react";
import { LoaderCircle } from "lucide-react";
import {
  commands,
  type SkillCopy,
  type SkillDetails,
  type SkillDiff,
  type SkillLifecycle,
  type SkillVersion,
  type SkillVersionBefore,
  type StatsRow,
} from "./bindings";
import { counted, saidAt } from "./dispatches";
import { Button } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./ui/table/Table";

/** A version's newest grade, as its row says it (SK-16): the trigger tests that passed with it. */
export function gradeSaid(version: SkillVersion): string {
  const grade = version.grade;
  if (grade === null) return "Not graded";
  return `${grade.passed} of ${counted(grade.fixtures, "trigger test", "trigger tests")} passed`;
}

/**
 * What a revert's confirm asks (#4017): a version from before a rename or a move comes back in the
 * folder the skill has now, and one from before a rename under the name it has now.
 */
export function revertSaid(name: string, before: SkillVersionBefore | null): string {
  const saved = "The revert is saved as a new version.";
  if (before === null) return `Put the skill back as this version had it? ${saved}`;
  if (before.name !== name) {
    return `This version is from when the skill was named ${before.name}, in ${before.folder}. Put its files back in the folder ${name} has now, with its name: line changed to ${name}? ${saved}`;
  }
  return `This version is from before the skill was moved, when it was in ${before.folder}. Put its files back in the folder it has now? ${saved}`;
}

/** A copy's state, as its tab says it in one line. */
export function lifecycleSaid(name: string, lifecycle: SkillLifecycle): string {
  switch (lifecycle.state) {
    case "active":
      return "In use.";
    case "deprecated":
      return lifecycle.replacement === null
        ? `Deprecated: ${name} still works, and is listed after the other skills.`
        : `Deprecated: ${name} still works, and chats are pointed to ${lifecycle.replacement} instead.`;
    case "archived":
      return `Archived: ${name} never loads until it is restored.`;
  }
}

/** What a line of a diff is: added, removed, or neither (its headers and context). */
export function diffLineKind(line: string): "added" | "removed" | undefined {
  if (line.startsWith("+") && !line.startsWith("+++")) return "added";
  if (line.startsWith("-") && !line.startsWith("---")) return "removed";
  return undefined;
}

/** What a copy's state could not be read says, in the window's words. */
export const UNREAD =
  "purlis could not read what it keeps about this skill, so it is taken as in use with no owners. Deprecating or archiving it writes it again.";

/** What runs add up to, as one cell says them: signals, never a verdict. */
export function runsSaid(runs: StatsRow): string {
  if (runs.runs === 0) return "No runs";
  return [
    counted(runs.runs, "run", "runs"),
    counted(runs.tool_calls, "tool call", "tool calls"),
    counted(runs.tool_errors, "error", "errors"),
  ].join(", ");
}

/**
 * **A copy's versions** (SK-19, #2782), newest first, each with its runs, its grade, its changes
 * and a revert, as a skill's History tab shows them (#3682). `waiting` is drawn first: a change a
 * chat made that waits for the person's review.
 */
export function SkillHistory({
  plane,
  name,
  copy,
  busy,
  waiting,
  onRevert,
}: {
  plane: string;
  name: string;
  copy: SkillCopy;
  busy: boolean;
  /** A change waiting for the person's review, with its way to review it. */
  waiting?: ReactNode;
  onRevert: (commit: string) => void;
}) {
  const [open, setOpen] = useState<string>();
  const [diff, setDiff] = useState<SkillDiff | string>();
  // The version a revert is asked for, and what it would bring back that widens a chat: null
  // where nothing widens, undefined while it is read.
  const [asked, setAsked] = useState<{ commit: string; widens?: string | null }>();
  const ask = (commit: string) => {
    setAsked({ commit });
    void commands.skillRevertWidens(plane, copy.level, name, commit).then((answer) => {
      setAsked({ commit, widens: answer.status === "ok" ? answer.data : null });
    });
  };
  const show = (commit: string) => {
    if (open === commit) {
      setOpen(undefined);
      return;
    }
    setOpen(commit);
    setDiff(undefined);
    void commands.skillDiff(plane, copy.level, name, commit).then((answer) => {
      setDiff(answer.status === "ok" ? answer.data : answer.error);
    });
  };
  return (
    <section aria-label={`History of ${name}`} data-testid="skill-history">
      {waiting}
      {copy.versions.length === 0 ? (
        <EmptyState
          headline={`${name} has no history yet`}
          body="Each saved change of a skill is a version of it. A skill kept on this machine alone has none."
          testid="skill-history-empty"
        />
      ) : (
        <Table aria-label={`History of ${name}`}>
          <TableHeader>
            <TableColumn isRowHeader>Saved</TableColumn>
            <TableColumn>By</TableColumn>
            <TableColumn>Change</TableColumn>
            <TableColumn>Runs on it</TableColumn>
            <TableColumn>Grade</TableColumn>
            <TableColumn hideName>Actions</TableColumn>
          </TableHeader>
          <TableBody>
            {copy.versions.map((version, at) => (
              <VersionRow
                key={version.commit}
                name={name}
                version={version}
                newest={at === 0}
                open={open === version.commit}
                diff={open === version.commit ? diff : undefined}
                busy={busy}
                onShow={() => show(version.commit)}
                asked={asked?.commit === version.commit ? asked : undefined}
                onAsk={() => ask(version.commit)}
                onRevert={() => {
                  setAsked(undefined);
                  onRevert(version.commit);
                }}
                onCancel={() => setAsked(undefined)}
              />
            ))}
          </TableBody>
        </Table>
      )}
      {copy.uncommitted.runs > 0 && (
        <p className="honest" data-testid="skill-uncommitted-runs">
          On changes not committed yet: {runsSaid(copy.uncommitted)}.
        </p>
      )}
    </section>
  );
}

function VersionRow({
  name,
  version,
  newest,
  open,
  diff,
  busy,
  onShow,
  asked,
  onAsk,
  onRevert,
  onCancel,
}: {
  name: string;
  version: SkillVersion;
  newest: boolean;
  open: boolean;
  diff: SkillDiff | string | undefined;
  busy: boolean;
  onShow: () => void;
  asked: { widens?: string | null } | undefined;
  onAsk: () => void;
  onRevert: () => void;
  onCancel: () => void;
}) {
  return (
    <>
      <TableRow
        id={version.commit}
        textValue={saidAt(version.at)}
        data-testid={`skill-version-${version.commit}`}
      >
        <TableCell>{saidAt(version.at)}</TableCell>
        <TableCell>{version.author}</TableCell>
        <TableCell>{version.subject}</TableCell>
        <TableCell>{runsSaid(version.runs)}</TableCell>
        <TableCell>{gradeSaid(version)}</TableCell>
        <TableCell>
          <Button size="sm" variant="ghost" aria-expanded={open} onPress={onShow}>
            {open ? "Hide changes" : "Show changes"}
          </Button>
          {!newest && (
            <Button size="sm" isDisabled={busy || asked !== undefined} onPress={onAsk}>
              Revert to this
            </Button>
          )}
        </TableCell>
      </TableRow>
      {asked !== undefined && (
        <TableRow
          id={`${version.commit}:revert`}
          textValue="Revert to this version"
          data-testid="skill-revert-confirm"
        >
          <TableCell colSpan={6}>
            <p>{revertSaid(name, version.before)}</p>
            {asked.widens === undefined ? (
              <p className="pending" aria-busy="true">
                <LoaderCircle className="node-icon spinning" />
                Reading the version…
              </p>
            ) : (
              asked.widens !== null && (
                <p className="honest" data-testid="skill-revert-widens">
                  {asked.widens}
                </p>
              )
            )}
            <Button
              size="sm"
              variant="primary"
              isDisabled={busy || asked.widens === undefined}
              onPress={onRevert}
            >
              Revert
            </Button>
            <Button size="sm" variant="ghost" onPress={onCancel}>
              Cancel
            </Button>
          </TableCell>
        </TableRow>
      )}
      {open && (
        <TableRow id={`${version.commit}:changes`} textValue="What it changed">
          <TableCell colSpan={6}>
            {diff === undefined ? (
              <p className="pending" aria-busy="true">
                <LoaderCircle className="node-icon spinning" />
                Reading what it changed…
              </p>
            ) : typeof diff === "string" ? (
              <p className="honest">{diff}</p>
            ) : (
              <>
                {diff.unseen.map((line) => (
                  <p className="honest" key={line}>
                    {line}
                  </p>
                ))}
                <pre className="skill-diff" data-testid="skill-diff">
                  {diff.text.split("\n").map((line, at) => (
                    <span
                      key={at}
                      className={diffLineKind(line) === "added" ? "ui-text-strong" : undefined}
                      data-line={diffLineKind(line)}
                    >
                      {line}
                      {"\n"}
                    </span>
                  ))}
                </pre>
                {diff.cut && (
                  <p className="honest">The changes are too long to show whole, and are cut.</p>
                )}
              </>
            )}
          </TableCell>
        </TableRow>
      )}
    </>
  );
}

/** **The skills this one names, and those that name it** (decision 57): on its Overview. */
export function SkillUses({ name, details: page }: { name: string; details: SkillDetails }) {
  return (
    <section aria-labelledby="skill-uses-heading" data-testid="skill-uses">
      <h3 id="skill-uses-heading">Uses and used by</h3>
      {page.uses.length === 0 ? (
        <p className="honest">{name} names no other skill.</p>
      ) : (
        <ul aria-label={`Skills ${name} names`}>
          {page.uses.map((one) => (
            <li key={one.said} data-broken={one.broken || undefined}>
              Uses <code>{one.name}</code>
              {one.project ? "" : " (built in)"}
              {one.broken && " — no skill has this name, so the reference is broken"}
            </li>
          ))}
        </ul>
      )}
      {page.used_by.length === 0 ? (
        <p className="honest">No other skill of this project names {name}.</p>
      ) : (
        <ul aria-label={`Skills that name ${name}`}>
          {page.used_by.map((other) => (
            <li key={other}>
              Used by <code>{other}</code>
            </li>
          ))}
        </ul>
      )}
      {page.uses_cut && (
        <p className="honest">
          This project holds more skills than purlis reads for this, so some are not counted.
        </p>
      )}
    </section>
  );
}
