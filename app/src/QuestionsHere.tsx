// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The project's task questions, lent to the rows that draw them** (#4564): the asks
 * registry's last list for the project, as `useAsks` read it, never a copy a row keeps. A row
 * asks for its own task's state through {@link useQuestionStanding}.
 */
import { createContext, useContext, useMemo } from "react";
import { standingOf, type QuestionStanding, type QuestionsHeld } from "./taskQuestions";

export const QuestionsHere = createContext<QuestionsHeld | undefined>(undefined);

/** Task `session`'s question, where it stands now, or nothing where it has none. */
export function useQuestionStanding(session: number): QuestionStanding | undefined {
  const held = useContext(QuestionsHere);
  const standing = standingOf(held, session);
  // Held on its values, so a list read again with the same question redraws no row.
  const level = standing?.level;
  const since = standing?.since;
  const asker = standing?.level === "with-asker" ? standing.asker : undefined;
  return useMemo<QuestionStanding | undefined>(
    () =>
      level === undefined
        ? undefined
        : level === "needs-you"
          ? { level, since: since ?? null }
          : { level, asker: asker ?? "", since: since ?? 0 },
    [level, since, asker],
  );
}

/** The project's task questions as the registry lists them, or nothing before its first read. */
export function useQuestionsHere(): QuestionsHeld | undefined {
  return useContext(QuestionsHere);
}
