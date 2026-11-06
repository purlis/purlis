// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, type ReactNode } from "react";
import { NotRead, ReadingRows } from "./SkillPanels";

/** A read that may fail: what it answered, or why it could not. */
export type Read<T> = { data: T; refused?: undefined } | { data?: undefined; refused: string };

/** Asks `run`, and answers what it read or why it failed, never throwing. */
export async function reading<T>(
  run: () => Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
): Promise<Read<T>> {
  try {
    const answer = await run();
    return answer.status === "ok" ? { data: answer.data } : { refused: answer.error };
  } catch (err) {
    return { refused: String(err) };
  }
}

/** What an action that cannot be pressed says while another change is being made. */
export const BUSY = "Another change is being made.";

/** **A read in place**: rows held while it is asked, why it failed with Try again, or what it
 *  read, without hiding the rest of the page. */
export function Held<T>({
  read,
  what,
  again,
  children,
}: {
  read: Read<T> | undefined;
  /** What is read, as the sentences name it: "this project's skill sources". */
  what: string;
  again: () => void;
  children: (data: T) => ReactNode;
}) {
  if (read === undefined) return <ReadingRows what={what} testid="skill-sources-reading" />;
  if (read.refused !== undefined)
    return (
      <NotRead
        what={what}
        why={read.refused}
        onAgain={again}
        testid="skill-sources-unread"
        size="sm"
      />
    );
  return <>{children(read.data)}</>;
}

/** **A read for a page**: asked each time its `key` is another, or when asked again. */
export function useRead<T>(
  key: string | undefined,
  run: () => Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
) {
  const [asked, setAsked] = useState(0);
  const [held, setHeld] = useState<{ key: string; read: Read<T> }>();
  const now = key === undefined ? undefined : `${key}\u0000${asked}`;
  useEffect(() => {
    if (now === undefined) return;
    let live = true;
    void reading(run).then((read) => {
      if (live) setHeld({ key: now, read });
    });
    return () => {
      live = false;
    };
    // Read again for another key only: `run` is a new function each draw.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [now]);
  return {
    read: held !== undefined && held.key === now ? held.read : undefined,
    again: () => setAsked((n) => n + 1),
  };
}
