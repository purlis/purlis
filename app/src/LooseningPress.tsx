// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { PolicyPress } from "./bindings";
import { inertly } from "./sandboxBlocks";

/** What the press is for, in one sentence: why it is here and what Accept does about it. */
export const pressReasonSaid = (press: PolicyPress): string => {
  switch (press.reason) {
    case "past-the-cap":
      return "A commit went past the most project versions purlis keeps as accepted, so nothing but starting a chat is allowed until you accept the project's policy again.";
    case "local-removal":
      return "A tightening is gone from your own settings file. Until you accept, the one you sealed stays in force.";
    case "lost-store":
      return press.lost ?? "";
    default:
      return "The project's policy loosens what you accepted.";
  }
};

/** The commit's line: who wrote it only where purlis verified it on the upstream's own copy. */
export const authorSaid = (press: PolicyPress): string | null => {
  if (press.commit === null) return null;
  return press.commit.author === null
    ? "Author unverified: purlis names an author only for a commit it finds on the upstream branch, and a repository with a replace ref never has one. Purlis does not review commits; accepting is the review."
    : `Author: ${inertly(press.commit.author)}.`;
};

/**
 * **The diff at the loosening press** (M75-15; spec §9.2): each rule that differs as loosens,
 * tightens or changes, each family's mode and each limit before and after, the commit and its
 * author, and every tightening of your own file the press would let go, so an Accept for an
 * unrelated commit never clears one unseen. It is drawn from what the core answered, against the
 * policy in force, never from the files. The dry run and Settings › Policy draw it too.
 */
export function LooseningPress({ press }: { press: PolicyPress }) {
  const commit = press.commit;
  const author = authorSaid(press);
  return (
    <div className="loosening-press" role="group" aria-label="What accepting changes">
      {commit !== null && (
        <p>
          Commit <code>{commit.sha.slice(0, 8)}</code>: {inertly(commit.subject)}
        </p>
      )}
      {author !== null && <p>{author}</p>}
      {press.rules.length > 0 && (
        <ul className="granted-list" aria-label="Rules that differ">
          {press.rules.map((one) => (
            <li key={one.id} data-kind={one.kind}>
              <code>{inertly(one.id)}</code>{" "}
              <span>
                {one.kind}: {one.what}
              </span>
            </li>
          ))}
        </ul>
      )}
      {press.modes.length > 0 && (
        <ul className="granted-list" aria-label="Modes that differ">
          {press.modes.map((one) => (
            <li key={one.key} data-kind={one.kind}>
              <code>{inertly(one.key)}</code>{" "}
              <span>
                {one.kind}: {one.before} to {one.after}
              </span>
            </li>
          ))}
        </ul>
      )}
      {press.limits.length > 0 && (
        <ul className="granted-list" aria-label="Limits that differ">
          {press.limits.map((one) => (
            <li key={one.key} data-kind={one.kind}>
              <code>{one.key}</code>{" "}
              <span>
                {one.kind}: {one.before} to {one.after}
              </span>
            </li>
          ))}
        </ul>
      )}
      {press.local.length > 0 && (
        <ul className="granted-list" aria-label="Your own file's tightenings that go">
          {press.local.map((one) => (
            <li key={one.key} data-kind="loosens">
              <code>{inertly(one.key)}</code>{" "}
              <span>
                your own file loosens: {one.was} to {one.now}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
