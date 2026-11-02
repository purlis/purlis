// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { ArrowDownToLine, ChevronLeft, ChevronRight, Hand } from "lucide-react";
import type { Shown } from "./bindings";
import { Button, IconButton } from "./ui/button/Button";
import { Twist } from "./ui/disclosure/Disclosure";
import { STRIP, cutOnOneLine, waitOf, type Wait } from "./waitingOn";

/**
 * **The waiting strip** (#4563, spec #4551, the reviewed prototype of #4561): one line at the top
 * of a chat's pane, above its band of Notices, that says what the chat waits on the person for,
 * however much it writes after. It is the strip of the chat the pane shows, and it goes the moment
 * the registry lists nothing that chat waits on them for.
 *
 * **At the top, not by the input** (the person's ruling of 2026-11-02): a chat's input is inside
 * its terminal, and a strip at the pane's foot moved that line as it came and went. Here the
 * terminal gives up the strip's one line at its top, refits, and its input stays at its foot.
 *
 * **It never takes the keyboard.** Nothing in it is focused as it comes, changes or goes; only a
 * press inside it moves the keyboard, and only to the control that takes the place of the one
 * pressed. So a person typing in the terminal types on whatever arrives.
 *
 * **One line, whatever it holds**: Show all opens the whole text, and the earlier messages are
 * read one at a time in the same line, so walking them never changes its height or the
 * terminal's.
 */
export function WaitingStrip({
  session,
  waiting,
}: {
  /** The chat the pane shows. */
  session: number;
  /** The project's asks, as the registry derived them last (the Inbox's own list). */
  waiting: readonly Shown[] | undefined;
}) {
  const wait = waitOf(waiting, session);
  if (wait === undefined) return null;
  // A new wait, or a new last message, starts from the last message, folded.
  return <Strip key={`${wait.ask}\u0000${wait.says}`} wait={wait} />;
}

function Strip({ wait }: { wait: Wait }) {
  const earlier = wait.kind === "prose" ? wait.earlier : [];
  /** Which earlier message is read, by its place in `earlier`; null for the last. */
  const [at, setAt] = useState<number | null>(null);
  const [open, setOpen] = useState(false);
  const says = at === null ? wait.says : earlier[at];
  /** The control the keyboard goes on to once a press took away the one it was on. */
  const goOn = useRef<"earlier" | "previous" | null>(null);
  const earlierAt = useRef<HTMLButtonElement>(null);
  const previousAt = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const to = goOn.current;
    goOn.current = null;
    if (to === "earlier") earlierAt.current?.focus();
    if (to === "previous") previousAt.current?.focus();
  }, [at]);
  const go = (to: number | null, then: "earlier" | "previous" | null) => {
    goOn.current = then;
    setAt(to);
  };
  const label =
    wait.kind !== "prose"
      ? wait.kind === "ask" || wait.kind === "prompt"
        ? `${STRIP.waitingOn} · ${STRIP.answerBelow}`
        : STRIP.waitingOn
      : at === null
        ? STRIP.lastMessage
        : STRIP.earlierAt(earlier.length - at, earlier.length);
  const long = cutOnOneLine(says);
  return (
    <div
      className="waiting-strip"
      role="group"
      aria-label={STRIP.waitingOn}
      data-kind={wait.kind}
      data-open={open ? "" : undefined}
    >
      <span className="waiting-strip-mark" aria-hidden="true">
        <Hand />
      </span>
      <div className="waiting-strip-words">
        <span className="waiting-strip-label ui-text-meta">{label}</span>
        <p
          className="waiting-strip-says"
          // Open, its whole text may scroll inside it, so the keyboard can reach it to.
          tabIndex={open ? 0 : undefined}
        >
          {says}
        </p>
      </div>
      {earlier.length > 0 && (
        <span className="waiting-strip-ways">
          {at === null ? (
            <Button
              ref={earlierAt}
              size="sm"
              variant="ghost"
              onPress={() => go(earlier.length - 1, "previous")}
            >
              {STRIP.earlier(earlier.length)}
            </Button>
          ) : (
            <>
              <IconButton
                ref={previousAt}
                size="sm"
                variant="ghost"
                aria-label={STRIP.previous}
                isDisabled={at === 0}
                onPress={() => go(at - 1, null)}
                icon={<ChevronLeft aria-hidden="true" />}
              />
              <IconButton
                size="sm"
                variant="ghost"
                aria-label={STRIP.next}
                onPress={() => (at === earlier.length - 1 ? go(null, "earlier") : go(at + 1, null))}
                icon={<ChevronRight aria-hidden="true" />}
              />
              <IconButton
                size="sm"
                variant="ghost"
                aria-label={STRIP.backToLast}
                onPress={() => go(null, "earlier")}
                icon={<ArrowDownToLine aria-hidden="true" />}
              />
            </>
          )}
        </span>
      )}
      {(long || open) && (
        <Button
          size="sm"
          variant="ghost"
          className="waiting-strip-all"
          aria-expanded={open}
          onPress={() => setOpen((was) => !was)}
        >
          <Twist open={open} />
          {open ? STRIP.showLess : STRIP.showAll}
        </Button>
      )}
    </div>
  );
}
