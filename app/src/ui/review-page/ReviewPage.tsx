// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The review page** (#3672, spec #2897, a member of the panel family): a page in a view tab
 * whose body scrolls over a fixed action bar, for a review too long for a dialog (the install
 * review and the update review, file by file; the grade plan, test by test). A screen draws one
 * with this and never writes `data-scope="review-page"` itself (`purlis/review-page-family`).
 *
 * **The rules**, each held by construction:
 *
 * - **Two rows**: the body, which is the only part that scrolls, and the bar, always in view at
 *   the foot of the page's pane, on the panel's fill. The body ends where the bar starts, so the
 *   page scrolls under the bar's edge and **a focused control is never under it** (WCAG 2.4.11):
 *   the two never overlap, and the keyboard that reaches the last control above the bar brings it
 *   wholly into the body. A body that scrolls is a Tab stop of its own while it holds nothing the
 *   keyboard can reach, so its arrows scroll it.
 * - **The bar** says at its start what pressing the main action does and what it spends
 *   (`summary`), then holds at most three answers in a dialog's places: Cancel leads, the main
 *   action trails, and one other answer may stand between them. They are the dialog's own answer
 *   row (`DialogAnswerRow`), so the two cannot drift. The summary describes the main action.
 * - **Ways on that are no answer to the review** (`actions`) stand between the summary and the
 *   answers: a draft's Grade beside its Accept. Pressing one answers nothing, and the summary does
 *   not describe it.
 * - **The main action can be unavailable with its reason** (`isDisabled` with a `tooltip`): it
 *   stays a Tab stop and its reason is heard (`ui/unavailable.ts`, #2908).
 * - **⏎ in a field of the body is the main action**, as in a dialog, unless it is a grant or an act
 *   that cannot be taken back (`returnFinds`): those are pressed, never found by Return.
 * - **The bar keeps its height** when its summary or its main action's words change: the main
 *   action's `reserve` and the summary's `summaryReserve` list every words they can say, and each
 *   keeps the room of the largest.
 * - **Narrow** (under 30rem, the page's own width), the summary takes its own line over the
 *   answers.
 * - **By its parts** (ADR 0103): the `root`, the `body`, the `bar`, its `summary`, its `actions`
 *   and its `answers`. The bar is a consent surface where it holds a grant, so **its edge is not a
 *   theme's**: the hairline over it is the window's.
 */
import {
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
  type RefObject,
} from "react";
import { DialogAnswerRow, returnFinds, type DialogAction } from "../dialog/Dialog";
import { Reserved } from "../dialog/Reserved";
import { Form } from "../field/Field";
import { submits as chordSends } from "../field/submitKey";

export interface ReviewPageProps {
  /** What the body is named by, as a region: "Install deploy from Anthropic's skills". */
  label: string;
  /** What is reviewed. It scrolls; the bar never covers it. */
  children: ReactNode;
  /**
   * The sentence at the bar's start: what pressing the main action does and what it spends.
   * "Nothing is installed until you press Install." It describes the main action.
   */
  summary: ReactNode;
  /** Every summary the page can show, so changing it never changes the bar's height. */
  summaryReserve?: readonly string[];
  /** Cancel, or any other way out: the leading answer's press when `cancel` names none. */
  onCancel: () => void;
  /** The way out, leading. `Cancel` and `onCancel` unless it says otherwise; `false` for none. */
  cancel?: Partial<DialogAction> | false;
  /** The main action, trailing: what ⏎ in a field does. `primary` unless it says otherwise. */
  main?: DialogAction;
  /** One more answer, between the two. */
  alternative?: DialogAction;
  /** Ways on that answer nothing, between the summary and the answers. */
  actions?: ReactNode;
  /** A hook for what is inside it, and for the tests that find it. Never its size or place. */
  className?: string;
  "data-testid"?: string;
}

/** Whether `body` holds something the keyboard can reach. */
const REACHABLE =
  'a[href], button:not(:disabled), input:not([type="hidden"]):not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"]), [contenteditable="true"]';

/**
 * **Whether the body is a Tab stop of its own**: while it scrolls and holds nothing the keyboard
 * can reach (a long list of tests), so the arrows scroll it (WCAG 2.1.1). Measured as it is drawn
 * and each time it or what it holds changes size.
 */
function useScrollStop(body: RefObject<HTMLDivElement | null>): boolean {
  const [stop, setStop] = useState(false);
  useLayoutEffect(() => {
    const at = body.current;
    if (!at) return;
    const measure = () =>
      setStop(at.scrollHeight > at.clientHeight && at.querySelector(REACHABLE) === null);
    measure();
    // What it holds comes and goes (a list that arrives, a button that appears): each change is
    // measured again, and each new child watched for its size.
    const watch = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    const follow = () => {
      watch?.disconnect();
      watch?.observe(at);
      for (const child of at.children) watch?.observe(child);
    };
    follow();
    const held =
      typeof MutationObserver === "undefined"
        ? null
        : new MutationObserver(() => {
            follow();
            measure();
          });
    held?.observe(at, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["disabled", "tabindex"],
    });
    return () => {
      watch?.disconnect();
      held?.disconnect();
    };
  }, [body]);
  return stop;
}

/** **A review page**: its body, then its bar with the summary and the answers. */
export function ReviewPage({
  label,
  children,
  summary,
  summaryReserve,
  onCancel,
  cancel,
  main,
  alternative,
  actions,
  className,
  "data-testid": testId,
}: ReviewPageProps) {
  const summaryId = useId();
  const body = useRef<HTMLDivElement>(null);
  const stop = useScrollStop(body);
  const submit = (event: FormEvent<HTMLFormElement>) => {
    // Only the page's own form: a submit from anything inside it is that thing's.
    if (event.target !== event.currentTarget) return;
    event.preventDefault();
    if (main !== undefined && returnFinds(main) && !main.isDisabled) main.onPress?.();
  };
  // ⌘⏎ (Ctrl+Enter off macOS) presses a main action that says so, as a dialog's does: never a
  // grant, never what cannot be taken back, never one that cannot be pressed now.
  const chord = (event: KeyboardEvent<HTMLFormElement>) => {
    if (!chordSends(event.nativeEvent) || !event.currentTarget.contains(event.target as Node))
      return;
    if (
      main?.byChord !== true ||
      main.isDisabled === true ||
      main.variant === "grants" ||
      main.variant === "danger"
    )
      return;
    event.preventDefault();
    main.onPress?.();
  };
  const described = main && {
    ...main,
    "aria-describedby": [summaryId, main["aria-describedby"]].filter(Boolean).join(" "),
  };
  return (
    <Form
      className={className}
      data-testid={testId}
      data-scope="review-page"
      data-part="root"
      onSubmit={submit}
      onKeyDown={chord}
    >
      <div
        ref={body}
        role="region"
        aria-label={label}
        tabIndex={stop ? 0 : undefined}
        data-scope="review-page"
        data-part="body"
      >
        {children}
      </div>
      <div data-scope="review-page" data-part="bar">
        <div id={summaryId} data-scope="review-page" data-part="summary">
          {summaryReserve && summaryReserve.length > 0 ? (
            <Reserved as="div" texts={summaryReserve}>
              {summary}
            </Reserved>
          ) : (
            summary
          )}
        </div>
        {actions !== undefined && actions !== null && (
          <div data-scope="review-page" data-part="actions">
            {actions}
          </div>
        )}
        <div data-scope="review-page" data-part="answers">
          <DialogAnswerRow
            cancel={cancel}
            main={described as DialogAction | undefined}
            alternative={alternative}
            onClose={onCancel}
          />
        </div>
      </div>
    </Form>
  );
}
