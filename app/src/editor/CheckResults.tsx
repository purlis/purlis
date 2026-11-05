// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A branch's check results, listed with the file they are read for** (ADR 0114, #5407). Each set
 * says its tool, how many results it has and the commit it was reported for, and that it is out of
 * date once the branch's head has moved. Each result says its severity, tool, rule, line and
 * message, under one of three headings: on the changed lines (marked in the gutter too), outside
 * them in this file, and in other files. **None is dropped**: past what a list holds, the rest are
 * counted. A result on a file of the branch opens at its line, in the file tab and in your editor.
 */
import type { CheckResult, CheckResults, PlaneId } from "../bindings";
import { jumpTo } from "../fileJump";
import type { Place } from "../pieceViews";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import type { CheckMark } from "./checkGutter";
import { ToYourEditor } from "./ToYourEditor";

const SEVERITY: Record<CheckResult["severity"], string> = {
  error: "Error",
  warning: "Warning",
  note: "Note",
  none: "No severity",
};

/** How much worse one severity is than another, to mark a line with its worst. */
const WORST: CheckResult["severity"][] = ["none", "note", "warning", "error"];

function results(count: number): string {
  return count === 1 ? "1 result" : `${count} results`;
}

/** What a result says, as one line: the words the gutter's title and the list both use. */
function said(one: CheckResult): string {
  const stale = one.outOfDate ? "Out of date. " : "";
  return `${stale}${one.tool}, ${SEVERITY[one.severity].toLowerCase()}: ${one.message}`;
}

/** One mark for each line a hunk shows that a result is on, in its worst severity. */
export function marksOf(checks: CheckResults): CheckMark[] {
  const lines = new Map<number, CheckResult[]>();
  for (const one of checks.onLines) {
    if (one.shownAt === null) continue;
    lines.set(one.shownAt, [...(lines.get(one.shownAt) ?? []), one]);
  }
  return [...lines].map(([line, on]) => ({
    line,
    severity: WORST[Math.max(...on.map((one) => WORST.indexOf(one.severity)))],
    said: on.map(said).join("\n"),
  }));
}

/** A path in the branch's folder can be opened; one the tool spelled absolute cannot. */
function opens(path: string | null): path is string {
  return path !== null && !path.startsWith("/") && !path.startsWith("..");
}

function Result({
  plane,
  cut,
  one,
  named,
}: {
  plane: PlaneId;
  cut: Place;
  one: CheckResult;
  named: boolean;
}) {
  const where = [
    named ? (one.path ?? "No file") : null,
    one.line === null ? null : `line ${one.line}`,
  ]
    .filter((part) => part !== null)
    .join(", ");
  const line = one.line ?? 1;
  const path = one.path;
  return (
    <li className="check-result">
      <span className="check-result-severity" data-severity={one.severity}>
        {SEVERITY[one.severity]}
      </span>
      <span className="check-result-said">
        <span className="check-result-from">
          {[one.tool, one.rule, where].filter((part) => part !== null && part !== "").join(", ")}
        </span>
        <span>{one.message}</span>
      </span>
      {opens(path) && (
        <span className="piece-files-actions">
          <Button size="sm" onPress={() => jumpTo({ plane, place: cut, path, line })}>
            {`Open at line ${line}`}
          </Button>
          <ToYourEditor plane={plane} cut={cut} path={path} line={line} />
        </span>
      )}
    </li>
  );
}

function Listed(props: {
  plane: PlaneId;
  cut: Place;
  title: string;
  list: CheckResult[];
  more: number;
  named?: boolean;
}) {
  const { plane, cut, title, list, more, named = false } = props;
  if (list.length === 0 && more === 0) return null;
  return (
    <>
      <h3 className="check-results-title">{title}</h3>
      <ul className="check-results-list">
        {list.map((one, at) => (
          <Result key={at} plane={plane} cut={cut} one={one} named={named} />
        ))}
      </ul>
      {more > 0 && <p className="piece-files-trouble">{`And ${results(more)} more.`}</p>}
    </>
  );
}

/** The list, or nothing for a branch nothing was reported for. */
export function CheckResultsList({
  plane,
  cut,
  path,
  results: checks,
}: {
  plane: PlaneId;
  cut: Place;
  path: string;
  results: CheckResults;
}) {
  if (checks.sets.length === 0 && checks.trouble === null) return null;
  return (
    <section className="check-results" aria-label={`Check results for ${path}`} tabIndex={0}>
      {checks.trouble !== null && (
        <p className="piece-files-trouble" role="status">
          {checks.trouble}
        </p>
      )}
      {checks.sets.map((set) => (
        <p key={set.tool} className="check-results-set">
          <span>
            {`${set.tool}: ${results(set.results)}, reported for ${set.head?.slice(0, 7) ?? "a branch with no commit"}`}
          </span>
          {set.outOfDate && (
            <>
              <Badge variant="waiting">Out of date</Badge>
              <span>The branch has moved since. Run the check and report it again.</span>
            </>
          )}
        </p>
      ))}
      <Listed
        plane={plane}
        cut={cut}
        title="On the changed lines"
        list={checks.onLines}
        more={checks.moreOnLines}
      />
      <Listed
        plane={plane}
        cut={cut}
        title="Outside the changed lines"
        list={checks.outside}
        more={checks.moreOutside}
      />
      <Listed
        plane={plane}
        cut={cut}
        title="In other files"
        list={checks.elsewhere}
        more={checks.moreElsewhere}
        named
      />
    </section>
  );
}
