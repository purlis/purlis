// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { LoaderCircle } from "lucide-react";
import {
  commands,
  type SkillChanged,
  type SkillCopy,
  type SkillDetails,
  type SkillDiff,
  type SkillLevel,
  type SkillLifecycle,
  type SkillVersion,
  type StatsRow,
} from "./bindings";
import { counted, saidAt } from "./dispatches";
import { Notice } from "./Notice";
import { Button } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./ui/table/Table";
import { TextField } from "./ui/field/Field";
import { SkillPinnedBy } from "./SkillPins";
import { SkillGrade } from "./SkillGrade";

/** A version's newest grade, as its row says it (SK-16): the trigger tests that passed with it. */
export function gradeSaid(version: SkillVersion): string {
  const grade = version.grade;
  if (grade === null) return "Not graded";
  return `${grade.passed} of ${counted(grade.fixtures, "trigger test", "trigger tests")} passed`;
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

/** What a change answers: what it said beyond being done, or why it was not made. */
type Changed = { status: "ok"; data: SkillChanged } | { status: "error"; error: string };

/** The words a change that failed is said with, and what it said beyond being done. */
type Said = { tone: "news" | "trouble"; label: string; text: string };

/**
 * **A skill's history, lifecycle, owners and uses** (SK-19, #2782): what its tab shows beside
 * its runs. Each copy of the name at the project's levels has its own state, owners and
 * history, with a diff and a revert per version and the runs that started on it; the skills it
 * names and that name it are the name's.
 */
export function SkillDetailsPanel({
  plane,
  name,
  changed,
  onGraded,
  onCopy,
}: {
  plane: string;
  name: string;
  changed: number;
  /** Told when a grade of the skill ended (SK-16), so the tab reads its grades again. */
  onGraded?: () => void;
  /** Told the level of the copy shown, each time it is read or picked: the tab's Grades panel
   * shows that copy's grades alone (#3353). */
  onCopy?: (level: SkillLevel) => void;
}) {
  const [page, setPage] = useState<SkillDetails>();
  const [refused, setRefused] = useState<string>();
  const [picked, setPicked] = useState(0);
  const [done, setDone] = useState(0);
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<Said>();
  useEffect(() => {
    let live = true;
    void commands.skillDetails(plane, name).then((answer) => {
      if (!live) return;
      if (answer.status === "ok") {
        setPage(answer.data);
        setRefused(undefined);
      } else setRefused(answer.error);
    });
    return () => {
      live = false;
    };
  }, [plane, name, changed, done]);

  const shown = page?.copies[Math.min(picked, page.copies.length - 1)];
  useEffect(() => {
    if (shown !== undefined) onCopy?.(shown.level);
  }, [shown, onCopy]);

  if (refused !== undefined)
    return (
      <EmptyState
        headline={`purlis could not read the history of ${name}`}
        body={refused}
        testid="skill-details-refused"
      />
    );
  if (page === undefined)
    return (
      <p className="pending" aria-busy="true">
        <LoaderCircle className="node-icon spinning" />
        Reading the skill's history…
      </p>
    );

  const copy: SkillCopy | undefined = shown;
  const change = (label: string, act: () => Promise<Changed>) => {
    setBusy(true);
    void act().then((answer) => {
      setBusy(false);
      if (answer.status === "ok") {
        const notes = answer.data.notes;
        setSaid(
          notes.length > 0
            ? { tone: "news", label: "Skill changed", text: notes.join(" ") }
            : undefined,
        );
      } else setSaid({ tone: "trouble", label, text: answer.error });
      setDone((n) => n + 1);
    });
  };

  return (
    <div className="skill-details" data-testid="skill-details">
      {said !== undefined && (
        <Notice
          cause="skill-change"
          tone={said.tone}
          label={said.label}
          onDismiss={() => setSaid(undefined)}
        >
          {said.text}
        </Notice>
      )}
      {page.copies.length > 1 && (
        <div
          className="skill-copy-pick"
          role="group"
          aria-label="Copy"
          data-testid="skill-copy-pick"
        >
          {page.copies.map((one, at) => (
            <Button
              key={one.label}
              size="sm"
              variant={at === picked ? "soft-accent" : "ghost"}
              aria-pressed={at === picked}
              onPress={() => setPicked(at)}
            >
              {one.label}
            </Button>
          ))}
        </div>
      )}
      {copy !== undefined && (
        <CopyState
          key={`${copy.label}\u0000${copy.owners.join(",")}`}
          name={name}
          copy={copy}
          busy={busy}
          onLifecycle={(lifecycle) =>
            change("The skill's state was not changed", () =>
              commands.skillLifecycle(plane, copy.level, name, lifecycle),
            )
          }
          onOwners={(owners) =>
            change("The skill's owners were not changed", () =>
              commands.skillOwners(plane, copy.level, name, owners),
            )
          }
        />
      )}
      {copy !== undefined && (
        <SkillGrade
          plane={plane}
          name={name}
          level={copy.level}
          onGraded={onGraded ?? (() => setDone((n) => n + 1))}
        />
      )}
      {copy !== undefined && (
        <History
          plane={plane}
          name={name}
          copy={copy}
          busy={busy}
          onRevert={(commit) =>
            change("The skill was not reverted", () =>
              commands.skillRevert(plane, copy.level, name, commit),
            )
          }
        />
      )}
      <Uses name={name} details={page} />
      <SkillPinnedBy
        name={name}
        pinners={page.pinned_by}
        refused={page.pins_refused}
        busy={busy}
        onUnpin={(by) =>
          change("The skill was not unpinned", () => commands.skillPin(plane, by, name, false))
        }
        onRemove={(by, bad) =>
          change("The pin was not removed", () => commands.skillPin(plane, by, bad, false))
        }
      />
    </div>
  );
}

/** A copy's state, its suggestion to deprecate, and its owners. */
function CopyState({
  name,
  copy,
  busy,
  onLifecycle,
  onOwners,
}: {
  name: string;
  copy: SkillCopy;
  busy: boolean;
  onLifecycle: (lifecycle: SkillLifecycle) => void;
  onOwners: (owners: string[]) => void;
}) {
  const [replacement, setReplacement] = useState("");
  const [owners, setOwners] = useState(copy.owners.join(", "));
  const deprecate = () =>
    onLifecycle({
      state: "deprecated",
      replacement: replacement.trim() === "" ? null : replacement.trim(),
    });
  const state = copy.lifecycle.state;
  return (
    <section aria-labelledby="skill-state-heading" data-testid="skill-state">
      <h3 id="skill-state-heading">State</h3>
      <p data-testid="skill-lifecycle">{lifecycleSaid(name, copy.lifecycle)}</p>
      {copy.unread && <p className="honest">{UNREAD}</p>}
      {copy.unused_days !== null && state === "active" && (
        <p className="honest" data-testid="skill-unused">
          No chat has run {name} for {counted(copy.unused_days, "day", "days")}. Deprecate it, so
          chats are pointed to something else?
        </p>
      )}
      {state === "active" ? (
        <div className="skill-state-acts">
          <TextField
            size="sm"
            label="Use instead (optional)"
            aria-label="The skill to use instead"
            placeholder="another skill's name"
            value={replacement}
            onChange={setReplacement}
          />
          <Button size="sm" isDisabled={busy} onPress={deprecate}>
            Deprecate
          </Button>
          <Button
            size="sm"
            variant="danger"
            isDisabled={busy}
            onPress={() => onLifecycle({ state: "archived" })}
          >
            Archive
          </Button>
        </div>
      ) : (
        <Button size="sm" isDisabled={busy} onPress={() => onLifecycle({ state: "active" })}>
          {state === "archived" ? "Restore" : "Undo deprecation"}
        </Button>
      )}
      <h4>Owners</h4>
      <p className="honest">Who looks after this skill, for people to know.</p>
      <div className="skill-state-acts">
        <TextField
          size="sm"
          aria-label="Owners"
          placeholder="logins or emails, separated by commas"
          value={owners}
          onChange={setOwners}
        />
        <Button
          size="sm"
          isDisabled={busy || owners === copy.owners.join(", ")}
          onPress={() =>
            onOwners(
              owners
                .split(",")
                .map((one) => one.trim())
                .filter((one) => one !== ""),
            )
          }
        >
          Save owners
        </Button>
      </div>
    </section>
  );
}

/** A copy's versions, each with its runs, its changes and a revert. */
function History({
  plane,
  name,
  copy,
  busy,
  onRevert,
}: {
  plane: string;
  name: string;
  copy: SkillCopy;
  busy: boolean;
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
    <section aria-labelledby="skill-history-heading" data-testid="skill-history">
      <h3 id="skill-history-heading">History</h3>
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
            <p>Put the skill back as this version had it? The revert is saved as a new version.</p>
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

/** The skills this one names, and those that name it. */
function Uses({ name, details: page }: { name: string; details: SkillDetails }) {
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
