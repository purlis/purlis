// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ReviewSummary, UpdateFile } from "./bindings";
import { Badge } from "./ui/badge/Badge";
import { Disclosure } from "./ui/disclosure/Disclosure";

/**
 * **What a review shows of a skill's change**: its summary, risky items marked, and each file's
 * diff, folded. One update's review (`SkillSources`) and several updates' (#5354,
 * `SkillSetUpdateReview`) draw them alike.
 */

/** The mark of a risky item and of a risky file: in the waiting tone, never a colour alone. */
export const LOOK_CLOSELY = <Badge variant="waiting">Look closely</Badge>;

/** A summary, risky items marked, as the core reads and marks it. */
export function Summary({ summary, testid }: { summary: ReviewSummary; testid?: string }) {
  if (summary.empty)
    return (
      <p className="honest" data-testid={testid}>
        Instructions only: no scripts, network hosts, ! commands or writes outside the workspace.
      </p>
    );
  const mark = (risky: boolean) => (risky ? LOOK_CLOSELY : null);
  return (
    <ul className="skill-summary" data-testid={testid}>
      {summary.scripts.map((one) => (
        <li key={`s${one.path}`} data-risky={one.risks.length > 0}>
          {mark(one.risks.length > 0)} Script <code>{one.path}</code> runs with {one.runs}
          {one.risks.length > 0 && `: ${one.risks.map((risk) => risk.said).join(", ")}`}
        </li>
      ))}
      {summary.hosts.map((one) => (
        <li key={`h${one.host}`} data-risky={one.risky}>
          {mark(one.risky)} Network host <code>{one.host}</code>, named in {one.files.join(", ")}
        </li>
      ))}
      {summary.commands.map((one) => (
        <li key={`c${one.path}${one.line}${one.text}`} data-risky={true}>
          {mark(true)} ! command <code>{one.text}</code> ({one.path}, line {one.line})
        </li>
      ))}
      {summary.outside.map((one) => (
        <li key={`o${one.path}${one.line}${one.text}`} data-risky={true}>
          {mark(true)} Writes outside the workspace: <code>{one.text}</code> ({one.path}, line{" "}
          {one.line})
        </li>
      ))}
      {summary.unseen.map((one) => (
        <li key={`u${one.path}${one.line}`} data-risky={true}>
          {mark(true)} Hidden text in {one.path}, line {one.line}:{" "}
          {one.kinds.map((kind) => kind.said).join(", ")}
        </li>
      ))}
      {summary.not_text.map((path) => (
        <li key={`n${path}`}>
          <code>{path}</code> is not text, so it is not shown
        </li>
      ))}
    </ul>
  );
}

/** Each file's change, folded, its kind of change after its path. */
export function Changes({ files, open = false }: { files: readonly UpdateFile[]; open?: boolean }) {
  return (
    <>
      {files.map((file) => (
        <Disclosure
          key={file.path}
          size="sm"
          defaultExpanded={open}
          label={file.path}
          trail={<Badge>{file.change}</Badge>}
        >
          <pre className="skill-review-file">
            {file.diff === "" ? "Not text, so not shown." : file.diff}
          </pre>
        </Disclosure>
      ))}
    </>
  );
}
