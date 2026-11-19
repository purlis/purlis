// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type AnswersShown, type PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { askSettingsLink } from "./settings/links";
import { SEALED_ANSWERS } from "./settings/Answers";

/** What the Notice says of the nevers imported from the old files. */
export function importedSays(nevers: readonly string[]): string {
  const count = nevers.length === 1 ? "1 never" : `${nevers.length} nevers`;
  return `purlis sealed the ${count} you said before: ${nevers.join(", ")}. They hold as they did. The files they were kept in are safe to delete, and doctor lists them.`;
}

/** What the Notice says where the answers are in the deny state. */
export function denySays(deny: readonly string[]): string {
  return `Your answers for this project are in the deny state, so nothing they allow holds: ${deny.join("; ")}. Accepting again in the window mends it.`;
}

/** Whether `data` is the shape the core answers: a window under test may answer anything. */
function readable(data: unknown): data is AnswersShown {
  const shown = data as Partial<AnswersShown> | null | undefined;
  return (
    shown !== null &&
    shown !== undefined &&
    typeof shown.state === "string" &&
    Array.isArray(shown.deny) &&
    Array.isArray(shown.imported) &&
    Array.isArray(shown.notices)
  );
}

/**
 * **What the sealed answers say at a project's window** (M75-07a, #7015): the nevers imported
 * from the old files (once, listed), the deny state and why, and what purlis can't do (no
 * keyring). Each is a standing line with Dismiss, and the deny state's way out is Settings'
 * Sealed answers, where the cause is told in full and the device id confirmed. Reading the state
 * registers the project and imports the nevers, so it is the first thing the window asks.
 */
export function AnswersNotices({ plane }: { plane: PlaneId }) {
  const [shown, setShown] = useState<AnswersShown>();
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
  useEffect(() => {
    let gone = false;
    void commands
      .answersShown(plane)
      .then((done) => {
        if (!gone && done.status === "ok" && readable(done.data)) setShown(done.data);
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [plane]);
  if (shown === undefined) return null;
  const dismiss = (cause: string) => setDismissed((was) => new Set(was).add(cause));
  const link = {
    label: "Show sealed answers",
    onPress: () => askSettingsLink(plane, { group: "project.sandbox", setting: SEALED_ANSWERS }),
  };
  const lines: {
    cause: string;
    tone: "news" | "attention" | "trouble";
    label: string;
    text: string;
    link?: typeof link;
  }[] = [];
  if (shown.import_failed !== null) {
    lines.push({
      cause: "answers-import-failed",
      tone: "trouble",
      label: "Nevers not imported",
      text: shown.import_failed,
      link,
    });
  }
  if (shown.state === "deny") {
    lines.push({
      cause: `answers-deny:${shown.deny.join("|")}`,
      tone: "trouble",
      label: "Answers in the deny state",
      text: denySays(shown.deny),
      link,
    });
  }
  if (shown.imported.length > 0) {
    lines.push({
      cause: `answers-imported:${shown.imported.join("|")}`,
      tone: "news",
      label: "Nevers sealed",
      text: importedSays(shown.imported),
    });
  }
  for (const text of shown.notices) {
    lines.push({ cause: `answers-notice:${text}`, tone: "news", label: "Sealed answers", text });
  }
  return (
    <>
      {lines
        .filter((line) => !dismissed.has(line.cause))
        .map((line) => (
          <Notice
            key={line.cause}
            cause={line.cause}
            tone={line.tone}
            label={line.label}
            link={line.link}
            onDismiss={() => dismiss(line.cause)}
          >
            {line.text}
          </Notice>
        ))}
    </>
  );
}
