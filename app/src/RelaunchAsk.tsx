// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { RelaunchChoice, RelaunchQuestion } from "./bindings";
import { AlertDialog } from "./ui/dialog/Dialog";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * What a launch asks before it puts anything back (charter-app#250): reopen every session, or
 * start fresh.
 *
 * The operator's words: *"on reopen - we need to suggest re-open all sessions - or start fresh
 * session, and same will work for update flow."* It is asked once, only when the last quit left
 * something open, and **nothing starts while it is up** — the core holds the launch's own
 * project back until `relaunch` has the answer, and the window restores the other projects
 * after it (`App.tsx`).
 *
 * **Every way out that is not the "Start fresh" button is "Reopen all"**: the leading answer,
 * which has the keyboard, and Escape. Starting fresh clears the record, and an answer lost to a
 * stray key must never be the one that throws work away.
 *
 * **The family's `AlertDialog`**, for `EndingChat`'s reasons: it arrives without being asked
 * for, a click outside answers nothing, and it starts on its safe answer.
 *
 * `question.after_update` is charter-app#251's: Restart to update writes it into each record,
 * and the launch it restarts into says why it is being asked. "Reopen all" is in front either
 * way.
 *
 * Each project row carries its whole path as its tooltip, for `calledOn`'s reason: two projects
 * can share a folder name.
 */
export function RelaunchAsk({
  question,
  nameOf,
  onAnswer,
}: {
  question: RelaunchQuestion;
  /** What a project is called on its tab, so the question names it the same way. */
  nameOf: (plane: string) => string;
  onAnswer: (choice: RelaunchChoice) => void;
}) {
  const chats = question.projects.reduce((sum, project) => sum + project.chats, 0);
  const views = question.projects.reduce((sum, project) => sum + project.views, 0);
  const open = counts(chats, views, " and ");
  return (
    <AlertDialog
      onClose={() => onAnswer("ReopenAll")}
      title="Reopen your sessions?"
      description={
        <p className="honest">
          {question.after_update && "purlis restarted to install an update. "}
          {`${open} ${chats + views === 1 ? "was" : "were"} open when purlis last quit.`}
        </p>
      }
      // The way out, which has the keyboard, is "Reopen all", as Escape is.
      cancel={{ children: "Reopen all sessions" }}
      main={{
        children: "Start fresh",
        // `danger`: starting fresh clears the record of what was open, and that cannot be
        // taken back (`docs/design-system.md`, the dialog's footer).
        variant: "danger",
        onPress: () => onAnswer("StartFresh"),
      }}
    >
      <ul className="ending">
        {question.projects.map((project) => (
          // Its whole directory, where its name shows less of it.
          <Tooltip
            key={project.plane}
            tip={nameOf(project.plane) === project.plane ? undefined : project.plane}
          >
            <li>
              <span>{nameOf(project.plane)}</span>
              <span className="what">{counts(project.chats, project.views, ", ")}</span>
            </li>
          </Tooltip>
        ))}
      </ul>
    </AlertDialog>
  );
}

/** `3 chats and 1 view tab`, leaving out a kind there are none of. */
function counts(chats: number, views: number, between: string): string {
  return [counted(chats, "chat"), counted(views, "view tab")].filter(Boolean).join(between);
}

/** `3 chats`, `1 view tab`, or nothing at all for none. */
function counted(howMany: number, what: string): string {
  if (howMany === 0) return "";
  return `${howMany} ${what}${howMany === 1 ? "" : "s"}`;
}
