// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **An alert the app raises, with no press of the person's** (#7728; ADR 0104 as amended
 * 2026-11-26, "the dialog family serves an app-raised alert"): the dialog family's
 * `AlertDialog`, with its rules whole (three rows, at most three answers in their places, ⏎ and
 * Esc, no dialog over another), and what a question the person did not ask for adds:
 *
 * - **Its answers' places.** `cancel` grants nothing: it leads, Esc chooses it, and the keyboard
 *   starts on it, since the person may be typing when it opens. `main` is the granting answer, of
 *   the `grants` variant, which Return never finds (`returnFinds`).
 * - **A settle guard** (#1695's {@link SETTLE_MS}): no key and no press reaches an answer for that
 *   long after it is drawn or its content changes. A press says why it did nothing; a key held
 *   down (one that repeats) answers nothing at any time.
 * - **No silent swap.** A change of what it shows (`shows`) while it is up is said, drawn, arms
 *   the guard again and moves the keyboard off the answers to `cancel`.
 * - **When it may open, and one at a time** (`raised.ts`): never during an IME composition, while
 *   a key is down, or while a dialog, the palette, a menu or a select's list is up; one per
 *   window, the rest waiting in the order raised, each saying where it stands ("1 of 2").
 * - **For a screen reader**: `aria-busy` while an answer's press is in flight (its `onPress`
 *   returned a promise that has not settled), and no second press meanwhile; described by its
 *   count and its why-line; a body that scrolls is a stop of the keyboard's.
 * - **The keyboard goes back** to where it was when it opened, as it closes.
 *
 * **It closes where it is answered**: the screen stops raising it (`raised` false) once the
 * answer has landed. An answer that does not land (a refusal) leaves it up to say so.
 */
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { SETTLE_MS } from "../../noticeSettle";
import { RaisedAlertFrame, type DialogAction, type DialogSize } from "./Dialog";
import { useRaisedTurn } from "./raised";

/** What a press within the settle interval says: nothing was done. */
export const RAISED_ALERT_TOO_SOON =
  "This question was drawn or changed just now, so nothing was done. Read it and press again.";

/** What a change of its content while it is up says. */
export const RAISED_ALERT_CHANGED = "This question changed while it was open. Read it again.";

/** The keys that answer: Return and Space press the answer they are on, Esc chooses `cancel`. */
const ANSWER_KEYS = new Set(["Enter", " ", "Escape"]);

type Pressed<A> = A extends unknown
  ? Omit<A, "onPress"> & {
      /** The answer. A promise says it is in flight until it settles. */
      onPress: (() => void) | (() => Promise<unknown>);
    }
  : never;

/** **One answer of a raised alert**: the family's answer, whose press may be in flight. */
export type RaisedAnswer = Pressed<DialogAction>;

export interface RaisedAlertProps {
  /** Whether the app has it to ask now. It waits for its turn and a quiet window to open. */
  raised: boolean;
  /**
   * **What it shows, as one value** (a digest, a reason): a change while it is up is said, drawn,
   * and arms the settle guard again. A redraw with the same value is no change.
   */
  shows: string;
  title: ReactNode;
  /** The why-line: what each answer does. It describes the alert, with its count. */
  why: ReactNode;
  /** What the footer says before its answers. */
  summary?: ReactNode;
  /** The answer that grants nothing: it leads, Esc chooses it, and the keyboard starts on it. */
  cancel: RaisedAnswer;
  /** The granting answer, trailing: `grants`, so Return never finds it. */
  main?: RaisedAnswer;
  /** One more answer, between the two. */
  alternative?: RaisedAnswer;
  size?: DialogSize;
  /** A hook for what is inside it, and for the tests that find it. */
  className?: string;
  /** Called once it has closed and the keyboard has gone back to what had it. */
  afterClose?: () => void;
  "data-testid"?: string;
  children?: ReactNode;
}

/**
 * **The settle guard and the press in flight** of a raised alert while it is `open` and shows
 * `shows`: what it says of itself, whether it is busy, the live region that says it (`saying`),
 * and `press`, which an answer's press goes through.
 */
function useSettledPresses(open: boolean, shows: string) {
  const [said, setSaid] = useState("");
  const [busy, setBusy] = useState(false);
  // What was last drawn: adjusted as it renders, so what it says is right in the same commit.
  const [drawn, setDrawn] = useState({ open, shows });
  if (drawn.open !== open || drawn.shows !== shows) {
    setDrawn({ open, shows });
    if (open && !drawn.open) setSaid("");
    else if (open) setSaid(RAISED_ALERT_CHANGED);
  }
  const inFlight = useRef(false);
  /** When it was last drawn anew or changed: the settle guard's start. */
  const armedAt = useRef(-Infinity);
  const saying = useRef<HTMLParagraphElement>(null);
  const settled = () => performance.now() - armedAt.current >= SETTLE_MS;

  useLayoutEffect(() => {
    if (open) armedAt.current = performance.now();
  }, [open]);

  const shown = useRef(shows);
  useLayoutEffect(() => {
    const was = shown.current;
    shown.current = shows;
    if (!open || was === shows) return;
    armedAt.current = performance.now();
    // Off any answer but the one that grants nothing.
    const alert = saying.current?.closest('[role="alertdialog"]');
    const answers = alert?.querySelector('[data-scope="dialog"][data-part="actions"]');
    const keep = answers?.querySelector<HTMLElement>('[data-dialog-action="cancel"]');
    const now = document.activeElement;
    if (keep && now !== keep && answers?.contains(now)) keep.focus();
  }, [open, shows]);

  // **No key answers while it settles, nor one held down**: heard on the window as it is
  // captured, before anything in the alert or the family's own Esc.
  useEffect(() => {
    if (!open) return;
    const guard = (event: KeyboardEvent) => {
      if (!ANSWER_KEYS.has(event.key) || event.isComposing) return;
      if (!event.repeat && performance.now() - armedAt.current >= SETTLE_MS) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (!event.repeat) setSaid(RAISED_ALERT_TOO_SOON);
    };
    window.addEventListener("keydown", guard, true);
    return () => window.removeEventListener("keydown", guard, true);
  }, [open]);

  const press = (onPress: RaisedAnswer["onPress"]) => {
    if (inFlight.current) return;
    if (!settled()) {
      setSaid(RAISED_ALERT_TOO_SOON);
      return;
    }
    setSaid("");
    const landing = onPress();
    if (!(landing instanceof Promise)) return;
    inFlight.current = true;
    setBusy(true);
    const landed = () => {
      inFlight.current = false;
      setBusy(false);
    };
    landing.then(landed, landed);
  };
  return { said, busy, saying, press };
}

/** **An alert the app raises** (see above). */
export function RaisedAlert({
  raised,
  shows,
  title,
  why,
  summary,
  cancel,
  main,
  alternative,
  size,
  className,
  afterClose,
  "data-testid": testId,
  children,
}: RaisedAlertProps) {
  const turn = useRaisedTurn(raised);
  const open = turn !== null;
  const { said, busy, saying, press } = useSettledPresses(open, shows);
  const answerOf = (answer: RaisedAnswer | undefined) =>
    answer === undefined
      ? undefined
      : ({ ...answer, onPress: () => press(answer.onPress) } as DialogAction);

  return (
    <RaisedAlertFrame
      isOpen={open}
      onClose={() => press(cancel.onPress)}
      busy={busy}
      title={title}
      subtitle={turn !== null && turn.of > 1 ? `${turn.at} of ${turn.of}` : undefined}
      description={why}
      summary={summary}
      cancel={answerOf(cancel)}
      main={answerOf(main)}
      alternative={answerOf(alternative)}
      initialFocus="cancel"
      size={size}
      className={className}
      afterClose={afterClose}
      data-testid={testId}
    >
      <p ref={saying} role="status" data-scope="dialog" data-part="said" className="ui-text-body">
        {said}
      </p>
      {children}
    </RaisedAlertFrame>
  );
}
