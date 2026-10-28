// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  createContext,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { PersonaMark } from "./PersonaMark";
import { NoticeWayGuard, useNoticeSettle } from "./noticeSettle";
import { ChevronDown, ChevronRight, X } from "lucide-react";
import { Button, ButtonRow, IconButton } from "./ui/button/Button";
import {
  NoticeBox,
  NoticeListBox,
  NoticeMark,
  NoticeMessage,
  NoticeMoreSays,
  NoticeRow,
  NoticeStack,
  NoticeTitle,
  NoticeUnder,
  NoticeWhose,
  type NoticeListAt,
  type NoticeTone,
} from "./ui/notice/Notice";

/**
 * **A Notice: a standing line in a project's window about something true now** (CONTEXT.md,
 * rulings V91a–d, NO-1 #1223).
 *
 * The window used to draw each of these by hand — a `<p>` with a sentence, sometimes a Dismiss,
 * sometimes nothing — and about thirty of them left the operator reading a problem with nothing
 * to press. This component is the one way to draw one, and **its props will not take a Notice
 * with no way out**: it needs at least one of
 *
 * - `fixes` — charter does it, here, on the press (Retry, Undo, Turn the sandbox on);
 * - `link` — it goes to where the problem is fixed (a Settings group, a record, a view);
 * - `copy` — the command to run elsewhere, **the last resort** (V91q): allowed only where the
 *   window has no fix, and every Notice whose only remedy it is, is listed as debt by
 *   `Notice.guard.test.ts`;
 * - `onDismiss` — hides it.
 *
 * **`cause` names what the line is about**, stably (`pin-gone:ide`, `chat-did-not-start:3`).
 * It is what a dismissal is keyed by once Dismiss lasts until the cause changes (NO-2), and it
 * is on the element as `data-cause` so a test can name the Notice it means.
 *
 * `tone` is `news` (nothing is wrong, the operator is being told), `attention` (nothing is wrong,
 * but the person must decide: a change to review, a plan that changed under them, a draft out of
 * date, #3678) or `trouble` (something went wrong, and only then), and it is only the look: a
 * Notice is always a polite `status`, because it stands until it is dealt with and an `alert`
 * would interrupt a screen reader on every relaunch. Attention is drawn on its own ground with
 * its mark, a triangle, and says "Needs a decision:" to a reader, so it is never told by colour
 * alone.
 * `at` is where it stands: in the Inbox's Notices (`inbox`, the default, `NoticeList`, #1695), or
 * in its pane's row of Notices above the terminal, never over it (`pane`, `NoticePaneRow`,
 * #1647).
 *
 * **A Notice in a pane fits its pane, whatever it says** (#1481). It began as one short line;
 * it now also carries a long sentence, three long buttons and what one of them opens. So
 * `at="pane"` draws one box holding the line and, under it, what a way out opened. The sentence
 * has the row; the ways out share it only when all of it fits unwrapped, and otherwise go under
 * the sentence; nothing is ever wider or taller than the pane.
 *
 * **It is drawn by the notice family** (#2912, `ui/notice`): its box, its line, what it says and
 * what a way out opened are the family's parts, its tone the box's variant and where it stands
 * its placement, and each way out is a button of the button family that says which it is
 * (`data-way`: `go`, `fix`, `link`, `copy`, `dismiss`; a pane row's `more`).
 *
 * `guard.test` in this folder fails on a standing line built any other way, so a new dead end
 * cannot come back in.
 */
export type NoticeAction = {
  /** The button's words: a verb for what it does ("Undo", "Open record"). */
  label: string;
  onPress: () => void;
  /** For a press that opens something under the line (a fix's form, #1250): the id of what
   *  it opens and whether it is open, said to a screen reader as the button's state. */
  opens?: { id: string; open: boolean };
  /** While what the press started is on its way: the button says it is busy and cannot be
   *  pressed, and a press sends nothing (an Undo in flight, #1190). `aria-disabled` and not
   *  `disabled`, so the keyboard stays on it rather than dropping to the page (#1700). */
  busy?: boolean;
  /** Why this way out is off, when it is: a read-only project's sentence for an answer that
   *  writes the project's files (#826). Drawn, said as its tooltip, and `aria-disabled` as a
   *  busy one is, so the keyboard stays on it; a press sends nothing. */
  off?: string;
  /** **An answer that grants** (an Allow, an Accept): drawn as the `grants` variant, edged in
   *  the accent and led by an open lock, beside Keep blocked and every other way out in the plain
   *  style, so the two are told apart at a glance in every theme with equal weight (ADR 0097
   *  §6). Only the look: its place, its words, its name and the keyboard's order are the
   *  Notice's as they were. */
  allows?: boolean;
  /** The one way on a card Notice leads with (#3344): drawn as the filled primary. */
  primary?: boolean;
  /** A card Notice's disclosure (its Details, #3344): a ghost button led by a chevron that
   *  points down while what it opens is open. Needs `opens`. */
  disclosure?: boolean;
};

type Ways = {
  /** What charter can do about it on a press, in the order they read. */
  fixes?: readonly [NoticeAction, ...NoticeAction[]];
  /** Where it is fixed. */
  link?: NoticeAction;
  /** The command that fixes it, offered as Copy command. The last resort, and debt (V91q). */
  copy?: string;
  /** Hides the Notice. */
  onDismiss?: () => void;
  /** What Dismiss does, where it is more than putting the line away: said as its tooltip
   *  (qw386). */
  dismissSays?: string;
};

/** At least one way out, said in the type: a Notice with none does not compile. */
type WayOut =
  | (Ways & { fixes: readonly [NoticeAction, ...NoticeAction[]] })
  | (Ways & { link: NoticeAction })
  | (Ways & { copy: string })
  | (Ways & { onDismiss: () => void });

/** What every Notice says: what it is about, its tone, its sentence and what a way out opened. */
type Said = {
  /** What the line is about, stably: what a dismissal will be keyed by. */
  cause: string;
  tone?: NoticeTone;
  /** The sentence (and anything else the line says before its ways out). */
  children: ReactNode;
  /** What a way out opened (a fix's form, #1250): drawn right after the line, outside its live
   *  region, and moved with it by the Inbox's list. */
  under?: ReactNode;
};

/** **A line Notice**: in a pane's row, the Inbox's list or alone on a page. */
type LineProps = WayOut &
  Said & {
    at?: "inbox" | "pane";
    /** The accessible name, where the sentence alone would not make a good one. */
    label?: string;
    /** The persona the line names, when it names one: its mark is drawn before the sentence
     *  (#1449). */
    persona?: string | null;
    title?: never;
    quiet?: never;
  };

/** A card's ways out: a fix, a link or its ×, never a command to copy (#3344). */
type CardWayOut =
  | (Ways & { copy?: never; fixes: readonly [NoticeAction, ...NoticeAction[]] })
  | (Ways & { copy?: never; link: NoticeAction })
  | (Ways & { copy?: never; onDismiss: () => void });

/**
 * **A card Notice** (#3344, the sidebar-views prototype's Q3 A): drawn as the family's card
 * (`layout="card"`): its mark at the start, its title with × at the top right, its sentence
 * (`children`) under the title, its ways on a row of their own under that, and what a way out
 * opened inside the card. Only the title and the sentence are its live region, so opening its
 * Details reads nothing out again. Never in a pane, whose row reads its line's live region, and
 * with no persona, label or command to copy: the title names it.
 */
type CardProps = CardWayOut &
  Said & {
    at?: "inbox";
    /** What happened, before its one sentence. */
    title: ReactNode;
    /** **Not a live region**: a copy of a Notice another one already said, as a result kept
     *  under Results is of the result's notice. */
    quiet?: boolean;
    label?: never;
    persona?: never;
  };

export type NoticeProps = LineProps | CardProps;

/**
 * **Whose Notice this is, for a Notice of a chat that is not the one its pane shows** (#1486).
 *
 * A session's tab shows one of its chats at a time, and every chat that lives in the tab says
 * what it has to say on that tab's pane: nothing that waits for the person may be off screen.
 * So a Notice of a chat that is not on screen says whose it is before its sentence, `steward
 * 4:` or, for a task, its whole path, `deep (a task of steward 4 › talk):` (#1508), and has
 * **Go to it**, which switches the tab to that
 * chat. Its ways out act on its own chat, as they always did: the Notice is that chat's, drawn
 * here. Nothing is provided for the chat on screen, whose Notices read as they always have.
 */
export const NoticeOf = createContext<{
  whose: string;
  onGo: () => void;
  /** Whether the chat is a task below the pane's session rather than the session's own chat:
   *  read off the core's lineage where `whose` is built, never off the name (#1601). */
  task?: boolean;
} | null>(null);

/** What an attention Notice says first to a reader, as its mark says it to the eye (#3678). */
const NEEDS_A_DECISION = "Needs a decision: ";

/** **The order of the tones in the Inbox's list**: trouble, then what needs a decision, then news. */
const TONE_ORDER: Record<NoticeTone, number> = { trouble: 0, attention: 1, news: 2 };

export function Notice(props: NoticeProps) {
  const {
    cause,
    tone = "news",
    at = "inbox",
    label,
    persona,
    children,
    under,
    title,
    quiet,
  } = props;
  const { fixes, link, copy, onDismiss, dismissSays } = props as Ways;
  const list = useContext(Listed);
  const provided = useContext(NoticeOf);
  // A granting way out waits for the list it is drawn in to say it has settled (#1695).
  const mayAct = useContext(NoticeWayGuard);
  // Only on a pane: a Notice a pane's chat sends to the Inbox is the project's.
  const of = at === "pane" ? provided : null;
  const stacked = list !== null && at === "inbox";
  const id = useId();
  // Where this Notice is drawn when the Inbox lists it: an element of its own, which the list
  // puts in its place, in importance order.
  const [host] = useState(() => document.createElement("div"));
  useLayoutEffect(
    () => (stacked ? list.register(id, { cause, tone, host }) : undefined),
    [list, cause, host, id, stacked, tone],
  );
  // The line is a row of its ways out that wraps (`ButtonRow`, #1855), every one small. Its box is
  // the notice family's root: in a pane the box itself, in the Inbox the list's row, and alone
  // (a Notice on a Settings page) the inline surface.
  const placement = at === "pane" ? "pane" : stacked ? "inbox" : "alone";
  if (title !== undefined) {
    const card = (
      <NoticeBox
        tone={tone}
        placement={placement}
        opened={under !== undefined}
        asks={fixes !== undefined || link !== undefined}
        layout="card"
      >
        <NoticeMark tone={tone} />
        <NoticeMessage live={quiet !== true} cause={cause}>
          {/* A refusal is told by more than its mark and its colour (WCAG 1.3.1, #3344). */}
          {tone === "trouble" && <span className="sr-only">Not done: </span>}
          {tone === "attention" && <span className="sr-only">{NEEDS_A_DECISION}</span>}
          <NoticeTitle>{title}</NoticeTitle>
          {children}
        </NoticeMessage>
        {onDismiss && (
          <IconButton
            size="sm"
            variant="ghost"
            aria-label="Dismiss"
            tooltip={dismissSays}
            onPress={onDismiss}
            data-way="dismiss"
            icon={<X aria-hidden="true" />}
          />
        )}
        {(fixes !== undefined || link !== undefined) && (
          <ButtonRow size="sm">
            {fixes?.map((fix) => (
              <Button
                key={fix.label}
                size="sm"
                variant={
                  fix.allows === true
                    ? "grants"
                    : fix.primary === true
                      ? "primary"
                      : fix.disclosure === true
                        ? "ghost"
                        : "secondary"
                }
                icon={
                  fix.disclosure !== true ? undefined : fix.opens?.open === true ? (
                    <ChevronDown aria-hidden="true" />
                  ) : (
                    <ChevronRight aria-hidden="true" />
                  )
                }
                data-way={fix.disclosure === true ? "disclosure" : "fix"}
                aria-expanded={fix.opens?.open}
                aria-controls={fix.opens?.open ? fix.opens.id : undefined}
                aria-disabled={fix.busy || fix.off !== undefined || undefined}
                aria-busy={fix.busy || undefined}
                tooltip={fix.off}
                onPress={
                  fix.busy || fix.off !== undefined
                    ? undefined
                    : (press) => {
                        if (mayAct(press.target)) fix.onPress();
                      }
                }
              >
                {fix.label}
              </Button>
            ))}
            {link && (
              <Button size="sm" onPress={link.onPress} data-way="link">
                {link.label}
              </Button>
            )}
          </ButtonRow>
        )}
        {under !== undefined && <NoticeUnder>{under}</NoticeUnder>}
      </NoticeBox>
    );
    return stacked ? createPortal(card, host) : card;
  }
  const line = (
    <ButtonRow
      size="sm"
      role="status"
      aria-label={of === null || label === undefined ? label : `${of.whose}: ${label}`}
      data-cause={cause}
    >
      {/* In a pane the mark is the sentence's first word, so it is never left alone on a row
          above a sentence that takes the next one whole (#1481). */}
      {persona != null && at !== "pane" && <PersonaMark persona={persona} />}
      <NoticeMessage>
        {/* Attention is told by its mark and its words, not its colour alone (#3678). In a
            chat's pane the band draws the mark itself (`paneFrame.css`). */}
        {tone === "attention" && at !== "pane" && <NoticeMark tone="attention" />}
        {tone === "attention" && <span className="sr-only">{NEEDS_A_DECISION}</span>}
        {persona != null && at === "pane" && <PersonaMark persona={persona} />}
        {of !== null && <NoticeWhose>{of.whose}: </NoticeWhose>}
        {children}
      </NoticeMessage>
      {/* The way to the chat it is about, first: its other ways out answer for that chat. */}
      {of !== null && (
        <Button size="sm" onPress={of.onGo} data-way="go">
          Go to it
        </Button>
      )}
      {fixes?.map((fix) => (
        <Button
          key={fix.label}
          size="sm"
          variant={fix.allows === true ? "grants" : "secondary"}
          data-way="fix"
          aria-expanded={fix.opens?.open}
          aria-controls={fix.opens?.open ? fix.opens.id : undefined}
          aria-disabled={fix.busy || fix.off !== undefined || undefined}
          aria-busy={fix.busy || undefined}
          tooltip={fix.off}
          onPress={
            fix.busy || fix.off !== undefined
              ? undefined
              : (press) => {
                  if (mayAct(press.target)) fix.onPress();
                }
          }
        >
          {fix.label}
        </Button>
      ))}
      {link && (
        <Button size="sm" onPress={link.onPress} data-way="link">
          {link.label}
        </Button>
      )}
      {copy !== undefined && (
        <Button
          size="sm"
          tooltip={copy}
          onPress={() => void navigator.clipboard?.writeText(copy).catch(() => undefined)}
          data-way="copy"
        >
          Copy command
        </Button>
      )}
      {onDismiss && (
        <Button
          size="sm"
          variant="ghost"
          tooltip={dismissSays}
          onPress={onDismiss}
          data-way="dismiss"
        >
          Dismiss
        </Button>
      )}
    </ButtonRow>
  );
  // What a way out opened is drawn after the line, never inside it: the line is a live region,
  // and a form in one would be read out again on every keystroke and every refusal. **The line
  // and what it opened are one box** (#1481): what keeps what a way out opened under its line
  // and at its width, whatever the pane's width; the Inbox's list moves the box whole.
  const drawn = (
    <NoticeBox
      tone={tone}
      placement={placement}
      opened={under !== undefined}
      // What the pane's row stands first (`NoticePaneRow`): a Notice with something to do
      // about it, before one whose only way out is to put it away.
      asks={fixes !== undefined || link !== undefined}
    >
      {line}
      {under !== undefined && <NoticeUnder>{under}</NoticeUnder>}
    </NoticeBox>
  );
  return stacked ? createPortal(drawn, host) : drawn;
}

/**
 * **How important a Notice is, by the family of its cause** (V91i): trouble, then attention, then
 * news ({@link TONE_ORDER}), then
 * this order, then the one drawn first. A family is the cause up to its first `:`; one not in
 * the list comes after every one that is.
 *
 * - an Undo that lasts a few seconds, before anything that waits;
 * - what the operator just did, before what charter found;
 * - what happened while the person was away (#1514, #1551), before everything else that waits:
 *   it is what they read first on coming back;
 * - an offer, before news about how things came back;
 * - a chat whose state the session host could not hand over, then one that lost its
 *   conversation, before one charter had to guess about, before one that came back as it was.
 */
export const IMPORTANCE: readonly string[] = [
  "window-trouble",
  "chat-did-not-start",
  "memory-deleted",
  "memory-moved",
  "pin-forgotten",
  "session-saved",
  "away-summary",
  "chat-quiet",
  "sandbox-offer",
  "sandbox-hosts",
  "sandbox-presets",
  "sandbox-expects",
  "vaults-waiting",
  "pin-dormant",
  "chat-untaken",
  "chat-fresh",
  "chat-guessed",
  "chat-resumed",
];

/** How many Notices stand in a pane's row; the rest are behind "+N more" (V91i, #1647). */
export const SHOWN = 2;

/** A pane Notice's own Dismiss, on its line (not one inside what a way out opened): by its
 *  class here, or by the way out it is once the notice family draws it (#2912). */
const DISMISS = ':scope > [role="status"] > :is(.notice-dismiss, [data-way="dismiss"])';

/** A cause's family: what it is up to the first `:` (`pin-dormant:ide` is a `pin-dormant`). */
export const familyOf = (cause: string): string => cause.split(":", 1)[0];

const rank = (cause: string) => {
  const at = IMPORTANCE.indexOf(familyOf(cause));
  return at < 0 ? IMPORTANCE.length : at;
};

/**
 * **The families of Notice the doctor's button counts already** (V91i): a doctor finding that
 * stands as a Notice (`DoctorNotices` in `Doctor.tsx`, #1250) is a `doctor-finding`. The status
 * line's Notices count leaves them out, so a Notice is never counted twice there.
 */
export const FROM_THE_DOCTOR: ReadonlySet<string> = new Set(["doctor-finding"]);

/**
 * **The families of Notice the ✋ stands for already** (#1695): a chat that cannot tell purlis
 * it is waiting, which the faint hand names. The status line's Notices count leaves it out, so
 * a shell left open is not a standing number there too.
 */
export const FROM_THE_HAND: ReadonlySet<string> = new Set(["chat-quiet"]);

/** Whether a Notice with this cause is counted by the status line's Notices button. */
export const countsInStatusBar = (cause: string): boolean =>
  !FROM_THE_DOCTOR.has(familyOf(cause)) && !FROM_THE_HAND.has(familyOf(cause));

/**
 * **The families whose arrival brings the Inbox on screen** (D-1695-3), where it is put away or
 * showing another view: each answers something the person just did. A refusal, an Undo of a few
 * seconds and a save's record answer a press; the summary of a time away answers their coming
 * back, and is the first thing they read then (#1514). A Notice that arrives on its own never
 * moves the window.
 */
export const ANSWERS: ReadonlySet<string> = new Set([
  "window-trouble",
  "memory-deleted",
  "memory-moved",
  "pin-forgotten",
  "session-saved",
  "away-summary",
]);

type Stacked = { cause: string; tone: NoticeTone; host: HTMLElement };
type Entry = Stacked & { seq: number };

/** What the Inbox's list is to the Notices drawn inside it. */
const Listed = createContext<{
  register: (id: string, notice: Stacked) => () => void;
} | null>(null);

/**
 * **A project's Notices, listed in its Inbox** (#1695, V91i): every `at="inbox"` Notice drawn
 * anywhere inside it, the most important first ({@link IMPORTANCE}), trouble before what needs
 * a decision before news. They
 * stood under the strip, two at a time with the rest behind "+N more", until the band folded
 * into the Inbox (spec #1688, I-4): there every one is listed, with its ways out.
 *
 * Each Notice is drawn into an element of its own, and the list only moves those elements: the
 * order is the list's, what each says is still its Notice's. **Only what is out of its place
 * moves**, so a button the person is on keeps the focus as another Notice arrives (F2).
 *
 * `onCount` hears how many it lists that the status line counts ({@link countsInStatusBar}),
 * and how many it lists in all;
 * `onAnswer` hears that a Notice of an {@link ANSWERS} family arrived;
 * `into` is where the list is drawn, when that is not where it is written: the window's own
 * lines are written once, and drawn at the top of the page a window with no project shows
 * (D-LB-1). Moving it there moves no Notice's state, since each stays where it is written.
 * `at` says which of those places it is drawn in (`NoticeListAt`), for its box.
 */
export function NoticeList({
  children,
  onCount,
  onAnswer,
  into,
  at = "inbox",
}: {
  children: ReactNode;
  onCount?: (counted: number, listed: number) => void;
  onAnswer?: () => void;
  into?: Element | null;
  at?: NoticeListAt;
}) {
  const [entries, setEntries] = useState<ReadonlyMap<string, Entry>>(() => new Map());
  // State, not a ref: the list's element changes when it is drawn somewhere else (`into`), and
  // the Notices are moved into the new one.
  const [listAt, setListAt] = useState<HTMLDivElement | null>(null);
  const next = useRef(0);

  const list = useMemo(
    () => ({
      register: (id: string, notice: Stacked) => {
        setEntries((was) =>
          new Map(was).set(id, { ...notice, seq: was.get(id)?.seq ?? next.current++ }),
        );
        return () =>
          setEntries((was) => {
            const without = new Map(was);
            without.delete(id);
            return without;
          });
      },
    }),
    [],
  );

  const ordered = useMemo(
    () =>
      [...entries.values()].sort(
        (a, b) =>
          TONE_ORDER[a.tone] - TONE_ORDER[b.tone] || rank(a.cause) - rank(b.cause) || a.seq - b.seq,
      ),
    [entries],
  );

  // Before the frame is painted, so a Notice is never seen out of its place.
  useLayoutEffect(() => {
    const at = listAt;
    if (at === null) return;
    const hosts = ordered.map((one) => one.host);
    for (const child of [...at.children]) if (!hosts.includes(child as HTMLElement)) child.remove();
    hosts.forEach((host, index) => {
      const there = at.children.item(index);
      if (there !== host) at.insertBefore(host, there);
    });
  }, [ordered, listAt]);

  const counted = ordered.filter((one) => countsInStatusBar(one.cause)).length;
  useEffect(() => onCount?.(counted, ordered.length), [onCount, counted, ordered.length]);

  /** The answers listed already, by cause: one arriving is one not listed before. */
  const answered = useRef(new Set<string>());
  useEffect(() => {
    const now = new Set(
      ordered.filter((one) => ANSWERS.has(familyOf(one.cause))).map((one) => one.cause),
    );
    const arrived = [...now].some((cause) => !answered.current.has(cause));
    answered.current = now;
    if (arrived) onAnswer?.();
  }, [ordered, onAnswer]);

  const drawn = <NoticeListBox at={at} ref={setListAt} hidden={ordered.length === 0} />;
  return (
    <Listed.Provider value={list}>
      {children}
      {into ? createPortal(drawn, into) : drawn}
    </Listed.Provider>
  );
}

/** **Whether a pane's row of Notices is drawn**: not while it holds none (the row's `data-empty`
 *  is `display: none`), which is how it starts, until it has counted what it holds (#3302). */
const rowDrawn = (row: HTMLElement) => !row.hasAttribute("data-empty");

/**
 * **A pane's Notices: a row of their own at the top of the pane, above its terminal** (#1647).
 *
 * The operator's screenshot (2026-10-10): three Notices stacked over a chat's terminal and hid
 * the conversation. They had been drawn in the pane's corner, over the terminal and taking no
 * row (#1481). Now they take a row, and the terminal gives that height up, so nothing purlis
 * says is ever drawn over what the chat wrote.
 *
 * **Two at a time** (V91i): the rest are behind **+N more**, which opens them in the row, with
 * their ways out, and closes on Escape or a press outside the row. The order is the pane's own, the order its Notices are written in (`ChatNotices`):
 * the one that waits for an answer first, then the hidden chats'. **The two that stand are the
 * first two that ask something** (a fix or a link, `data-asks`), and only then the first that
 * only say something (Dismiss alone): an "Allowed." the person can read later never keeps a
 * question that waits for them behind "+N more". Every Notice is drawn where it is written, and
 * the row only hides the others (`hidden`), so the order on screen is still the pane's, React
 * keeps each one's state, and nothing is moved out from under the keyboard.
 *
 * **The terminal is refitted on every change of the row's height** (its `ResizeObserver`), and
 * that resizes the chat's pty and redraws its harness. With two at most, the row's height
 * changes when a pane goes from none to one or from one to two, and when one that stands is
 * answered or opens something; a third and every one after it changes nothing but the count.
 *
 * **A Notice the keyboard is on is never hidden** (as the Inbox's list never moves one): one that
 * arrives above it stands as a third until the focus leaves it.
 */
export function NoticePaneRow({
  children,
  band,
}: {
  children: ReactNode;
  /** The pane this band is (`paneBand`): what stood in it is remembered when it is drawn anew. */
  band?: string;
}) {
  const stackAt = useRef<HTMLDivElement>(null);
  const moreAt = useRef<HTMLButtonElement>(null);
  // **A granting Notice's ways out wait for it to settle once it moved** (#1695, train 54's
  // review): a Notice above it answered or gone can move another's way out under the pointer.
  const { at: rowAt, allows, said } = useNoticeSettle<HTMLDivElement>("stands", band, rowDrawn);
  const [count, setCount] = useState(0);
  const [open, setOpen] = useState(false);
  const behind = Math.max(count - SHOWN, 0);
  // Nothing behind it: the list closes, so the next one to fall behind does not open it again.
  if (behind === 0 && open) setOpen(false);

  // Each Notice is one element of the stack (the notice family's root). Read off the stack itself, so
  // a Notice is counted however deep in the pane's components it is written.
  useLayoutEffect(() => {
    const stack = stackAt.current;
    if (stack === null) return;
    const place = () => {
      const boxes = [...stack.children] as HTMLElement[];
      setCount(boxes.length);
      // The pane's order, those that ask first: a stable sort keeps the pane's order in each.
      const asks = (box: HTMLElement) => (box.hasAttribute("data-asks") ? 0 : 1);
      const stand = new Set([...boxes].sort((a, b) => asks(a) - asks(b)).slice(0, SHOWN));
      const on = document.activeElement;
      for (const box of boxes) {
        const hide = !open && !stand.has(box) && !(on !== null && box.contains(on));
        if (box.hidden !== hide) box.hidden = hide;
      }
    };
    place();
    // A Notice that comes or goes, and one that comes to ask something or stops asking where it
    // stands (an answered question says what was done in the same box): either changes the two.
    // (`data-asks` is on each Notice's box, so the subtree is watched for that one attribute.)
    const watching = new MutationObserver(place);
    watching.observe(stack, { childList: true });
    const asking = new MutationObserver(place);
    asking.observe(stack, { subtree: true, attributeFilter: ["data-asks"] });
    // A Notice kept for the keyboard goes behind "+N more" once the keyboard has left it: after
    // the focus has moved, which is after `focusout`.
    let later: ReturnType<typeof setTimeout> | undefined;
    const left = () => {
      clearTimeout(later);
      later = setTimeout(place, 0);
    };
    stack.addEventListener("focusout", left);
    return () => {
      watching.disconnect();
      asking.disconnect();
      stack.removeEventListener("focusout", left);
      clearTimeout(later);
    };
  }, [open]);

  // **Esc puts away the warning the keyboard is on, and the keyboard goes on to the next**
  // (#3142, rule 14): to the warning that took its place, or to the pane's terminal when it was
  // the last. Only a warning with a Dismiss: a consent has none, and Esc leaves it standing
  // (rule 9). While "+N more" is open, Esc closes the list first (F3, below).
  useEffect(() => {
    const row = rowAt.current;
    const stack = stackAt.current;
    if (row === null || stack === null) return;
    const shown = () => [...stack.children].filter((box) => !(box as HTMLElement).hidden);
    const boxOf = (node: EventTarget | null) =>
      node instanceof Node ? [...stack.children].find((box) => box.contains(node)) : undefined;
    /** The warning the keyboard was last in, and where it stood among those shown. */
    let was: { box: Element; at: number } | undefined;
    const into = (event: FocusEvent) => {
      const box = boxOf(event.target);
      was = box === undefined ? undefined : { box, at: shown().indexOf(box) };
    };
    // A warning answered, dismissed or put away by Esc leaves the page with the focus on it:
    // the keyboard goes on from where it stood, never to the top of the window.
    const goOn = () => {
      if (was === undefined || was.box.isConnected) return;
      const at = was.at;
      was = undefined;
      const on = document.activeElement;
      if (on !== null && on !== document.body) return;
      const left = shown();
      const next = left[Math.min(at, left.length - 1)];
      // **Never onto an answer that grants** (ADR 0097 §6): a consent is landed on by its line,
      // as "Show the request" lands on it, so an Enter after a Dismiss allows nothing.
      const line = next?.querySelector<HTMLElement>(':scope > [role="status"]');
      if (line && next?.querySelector('[data-variant="grants"]')) {
        if (!line.hasAttribute("tabindex")) line.tabIndex = -1;
        line.focus();
        return;
      }
      const to =
        next?.querySelector<HTMLElement>("button:not([disabled])") ??
        row.closest(".pane-frame")?.querySelector<HTMLElement>(".xterm-helper-textarea");
      to?.focus();
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || open) return;
      // On the warning's own line only: Esc in what a way out opened (a field being written)
      // is that field's, and never puts the warning away.
      const box = boxOf(event.target);
      const line = event.target instanceof Element ? event.target.closest('[role="status"]') : null;
      if (box === undefined || line?.parentElement !== box) return;
      const dismiss = box.querySelector<HTMLElement>(DISMISS);
      if (!dismiss) return;
      event.preventDefault();
      event.stopPropagation();
      dismiss.click();
    };
    const watching = new MutationObserver(goOn);
    watching.observe(stack, { childList: true });
    stack.addEventListener("focusin", into);
    row.addEventListener("keydown", escape);
    return () => {
      watching.disconnect();
      stack.removeEventListener("focusin", into);
      row.removeEventListener("keydown", escape);
    };
  }, [open, rowAt]);

  // **The list closes on Escape and on a press outside the row** (F3).
  useEffect(() => {
    const row = rowAt.current;
    if (!open || row === null) return;
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      // The focus first, so the Notice it was on is not kept for it.
      moreAt.current?.focus();
      setOpen(false);
    };
    const outside = (event: PointerEvent) => {
      if (!row.contains(event.target as Node)) setOpen(false);
    };
    row.addEventListener("keydown", escape);
    document.addEventListener("pointerdown", outside);
    return () => {
      row.removeEventListener("keydown", escape);
      document.removeEventListener("pointerdown", outside);
    };
  }, [open, rowAt]);

  return (
    <NoticeRow
      empty={count === 0}
      // The pane's warnings, named as one place (#3142), while it has any.
      role={count === 0 ? undefined : "region"}
      aria-label={count === 0 ? undefined : "Warnings for this pane"}
      ref={rowAt}
    >
      <NoticeStack ref={stackAt}>
        <NoticeWayGuard value={allows}>{children}</NoticeWayGuard>
      </NoticeStack>
      {said !== undefined && (
        <p className="inbox-refused" role="status">
          {said}
        </p>
      )}
      {behind > 0 && (
        <Button
          size="sm"
          ref={moreAt}
          aria-expanded={open}
          onPress={() => setOpen((was) => !was)}
          data-way="more"
        >
          {/* Open, it says how to close it again (#3142). */}
          <NoticeMoreSays open={open} closed={`+${behind} more`} opened="Show fewer" />
        </Button>
      )}
    </NoticeRow>
  );
}
