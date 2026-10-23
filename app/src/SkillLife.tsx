// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { LoaderCircle } from "lucide-react";
import {
  commands,
  type SkillCopy,
  type SkillDiff,
  type SkillLifecycle,
  type SkillPage,
  type SkillVersion,
} from "./bindings";
import { counted, saidAt } from "./dispatches";
import { Notice } from "./Notice";
import { Button } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";

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

/** A version's runs, as its row says them: signals, never a verdict. */
export function versionRunsSaid(version: SkillVersion): string {
  const runs = version.runs;
  if (runs.runs === 0) return "No runs";
  return [
    counted(runs.runs, "run", "runs"),
    counted(runs.tool_calls, "tool call", "tool calls"),
    counted(runs.tool_errors, "error", "errors"),
  ].join(", ");
}

/** The words a change that failed is said with, and what it said beyond being done. */
type Said = { tone: "news" | "error"; label: string; text: string };

/**
 * **A skill's history, lifecycle, owners and uses** (SK-19, #2782): what its tab shows beside
 * its runs. Each copy of the name at the project's levels has its own state, owners and
 * history, with a diff and a revert per version and the runs that started on it; the skills it
 * names and that name it are the name's.
 */
export function SkillLife({
  plane,
  name,
  changed,
}: {
  plane: string;
  name: string;
  changed: number;
}) {
  const [page, setPage] = useState<SkillPage>();
  const [refused, setRefused] = useState<string>();
  const [picked, setPicked] = useState(0);
  const [done, setDone] = useState(0);
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<Said>();
  useEffect(() => {
    let live = true;
    void commands.skillPage(plane, name).then((answer) => {
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

  if (refused !== undefined)
    return (
      <EmptyState
        headline={`purlis could not read the history of ${name}`}
        body={refused}
        testid="skill-life-refused"
      />
    );
  if (page === undefined)
    return (
      <p className="pending" aria-busy="true">
        <LoaderCircle className="node-icon spinning" />
        Reading the skill's history…
      </p>
    );

  const copy: SkillCopy | undefined = page.copies[Math.min(picked, page.copies.length - 1)];
  const change = (label: string, act: () => Promise<{ status: string; data?: unknown; error?: unknown }>) => {
    setBusy(true);
    void act().then((answer) => {
      setBusy(false);
      if (answer.status === "ok") {
        const notes = (answer.data as { notes: string[] }).notes;
        setSaid(
          notes.length > 0 ? { tone: "news", label: "Skill changed", text: notes.join(" ") } : undefined,
        );
      } else setSaid({ tone: "error", label, text: String(answer.error) });
      setDone((n) => n + 1);
    });
  };

  return (
    <div className="skill-life" data-testid="skill-life">
      {said !== undefined && (
        <Notice cause="skill-change" tone={said.tone} label={said.label} onDismiss={() => setSaid(undefined)}>
          {said.text}
        </Notice>
      )}
      {page.copies.length > 1 && (
        <label className="skill-copy-pick">
          Copy
          <select
            value={picked}
            onChange={(event) => setPicked(Number(event.target.value))}
            data-testid="skill-copy-pick"
          >
            {page.copies.map((one, at) => (
              <option key={one.label} value={at}>
                {one.label}
              </option>
            ))}
          </select>
        </label>
      )}
      {copy !== undefined && (
        <CopyState
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
        <History
          plane={plane}
          name={name}
          copy={copy}
          busy={busy}
          onRevert={(commit) =>
            change("The skill was not reverted", () => commands.skillRevert(plane, copy.level, name, commit))
          }
        />
      )}
      <Uses name={name} page={page} />
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
  useEffect(() => setOwners(copy.owners.join(", ")), [copy.owners]);
  const deprecate = () =>
    onLifecycle({ state: "deprecated", replacement: replacement.trim() === "" ? null : replacement.trim() });
  const state = copy.lifecycle.state;
  return (
    <section aria-labelledby="skill-state-heading" data-testid="skill-state">
      <h3 id="skill-state-heading">State</h3>
      <p data-testid="skill-lifecycle">{lifecycleSaid(name, copy.lifecycle)}</p>
      {copy.unread !== null && <p className="honest">Its state could not be read, so it is taken as in use: {copy.unread}</p>}
      {copy.unused_days !== null && state === "active" && (
        <p className="honest" data-testid="skill-unused">
          No chat has run {name} for {counted(copy.unused_days, "day", "days")}. Deprecate it, so chats are
          pointed to something else?
        </p>
      )}
      {state === "active" ? (
        <div className="skill-state-acts">
          <label>
            Use instead (optional)
            <input
              value={replacement}
              onChange={(event) => setReplacement(event.target.value)}
              placeholder="another skill's name"
              aria-label="The skill to use instead"
            />
          </label>
          <Button size="sm" isDisabled={busy} onPress={deprecate}>
            Deprecate
          </Button>
          <Button size="sm" variant="danger" isDisabled={busy} onPress={() => onLifecycle({ state: "archived" })}>
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
        <input
          value={owners}
          onChange={(event) => setOwners(event.target.value)}
          placeholder="logins or emails, separated by commas"
          aria-label="Owners"
          data-testid="skill-owners"
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
        <table className="ui-table" aria-label={`History of ${name}`}>
          <thead>
            <tr>
              <th scope="col">Saved</th>
              <th scope="col">By</th>
              <th scope="col">Change</th>
              <th scope="col">Runs on it</th>
              <th scope="col">
                <span className="visually-hidden">Actions</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {copy.versions.map((version, at) => (
              <VersionRow
                key={version.commit}
                version={version}
                newest={at === 0}
                open={open === version.commit}
                diff={open === version.commit ? diff : undefined}
                busy={busy}
                onShow={() => show(version.commit)}
                onRevert={() => onRevert(version.commit)}
              />
            ))}
          </tbody>
        </table>
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
  onRevert,
}: {
  version: SkillVersion;
  newest: boolean;
  open: boolean;
  diff: SkillDiff | string | undefined;
  busy: boolean;
  onShow: () => void;
  onRevert: () => void;
}) {
  return (
    <>
      <tr data-testid={`skill-version-${version.commit}`}>
        <td>{saidAt(version.at)}</td>
        <td>{version.author}</td>
        <td>{version.subject}</td>
        <td>{versionRunsSaid(version)}</td>
        <td>
          <Button size="sm" variant="ghost" aria-expanded={open} onPress={onShow}>
            {open ? "Hide changes" : "Show changes"}
          </Button>
          {!newest && (
            <Button size="sm" isDisabled={busy} onPress={onRevert}>
              Revert to this
            </Button>
          )}
        </td>
      </tr>
      {open && (
        <tr>
          <td colSpan={5}>
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
                      data-line={
                        line.startsWith("+") && !line.startsWith("+++")
                          ? "added"
                          : line.startsWith("-") && !line.startsWith("---")
                            ? "removed"
                            : undefined
                      }
                    >
                      {line}
                      {"\n"}
                    </span>
                  ))}
                </pre>
                {diff.cut && <p className="honest">The changes are too long to show whole, and are cut.</p>}
              </>
            )}
          </td>
        </tr>
      )}
    </>
  );
}

/** The skills this one names, and those that name it. */
function Uses({ name, page }: { name: string; page: SkillPage }) {
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
      {page.uses_cut && <p className="honest">This project holds more skills than purlis reads for this, so some are not counted.</p>}
    </section>
  );
}
