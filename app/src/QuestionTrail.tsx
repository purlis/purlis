// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import type { QuestionStep } from "./bindings";
import { Notice } from "./Notice";
import { TRAIL, trailSaid } from "./questionTrailWords";
import { Disclosure } from "./ui/disclosure/Disclosure";

/** What the trail knows: nothing yet, its steps, or why they could not be read. */
type Read = { steps?: readonly QuestionStep[]; trouble?: string };

/**
 * **What happened to a task's question** (#4567, spec #4551's story 55): a fold under the
 * question's line in its task's Activity, opened from **What happened**, that lists each step of
 * the question (asked, handed to its asking chat, passed on, reached you, answered and by whom,
 * closed) with its time, as the reviewed prototype draws it (#4561).
 *
 * **There when looked for, never pushed at the person**: it starts folded, and its steps are read
 * (`read`, from the audit entries) only as it opens, again each time. Nothing here counts on the
 * bell or makes an Update.
 *
 * **It holds no question words** (ADR 0075): the steps are a closed set, said in the window's
 * own words with the names of `task` and of `asker`, its asking chat, and its foot says so.
 */
export function QuestionTrail({
  task,
  asker,
  read,
  defaultExpanded = false,
}: {
  /** The task that asked, by the name its line shows. */
  task: string;
  /** The chat it asked, by the name its line shows. */
  asker: string;
  /** Reads the question's steps from the audit, oldest first. */
  read: () => Promise<QuestionStep[]>;
  /** Open from the start: a story's. */
  defaultExpanded?: boolean;
}) {
  const [open, setOpen] = useState(defaultExpanded);
  const [said, setSaid] = useState<Read>();
  const [again, setAgain] = useState(0);
  // The reader the owner hands now: a new one each render reads nothing again by itself.
  const reader = useRef(read);
  useEffect(() => {
    reader.current = read;
  }, [read]);
  useEffect(() => {
    if (!open) return;
    let left = false;
    reader.current().then(
      (steps) => !left && setSaid({ steps }),
      (why: unknown) =>
        !left && setSaid({ trouble: why instanceof Error ? why.message : String(why) }),
    );
    return () => {
      left = true;
    };
  }, [open, again]);

  return (
    <Disclosure
      label={TRAIL.open}
      size="sm"
      isExpanded={open}
      onExpandedChange={(now) => {
        // Read again each time it opens, from nothing.
        setSaid(undefined);
        setOpen(now);
      }}
      className="question-trail"
      data-testid="question-trail"
    >
      {open && (
        <TrailOf
          said={said}
          task={task}
          asker={asker}
          again={() => {
            setSaid(undefined);
            setAgain((n) => n + 1);
          }}
        />
      )}
    </Disclosure>
  );
}

/** The fold's panel: reading, the steps, none, or why not. */
function TrailOf({
  said,
  task,
  asker,
  again,
}: {
  said: Read | undefined;
  task: string;
  asker: string;
  again: () => void;
}) {
  if (said === undefined) {
    return (
      <p className="question-trail-quiet" aria-busy="true">
        {TRAIL.reading}
      </p>
    );
  }
  if (said.steps === undefined) {
    return (
      <Notice
        cause="question-trail-unread"
        tone="trouble"
        fixes={[{ label: "Read again", onPress: again }]}
      >
        {TRAIL.unread(said.trouble ?? "nothing was answered")}
      </Notice>
    );
  }
  const lines = trailSaid(said.steps, task, asker);
  if (lines.length === 0) return <p className="question-trail-quiet">{TRAIL.none}</p>;
  return (
    <>
      <p className="question-trail-quiet">{TRAIL.title}</p>
      <ol className="question-trail-steps" aria-label={TRAIL.title}>
        {lines.map((line, n) => (
          <li key={n}>
            <time className="question-trail-at" dateTime={line.at}>
              {line.time}
            </time>
            <span>{line.said}</span>
          </li>
        ))}
      </ol>
      <p className="question-trail-foot">{TRAIL.words}</p>
    </>
  );
}
