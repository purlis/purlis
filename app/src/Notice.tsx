// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
  type ReactNode,
  type Ref,
} from "react";
import { createPortal } from "react-dom";
import { holdOrCarryOut, type DismissHold } from "./dismissHold";
import { useNow } from "./InboxRow";
import { toneOfNotice } from "./messageTone";
import { agoSaid } from "./inboxRules";
import { PersonaMark } from "./PersonaMark";
import {
  closedIn,
  closeIn,
  grants,
  reopenIn,
  NoticeWayGuard,
  useNoticeSettle,
} from "./noticeSettle";
import {
  bandHeadSays,
  bandRegionSays,
  dismissAllSays,
  foldedSays,
  HOLDS_NOTHING,
  sameHolds,
  type BandHolds,
} from "./paneNotices";
import {
  ChevronUp,
  CircleAlert,
  CircleCheck,
  CircleQuestionMark,
  LoaderCircle,
  Search,
  X,
} from "lucide-react";
import { Twist } from "./ui/disclosure/Disclosure";
import { TextField } from "./ui/field/Field";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { KEPT } from "./keptCauses";
import { aboutOf, kindOf, searchFinds } from "./noticeKinds";
import { controlReached, useAskedControl } from "./windowControls";
import {
  Button,
  ButtonRow,
  IconButton,
  ToggleButton,
  type ButtonVariant,
} from "./ui/button/Button";
import {
  MessageBody,
  MessageForm,
  MessageLinks,
  MessageMeta,
  MessageRow,
  MessageTop,
} from "./ui/message/Message";
import {
  NoticeBox,
  NoticeListBox,
  NoticeMark,
  NoticeMessage,
  NoticeRow,
  NoticeRowFolded,
  NoticeRowHead,
  NoticeSaid,
  NoticeStack,
  NoticeSub,
  NoticeTitle,
  NoticeUnder,
  NoticeWays,
  NoticeWaySlot,
  NoticeWhose,
  NoticeWords,
  usePinnedRoom,
  type NoticeListAt,
  type NoticeTone,
} from "./ui/notice/Notice";
import { noticeSeenKey, useSeenRow } from "./seenRows";

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
 * its mark, a question in a circle, and says "Needs a decision:" to a reader, so it is never told by colour
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
  /** The button itself, for a caller that moves the keyboard to it once the way out it pressed
   *  has gone (#4278, WCAG 2.4.3). */
  button?: Ref<HTMLButtonElement>;
  /** A card Notice's disclosure (its Details, #3344): a ghost button led by a chevron that
   *  points down while what it opens is open. Needs `opens`. */
  disclosure?: boolean;
  /** **The way to the chat it is about** (#5217): in the Notices centre it is Go to chat, a quiet
   *  link in the card's head. Elsewhere it is a way like any other. */
  go?: boolean;
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
  /** Work still under way: its mark says so, rather than a done check (#4079). */
  running?: boolean;
  /** The sentence (and anything else the line says before its ways out). */
  children: ReactNode;
  /** What a way out opened (a fix's form, #1250): drawn right after the line, outside its live
   *  region, and moved with it by the Inbox's list. */
  under?: ReactNode;
  /** **A consent surface** (ADR 0097 §6), though no way out on it says `allows`: a sandbox block
   *  not yet answered. In the Notices centre it keeps its full waiting edge and takes no primary
   *  way, so its answers stand at equal weight. It says it waits only where it has an answer. */
  consent?: boolean;
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
    /** **An ask, in a pane** (#4754): a blocking question the person answers, which stays a
     *  full card when the pane's notices are minimised, is never counted on its bell, and has
     *  no Dismiss beside its answers, so Dismiss all and Esc leave it standing. A consent (a
     *  way out that `allows`) is one whether or not it says so. */
    waits?: boolean;
    /** **A row's sub-line in a chat pane's band** (#5215): who asks or what it is, and its age.
     *  Without, its `label`. */
    sub?: ReactNode;
    /** **When it began to wait, in seconds**, where it is known: said as its age (#5215). */
    since?: number | null;
    /** **A row's icon, where its tone does not say what it is** (#5215): a prompt in the
     *  terminal. */
    mark?: "prompt";
    title?: never;
    quiet?: never;
    from?: never;
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
    /** **The chat it came from** (#5694): its name, drawn first in the card's head and named
     *  by its Go to chat. Data, drawn as text. */
    from?: string;
    /** **Not a live region**: a copy of a Notice another one already said, as a result kept
     *  under Results is of the result's notice. */
    quiet?: boolean;
    label?: never;
    persona?: never;
    sub?: never;
    since?: never;
    mark?: never;
  };

export type NoticeProps = LineProps | CardProps;

/**
 * **Whose Notice this is, for a Notice of a chat that is not the one its pane shows** (#1486).
 *
 * A pane shows one of its chats at a time, and every chat at home in the pane says
 * what it has to say on that pane: nothing that waits for the person may be off screen.
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

/**
 * **A pane's Notices, said in the Notices centre instead** (#4759, spec #4750): what a chat in
 * no pane has to say has no band to stand on, so its Notices are drawn as the centre's lines,
 * each saying whose it is and with Go to it ({@link NoticeOf}). The value is told, by cause,
 * when one comes and goes, so the chat's row can say it has something to read.
 */
export const NoticeInCentre = createContext<((cause: string, held: boolean) => void) | null>(null);

/** The open band's fold (#5215). */
export const FOLD_BAND = "Fold the band";
/** The folded band's line's last word (#5215). */
const SHOW_BAND = "Show";

/** What an attention Notice says first to a reader, as its mark says it to the eye (#3678). */
const NEEDS_A_DECISION = "Needs a decision: ";

/** **The order of the tones in the Inbox's list**: trouble, then what needs a decision, then news. */
const TONE_ORDER: Record<NoticeTone, number> = { trouble: 0, attention: 1, news: 2 };

export function Notice(props: NoticeProps) {
  const centre = useContext(NoticeInCentre);
  const {
    cause,
    tone = "news",
    running = false,
    at: askedAt = "inbox",
    label,
    persona,
    children,
    under,
    title,
    quiet,
    // Only a card names the chat it came from: a line's `from` is `never`.
    from,
  } = props;
  // A pane's Notice of a chat in no pane is the Notices centre's line (#4759).
  const inCentre = centre !== null && askedAt === "pane";
  const at = inCentre ? "inbox" : askedAt;
  const { fixes, link, copy, dismissSays } = props as Ways;
  // **An ask in a pane is answered, never put away** (#4754): it has no Dismiss beside an answer.
  const waits =
    at === "pane" &&
    ((props as { waits?: boolean }).waits === true || fixes?.some((fix) => fix.allows) === true);
  const onDismiss =
    waits && (fixes !== undefined || link !== undefined) ? undefined : (props as Ways).onDismiss;
  const list = useContext(Listed);
  const provided = useContext(NoticeOf);
  // A granting way out waits for the list it is drawn in to say it has settled (#1695).
  const mayAct = useContext(NoticeWayGuard);
  // Only on a pane, or a pane's said in the centre: a Notice a pane's chat sends to the Inbox
  // is the project's.
  const of = askedAt === "pane" ? provided : null;
  // **In a chat pane's band, a compact row** (#5215): the band says so, with how to close it here.
  const band = useContext(BandRows);
  const stands = band?.stands;
  // **Its band reads it in the commit it comes or goes in** (#3437), not a task later from what
  // its observer saw: so the band's room and its last warning go in one layout, and the
  // terminal is fitted once.
  useStandsInBand(at === "pane");
  useEffect(
    () => (at === "pane" && title === undefined ? stands?.(cause) : undefined),
    [at, title, stands, cause],
  );
  const stacked = list !== null && at === "inbox";
  const id = useId();
  // Where this Notice is drawn when the Inbox lists it: an element of its own, which the list
  // puts in its place, in importance order.
  const [host] = useState(() => document.createElement("div"));
  // Its own Dismiss, as the list carries it out for Dismiss all or after an Undo (#5650): the
  // latest one, read when it is carried out.
  const ownDismiss = useRef(onDismiss);
  useLayoutEffect(() => {
    ownDismiss.current = onDismiss;
  });
  const dismissable = onDismiss !== undefined;
  useLayoutEffect(
    () =>
      stacked
        ? list.register(id, {
            cause,
            tone,
            host,
            dismiss: dismissable ? () => ownDismiss.current : undefined,
          })
        : undefined,
    [list, cause, host, id, stacked, tone, dismissable],
  );
  // Its chat's row is told it has something to read while it stands (#4759).
  useLayoutEffect(() => {
    if (centre === null || askedAt !== "pane") return undefined;
    centre(cause, true);
    return () => centre(cause, false);
  }, [centre, cause, askedAt]);
  // The line is a row of its ways out that wraps (`ButtonRow`, #1855), every one small. Its box is
  // the notice family's root: in a pane the box itself, in the Inbox the list's row, and alone
  // (a Notice on a Settings page) the inline surface.
  const placement = at === "pane" ? "pane" : stacked ? "inbox" : "alone";
  // Standing in a list, it is one of the list's cards (#5217): the Notices centre's, and the
  // window's own lines, wherever they are drawn.
  if (stacked)
    return createPortal(
      <NoticeViewRow
        cause={cause}
        tone={tone}
        running={running}
        title={title}
        from={from}
        label={label}
        persona={persona}
        whose={of ?? undefined}
        quiet={quiet}
        since={(props as { since?: number | null }).since}
        consent={
          props.consent === true ||
          (props as { waits?: boolean }).waits === true ||
          fixes?.some((fix) => fix.allows) === true
        }
        fixes={fixes}
        link={link}
        copy={copy}
        onDismiss={onDismiss === undefined ? undefined : () => list.dismiss(id)}
        dismissSays={dismissSays}
        mayAct={mayAct}
        under={under}
      >
        {children}
      </NoticeViewRow>,
      host,
    );
  if (at === "pane" && band !== null && title === undefined) {
    if (band.closed.has(cause)) return null;
    return (
      <BandRow
        {...props}
        whose={of}
        waitsHere={waits}
        onDismiss={onDismiss}
        mayAct={mayAct}
        onClose={() => band.close(cause)}
      />
    );
  }
  if (title !== undefined) {
    const card = (
      <NoticeBox
        tone={tone}
        placement={placement}
        opened={under !== undefined}
        asks={fixes !== undefined || link !== undefined}
        layout="card"
      >
        <NoticeMark tone={tone} running={running} />
        <NoticeMessage live={quiet !== true} cause={cause}>
          {/* A refusal is told by more than its mark and its colour (WCAG 1.3.1, #3344). */}
          {tone === "trouble" && <span className="sr-only">Not done: </span>}
          {tone === "attention" && <span className="sr-only">{NEEDS_A_DECISION}</span>}
          <NoticeTitle>{title}</NoticeTitle>
          {from !== undefined && <NoticeWhose>{from}: </NoticeWhose>}
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
              <FixWay
                key={fix.label}
                fix={fix}
                mayAct={mayAct}
                variant={
                  fix.allows === true
                    ? "grants"
                    : fix.primary === true
                      ? "primary"
                      : fix.disclosure === true
                        ? "ghost"
                        : "secondary"
                }
              />
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
    return card;
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
        <FixWay key={fix.label} fix={fix} mayAct={mayAct} />
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
      asks={fixes !== undefined || link !== undefined || under !== undefined}
      waits={waits}
    >
      {line}
      {under !== undefined && <NoticeUnder>{under}</NoticeUnder>}
    </NoticeBox>
  );
  return drawn;
}

/** The longest label a Notices centre card takes for its one-line title. */
const TITLE_AT_MOST = 40;

/** What a Notices centre card says of itself under its title, in its tone. */
function stateSaid({
  tone,
  running,
  waitsOnYou,
  isCard,
}: {
  tone: NoticeTone;
  running: boolean;
  waitsOnYou: boolean;
  isCard: boolean;
}): string {
  if (waitsOnYou) return "Waiting on you";
  if (running) return "Working…";
  if (tone === "attention") return "Needs a decision";
  // A card answers what the person did; a line says what purlis found.
  if (tone === "trouble") return isCard ? "Not done" : "Problem";
  return "Note";
}

/**
 * **A Notice as a row of the Notices view** (#5649, ADR 0119 decision 6; a card of the Notices
 * centre before, #5217): the message family's row, as the view's other lines are. Its tone's
 * glyph in its state's tint; its title, with how long it has stood at the right and its × in that
 * place under the pointer; its state under it; what it says; its ways as text links (its fix,
 * Go to chat); and, for a consent, its answers as buttons in the row. A chat's own prose has no
 * part of its own here (#5363): no Notice in the view carries any, and Asks, which does, draws
 * it through `ChatText`.
 *
 * **One way leads, where it has an answer**: the way its caller leads with, else its first, in the
 * accent's words. **A consent takes no lead** (ADR 0097 §6): Allow keeps the `grants` look and
 * what stands beside it the plain one, at equal weight; only what opens more is a link.
 *
 * **Its title is the one thing it may not lack**: what it names (a card's title, whose it is, its
 * label), or else its state, which then is not said again under it. Its age is from when what it
 * says began where its caller knows that (`since`, #5702); else it is how long it has stood in this
 * window, so a relaunch starts it again.
 */
function NoticeViewRow({
  cause,
  tone,
  running,
  title,
  from,
  label,
  persona,
  whose,
  quiet,
  since: given,
  consent,
  fixes,
  link,
  copy,
  onDismiss,
  dismissSays,
  mayAct,
  under,
  children,
}: Ways & {
  cause: string;
  tone: NoticeTone;
  running: boolean;
  title?: ReactNode;
  from?: string;
  label?: string;
  persona?: string | null;
  whose?: { whose: string; onGo: () => void };
  quiet?: boolean;
  /** When what it says began, in seconds, where its caller knows (#5702). */
  since?: number | null;
  consent: boolean;
  mayAct: (target: Element) => boolean;
  under?: ReactNode;
  children: ReactNode;
}) {
  const [drawn] = useState(() => Date.now());
  const since = given == null ? drawn : given * 1000;
  // **New until seen** (#6002): its accent bar, until it has been in sight for about a second.
  const row = useRef<HTMLElement>(null);
  const [isNew] = useSeenRow(noticeSeenKey(cause), row);
  const now = useNow();
  const toChat = fixes?.find((fix) => fix.go === true);
  const others = fixes?.filter((fix) => fix !== toChat) ?? [];
  // A label as long as a sentence (the away summary's) names it for a reader, and is no title.
  const short = label !== undefined && label.length <= TITLE_AT_MOST ? label : undefined;
  const named = title ?? whose?.whose ?? short;
  // A consent with nothing left to press (purlis never allows it) waits on nobody.
  const waitsOnYou = consent && others.some((fix) => fix.opens === undefined);
  const state = stateSaid({ tone, running, waitsOnYou, isCard: title !== undefined });
  const answer = consent
    ? undefined
    : (others.find((fix) => fix.primary === true) ??
      others.find((fix) => fix.disclosure !== true && fix.opens === undefined));
  // **A consent's answers are buttons, in the row** (ADR 0097 §6): Allow keeps the `grants` look
  // and what stands beside it the plain one, at equal weight; only what opens more is a link.
  // Every other way is a text link (#5649), its one answer leading in the accent's words.
  const variantOf = (fix: NoticeAction): ButtonVariant => {
    if (fix.allows === true) return "grants";
    return consent && fix.opens === undefined ? "secondary" : "quiet-link";
  };
  const answers = consent ? others.filter((fix) => variantOf(fix) !== "quiet-link") : [];
  const links = others.filter((fix) => !answers.includes(fix));
  const goes = whose?.onGo ?? toChat?.onPress;
  const hasLinks =
    goes !== undefined || links.length > 0 || link !== undefined || copy !== undefined;
  return (
    // **A Notice as a row of its view** (#5649, ADR 0119 decision 6): its tone's glyph in its
    // state's tint; its title with how long it has stood at the right, the × in its place under
    // the pointer; its state; what it says; its ways as links (Go to chat, its fix); a
    // consent's answers and what a way out opened, in the row.
    <>
      {/* Its live words are the whole row, its × too, as its card's head held it; never what a
          way out opened, which stands under it, at its words' place. */}
      <NoticeSaid
        label={whose !== undefined && label !== undefined ? `${whose.whose}: ${label}` : label}
        cause={cause}
        live={quiet !== true}
      >
        <MessageRow
          as="div"
          ref={row}
          isNew={isNew}
          tone={toneOfNotice(tone, waitsOnYou || consent)}
          consent={consent}
          icon={running ? <LoaderCircle className="spinning" /> : <NoticeGlyph tone={tone} />}
          data-tone={tone}
          dismiss={
            onDismiss === undefined ? undefined : (
              <IconButton
                size="sm"
                variant="quiet"
                aria-label="Dismiss"
                tooltip={dismissSays}
                onPress={onDismiss}
                data-way="dismiss"
                icon={<X aria-hidden="true" />}
              />
            )
          }
        >
          {/* A refusal, and a decision, are told by more than their mark and colour (#3344). */}
          {tone === "trouble" && <span className="sr-only">Not done: </span>}
          {tone === "attention" && <span className="sr-only">{NEEDS_A_DECISION}</span>}
          <MessageTop
            title={
              <>
                {persona != null && <PersonaMark persona={persona} />}
                {named ?? state}
              </>
            }
            time={
              // Its age changes every minute: nothing to read the whole row out again for.
              <time aria-live="off" dateTime={new Date(since).toISOString()}>
                {agoSaid(since, now)}
              </time>
            }
          />
          {/* The chat a card came from, first in its meta (#5694). */}
          {(from !== undefined || named !== undefined) && (
            <MessageMeta>
              {from !== undefined && <span>{from}</span>}
              {from !== undefined && named !== undefined && " · "}
              {named !== undefined && state}
            </MessageMeta>
          )}
          {children !== undefined && (
            <MessageBody>
              <NoticeMessage>{children}</NoticeMessage>
            </MessageBody>
          )}
          {hasLinks && (
            <MessageLinks>
              {links.map((fix) => (
                <FixWay
                  key={fix.label}
                  fix={fix}
                  mayAct={mayAct}
                  variant="quiet-link"
                  lead={fix === answer}
                />
              ))}
              {link && (
                <Button size="sm" variant="quiet-link" onPress={link.onPress} data-way="link">
                  {link.label}
                </Button>
              )}
              {copy !== undefined && (
                <Button
                  size="sm"
                  variant="quiet-link"
                  tooltip={copy}
                  onPress={() => void navigator.clipboard?.writeText(copy).catch(() => undefined)}
                  data-way="copy"
                >
                  Copy command
                </Button>
              )}
              {goes !== undefined && (
                <Button
                  size="sm"
                  variant="quiet-link"
                  aria-label={
                    whose !== undefined
                      ? `Go to chat ${whose.whose}`
                      : from !== undefined
                        ? `Go to chat ${from}`
                        : "Go to chat"
                  }
                  onPress={(press) => {
                    if (whose !== undefined) whose.onGo();
                    else if (mayAct(press.target)) toChat?.onPress();
                  }}
                  data-way="go"
                >
                  Go to chat
                </Button>
              )}
            </MessageLinks>
          )}
          {answers.length > 0 && (
            <MessageForm>
              <ButtonRow size="sm">
                {answers.map((fix) => (
                  <FixWay key={fix.label} fix={fix} mayAct={mayAct} variant={variantOf(fix)} />
                ))}
              </ButtonRow>
            </MessageForm>
          )}
        </MessageRow>
      </NoticeSaid>
      {under !== undefined && <NoticeUnder>{under}</NoticeUnder>}
    </>
  );
}

/** **A Notice's glyph** in its row's mark (#5649): the icons its card drew (`NoticeMark`). */
function NoticeGlyph({ tone }: { tone: NoticeTone }) {
  const Glyph =
    tone === "trouble" ? CircleAlert : tone === "attention" ? CircleQuestionMark : CircleCheck;
  return <Glyph />;
}

/** **A way out that fixes**, on a pane's line or row, and on a decision's row in Asks (#5998):
 *  Allow in its own look, and every press asks the place it stands in first (`mayAct`, #1695). */
export function FixWay({
  fix: { button, label, allows, opens, busy, off, onPress, disclosure },
  mayAct,
  variant = allows === true ? "grants" : "secondary",
  lead = false,
}: {
  fix: NoticeAction;
  mayAct: (way: Element) => boolean;
  /** **The way a message row leads with** (#5649): its one answer, in the accent's words. */
  lead?: boolean;
  /** Its look where it stands: a card weighs its ways (#3478, #5217); a line or a row draws each
   *  as an answer that grants or a plain one. */
  variant?: ButtonVariant;
}) {
  return (
    <Button
      ref={button}
      size="sm"
      variant={variant}
      icon={disclosure !== true ? undefined : <Twist open={opens?.open === true} />}
      data-way={disclosure === true ? "disclosure" : "fix"}
      data-lead={lead ? "" : undefined}
      aria-expanded={opens?.open}
      aria-controls={opens?.open ? opens.id : undefined}
      aria-disabled={busy || off !== undefined || undefined}
      aria-busy={busy || undefined}
      tooltip={off}
      onPress={
        busy || off !== undefined
          ? undefined
          : (press) => {
              if (mayAct(press.target)) onPress();
            }
      }
    >
      {label}
    </Button>
  );
}

/** What a toggle says, by whether the row's message is opened (#5215). */
export const SHOW_WHOLE = "Show the whole message";
export const SHOW_LESS = "Show less";
/** What a row's × says (#5215): it hides it from this pane and answers nothing. */
export const CLOSE_ROW = "Close";
export const CLOSE_ASK_SAYS = "Close here. It stays in the Inbox until you answer it.";

/**
 * **The band's rows** (#5215), given by a chat pane's band to the Notices it holds: which it was
 * told to close here, and how to close one. None in any other place a Notice stands, each of
 * which draws its line as it did.
 */
const BandRows = createContext<{
  closed: ReadonlySet<string>;
  close: (cause: string) => void;
  /** Says a Notice of this cause stands, and returns how to say it has gone: once none does, its
   *  cause is no longer closed, so the same ask asked again is seen. */
  stands: (cause: string) => () => void;
} | null>(null);

/** **How a pane's Notice tells its band it came or went** (#3437), in the commit that draws it or
 *  takes it away. An update made in a commit is rendered before the frame is laid out, so the
 *  band reads its stack then, where its observers would read it a task later, after a frame
 *  that drew the band with nothing in it. None outside a band. */
const BandChanged = createContext<(() => void) | null>(null);

/** **A row of a pane's band says when it comes and goes** (#3437), in the commit that draws it or
 *  takes it away, so the band reads its stack before the frame is laid out (`BandChanged`). A
 *  `Notice` says so itself; a row the band holds that is no Notice (a task's question,
 *  `QuizBand`) calls this while `drawn`. Nothing outside a band. */
export function useStandsInBand(drawn: boolean) {
  const reread = useContext(BandChanged);
  useLayoutEffect(() => {
    if (!drawn || reread === null) return undefined;
    reread();
    return reread;
  }, [drawn, reread]);
}

/** **What this band has closed, and how to close**, for what draws a part of it itself (a task's
 *  question, `QuizBand`); none outside a chat's band. */
export const useBandRows = () => useContext(BandRows);

/** **Whether its one line cuts what it says**, measured as it is laid out and as its width changes:
 *  what a row's toggle is offered on. */
function useCut(whole: boolean) {
  const at = useRef<HTMLDivElement>(null);
  const [cut, setCut] = useState(false);
  useLayoutEffect(() => {
    const words = at.current;
    if (words === null || whole) return;
    const measure = () => setCut(words.scrollHeight > words.clientHeight);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const watch = new ResizeObserver(measure);
    watch.observe(words);
    return () => watch.disconnect();
  }, [whole]);
  return { at, cut };
}

/**
 * **A Notice as one compact row of a chat pane's band** (#5215, the person's approved prototype):
 * its icon, what it says on one line with its sub-line muted beneath, its own ways out, a toggle
 * where the line cuts it, and a × last.
 *
 * - **× never answers.** A note's is its Dismiss. An ask that may not be put away (`waits`) is
 *   closed from this pane alone, so it stays in the Inbox and on the ✋. A note with no Dismiss
 *   has no × (#5311): nothing else lists it, so closing it would lose it.
 * - **Opened, its message shows whole**, and its ways out are pinned at its foot (`data-whole`,
 *   `paneFrame.css`). Esc folds it again, from the band (`NoticePaneRow`).
 * - **A consent's edge and mark hold**, and Allow keeps its look and its settle guard
 *   (ADR 0097 §6): the row draws them with the same `Button` and the same `mayAct`.
 */
function BandRow({
  cause,
  tone = "news",
  running = false,
  label,
  persona,
  children,
  under,
  sub,
  since,
  mark,
  fixes,
  link,
  copy,
  dismissSays,
  whose,
  waitsHere,
  onDismiss,
  mayAct,
  onClose,
}: Said &
  Omit<Ways, "onDismiss"> &
  Pick<LineProps, "label" | "persona" | "sub" | "since" | "mark"> & {
    whose: { whose: string; onGo: () => void } | null;
    waitsHere: boolean;
    onDismiss?: () => void;
    mayAct: (way: Element) => boolean;
    onClose: () => void;
  }) {
  const [whole, setWhole] = useState(false);
  const { at, cut } = useCut(whole);
  // Pinned at its foot, opened, its ways keep the keyboard clear of them (WCAG 2.4.11).
  const waysAt = useRef<HTMLDivElement>(null);
  usePinnedRoom(waysAt, whole);
  const asksConsent = fixes?.some((fix) => fix.allows) === true;
  const holdsStill = asksConsent || grants(cause);
  const kind = mark ?? (asksConsent ? "consent" : tone === "news" && !running ? "note" : undefined);
  const now = useNow();
  const age = since == null ? undefined : agoSaid(since * 1000, now);
  const what = sub ?? label;
  const says =
    what == null || age === undefined ? (
      (what ?? age)
    ) : (
      <>
        {what} · {age}
      </>
    );
  return (
    <NoticeBox
      tone={tone}
      placement="pane"
      layout="row"
      opened={under !== undefined}
      asks={fixes !== undefined || link !== undefined || under !== undefined}
      waits={waitsHere}
      whole={whole}
      what={label}
      consent={holdsStill}
    >
      <ButtonRow
        size="sm"
        role="status"
        aria-label={whose === null || label === undefined ? label : `${whose.whose}: ${label}`}
        data-cause={cause}
      >
        <NoticeMark tone={tone} running={running} kind={kind} />
        <NoticeWords>
          <NoticeMessage ref={at}>
            {tone === "attention" && <span className="sr-only">{NEEDS_A_DECISION}</span>}
            {persona != null && <PersonaMark persona={persona} />}
            {whose !== null && <NoticeWhose>{whose.whose}: </NoticeWhose>}
            {children}
          </NoticeMessage>
          {says != null && <NoticeSub>{says}</NoticeSub>}
        </NoticeWords>
        <NoticeWays ref={waysAt}>
          {whose !== null && (
            <Button size="sm" onPress={whose.onGo} data-way="go">
              Go to it
            </Button>
          )}
          {fixes?.map((fix) => (
            <FixWay key={fix.label} fix={fix} mayAct={mayAct} />
          ))}
          {link && (
            <Button size="sm" variant="link" onPress={link.onPress} data-way="link">
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
          {cut || whole ? (
            <IconButton
              size="sm"
              variant="ghost"
              aria-label={whole ? SHOW_LESS : SHOW_WHOLE}
              aria-expanded={whole}
              onPress={() => setWhole((was) => !was)}
              data-way="whole"
              icon={<Twist open={whole} />}
            />
          ) : (
            // **A way out the settle guard holds never moves for the toggle** (#1695): the line is
            // measured only once it is drawn, so its room is kept from the first drawing.
            holdsStill && <NoticeWaySlot />
          )}
          {onDismiss ? (
            <IconButton
              size="sm"
              variant="ghost"
              aria-label="Dismiss"
              tooltip={dismissSays}
              onPress={onDismiss}
              data-way="dismiss"
              icon={<X aria-hidden="true" />}
            />
          ) : waitsHere ? (
            <IconButton
              size="sm"
              variant="ghost"
              aria-label={CLOSE_ROW}
              tooltip={CLOSE_ASK_SAYS}
              onPress={onClose}
              data-way="close"
              icon={<X aria-hidden="true" />}
            />
          ) : (
            // **A note with no Dismiss has no other home** (#5311): nothing else lists it, so no ×
            // hides it. It stands until its cause goes, its ways out where every row's are.
            <NoticeWaySlot />
          )}
        </NoticeWays>
      </ButtonRow>
      {under !== undefined && <NoticeUnder>{under}</NoticeUnder>}
    </NoticeBox>
  );
}

/**
 * **How important a Notice is, by the family of its cause** (V91i): after the families listed
 * {@link AHEAD} of all, trouble, then attention, then news ({@link TONE_ORDER}), then
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

/** A pane Notice's own Dismiss, on its line (not one inside what a way out opened): by its
 *  class here, or by the way out it is once the notice family draws it (#2912). */
const DISMISS = ':scope > [role="status"] :is(.notice-dismiss, [data-way="dismiss"])';

/** A cause's family: what it is up to the first `:` (`pin-dormant:ide` is a `pin-dormant`). */
export const familyOf = (cause: string): string => cause.split(":", 1)[0];

/**
 * **The families listed before every other Notice, whatever its tone** (#5702): what has appeared
 * in a chat's folders and can run later (ADR 0067 §5). It is told after the turn that caused it,
 * and the person has to find it then, before they or a tool work in that folder: listed by its
 * tone, it stood below the project's older Notices, out of sight.
 */
export const AHEAD: ReadonlySet<string> = new Set(["arrived-files"]);

const ahead = (cause: string) => (AHEAD.has(familyOf(cause)) ? 0 : 1);

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

/**
 * **The Notices icon's dot** (#5645; ADR 0121 decision 3, #6004): `trouble`, red, while a Notice
 * listed says something is broken; else `attention`, amber, while one asks a decision that stops
 * no chat; else none. News alone draws no dot: it is New, and its pip says so.
 */
export type NoticesDot = "trouble" | "attention" | undefined;

/** The dot the Notices of `tones` draw on the Notices icon ({@link NoticesDot}). */
export const dotOf = (tones: readonly NoticeTone[]): NoticesDot =>
  tones.includes("trouble") ? "trouble" : tones.includes("attention") ? "attention" : undefined;

/** Whether a Notice with this cause is counted by the status line's Notices button. */
export const countsInStatusBar = (cause: string): boolean =>
  !FROM_THE_DOCTOR.has(familyOf(cause)) && !FROM_THE_HAND.has(familyOf(cause));

/**
 * **The families whose arrival brings the Inbox on screen** (D-1695-3), where it is put away or
 * showing another view: each answers the press the person just made, at once. A refusal and an
 * Undo of a few seconds do. A Notice that arrives on its own never moves the window (ADR 0121:
 * nothing but a Blocking ask interrupts), and neither does one that comes later than its press.
 * A save's record lands when a chat has written it, minutes after Smart close or with no press
 * at all, as a task closes with the chat that asked for it. The summary of a time away arrives
 * on every return. Both opened Notices over whatever the person had open, so they are only
 * listed.
 */
export const ANSWERS: ReadonlySet<string> = new Set([
  "window-trouble",
  "memory-deleted",
  "memory-moved",
  "pin-forgotten",
]);

/**
 * **The lines whose own Dismiss stands no Undo** (#5650): each answers something the person
 * did ({@link ANSWERS}, a save's record, the summary of a time away) or is an Undo itself (a
 * layout change's), and goes by itself. Put away, it is gone at once, as it was; Dismiss all
 * still takes it with the rest, under one Undo.
 */
const GOES_AT_ONCE: ReadonlySet<string> = new Set([
  ...ANSWERS,
  "session-saved",
  "away-summary",
  "layout-changed",
]);

type Stacked = {
  cause: string;
  tone: NoticeTone;
  host: HTMLElement;
  /** Its own Dismiss as it is now, where it has one: taken when pressed, so a Dismiss held for
   *  its Undo is the one of the cause that was shown. */
  dismiss?: () => (() => void) | undefined;
};
type Entry = Stacked & { seq: number };

/** What the Inbox's list is to the Notices drawn inside it. */
const Listed = createContext<{
  register: (id: string, notice: Stacked) => () => void;
  /** The Notice `id`'s own Dismiss was pressed: carried out now, or held for its Undo. */
  dismiss: (id: string) => void;
} | null>(null);

/**
 * **Dismiss all on a list of Notices** (#5650): how many of its Notices have a Dismiss of their
 * own, and the press that does each one's, by its own rule. A Notice without one (an ask, a
 * teammate's change read by its Got it alone) is left standing.
 */
export type NoticesClear = {
  count: number;
  /** Taken at the press: the keys the Notices are held by, where Dismiss all waits for its
   *  Undo, and each one's own Dismiss, done when `out` is called. */
  take: () => { keys: readonly string[]; out: (still?: ReadonlySet<string>) => void };
};

/** The key a Notice is held by in a {@link DismissHold}: its place in the list and its cause,
 *  so a new cause in the same place shows at once. */
const heldKey = (id: string, cause: string) => `notice:${id}:${cause}`;

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
 * how many it lists in all, and the Notices icon's dot ({@link NoticesDot});
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
  hold,
  onClear,
  onCauses,
  findable,
}: {
  children: ReactNode;
  /** **The Notices view's list, made findable** (Q182, Q183): drawn under its search box and
   *  its filters by kind, grouped by kind, and with Dismissed (N), which lists what is dismissed
   *  here and brings any of it back ({@link NoticesFindable}). */
  findable?: NoticesFindable;
  onCount?: (counted: number, listed: number, dot: NoticesDot) => void;
  /** Hears the causes of the Notices it lists, whenever they change: what is New of them
   *  (#6002) is counted where its view's icon is drawn. */
  onCauses?: (causes: readonly string[]) => void;
  onAnswer?: () => void;
  into?: Element | null;
  at?: NoticeListAt;
  /** **Where a Dismiss waits for its Undo** (#5650): a Notice dismissed here, alone or by Dismiss
   *  all, is taken out of the list at once and its own Dismiss carried out when the Undo goes.
   *  Without it, a Dismiss is carried out at once. */
  hold?: DismissHold;
  /** Hears what Dismiss all on this list would dismiss now: none while no Notice has a
   *  Dismiss of its own. */
  onClear?: (clear: NoticesClear | undefined) => void;
}) {
  const [entries, setEntries] = useState<ReadonlyMap<string, Entry>>(() => new Map());
  // State, not a ref: the list's element changes when it is drawn somewhere else (`into`), and
  // the Notices are moved into the new one.
  const [listAt, setListAt] = useState<HTMLDivElement | null>(null);
  const next = useRef(0);
  /** The Notices registered now, read when a Dismiss is pressed. */
  const registered = useRef(entries);
  const holding = useRef(hold);
  useLayoutEffect(() => {
    registered.current = entries;
    holding.current = hold;
  });

  /** The keys of the Notices `ids` as they stand now, and their own Dismiss, done when called. */
  const outOf = useCallback((ids: readonly string[]) => {
    const now = ids.flatMap((id) => {
      const one = registered.current.get(id);
      return one === undefined ? [] : [{ key: heldKey(id, one.cause), own: one.dismiss?.() }];
    });
    return {
      keys: now.map((one) => one.key),
      // Each one's own, unless it was brought back while its Undo stood (`still` holds it not).
      out: (still?: ReadonlySet<string>) =>
        now.forEach((one) => (still === undefined || still.has(one.key)) && one.own?.()),
    };
  }, []);
  /** Does the Dismiss of each Notice of `ids`: held for its Undo where the list has a hold. */
  const dismissEach = useCallback(
    (ids: readonly string[]) => {
      const { keys, out } = outOf(ids);
      holdOrCarryOut(holding.current, keys, out);
    },
    [outOf],
  );

  const list = useMemo(
    () => ({
      dismiss: (id: string) => {
        const one = registered.current.get(id);
        // A line that answers a press and goes by itself is put away at once: no Undo of an Undo.
        if (one !== undefined && GOES_AT_ONCE.has(familyOf(one.cause))) outOf([id]).out();
        else dismissEach([id]);
      },
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
    [dismissEach, outOf],
  );

  const held = hold?.held;
  const ordered = useMemo(
    () =>
      [...entries]
        .filter(([id, one]) => held?.has(heldKey(id, one.cause)) !== true)
        .map(([, entry]) => entry)
        .sort(
          (a, b) =>
            ahead(a.cause) - ahead(b.cause) ||
            TONE_ORDER[a.tone] - TONE_ORDER[b.tone] ||
            rank(a.cause) - rank(b.cause) ||
            a.seq - b.seq,
        ),
    [entries, held],
  );

  /** The Notices dismissed here and waiting for their Undo whose dismissal purlis keeps: they
   *  are counted under Dismissed (N) from the press, not from when the Undo lapses (Q234). */
  const heldKept = useMemo(
    () =>
      [...entries]
        .filter(
          ([id, one]) =>
            held?.has(heldKey(id, one.cause)) === true && KEPT.has(familyOf(one.cause)),
        )
        .map(([, one]) => one.cause),
    [entries, held],
  );
  /** Show again on a Notice held for its Undo brings back that one alone (R-334). */
  const release = hold?.release;
  const showHeld = useCallback(
    (cause: string) => {
      for (const [id, one] of entries)
        if (one.cause === cause && held?.has(heldKey(id, cause)) === true)
          release?.(heldKey(id, cause));
    },
    [entries, held, release],
  );
  const found = useFinding(ordered, findable, listAt, heldKept, showHeld);
  const all = useMemo(() => ordered.map((one) => one.host), [ordered]);
  const hosts = found?.hosts ?? all;

  /** The Notices listed now that have a Dismiss of their own, and that the filter and the
   *  search leave drawn: Dismiss all puts away only what is found. */
  const clearable = useMemo(() => {
    const drawn = new Set(hosts);
    return [...entries]
      .filter(
        ([id, one]) =>
          one.dismiss !== undefined &&
          held?.has(heldKey(id, one.cause)) !== true &&
          drawn.has(one.host),
      )
      .map(([id]) => id);
  }, [entries, held, hosts]);
  useEffect(
    () =>
      onClear?.(
        clearable.length === 0
          ? undefined
          : { count: clearable.length, take: () => outOf(clearable) },
      ),
    [onClear, clearable, outOf],
  );

  // Before the frame is painted, so a Notice is never seen out of its place.
  useLayoutEffect(() => {
    const at = listAt;
    if (at === null) return;
    const kept = new Set<Element>(hosts);
    for (const child of [...at.children]) if (!kept.has(child)) child.remove();
    hosts.forEach((host, index) => {
      const there = at.children.item(index);
      if (there !== host) at.insertBefore(host, there);
    });
  }, [hosts, listAt]);
  // **A control a message asked for** (R-333), once its Notice is in its place above.
  const reach = found?.reach;
  useLayoutEffect(() => reach?.());

  const counted = ordered.filter((one) => countsInStatusBar(one.cause)).length;
  const dot = dotOf(ordered.map((one) => one.tone));
  useEffect(() => onCount?.(counted, ordered.length, dot), [onCount, counted, ordered.length, dot]);
  const causes = ordered.map((one) => one.cause).join("\n");
  useEffect(() => onCauses?.(causes === "" ? [] : causes.split("\n")), [onCauses, causes]);

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

  const box = <NoticeListBox at={at} ref={setListAt} hidden={hosts.length === 0} />;
  const drawn =
    found === undefined ? (
      box
    ) : (
      <>
        {found.bar}
        {box}
        {found.after}
      </>
    );
  return (
    <Listed.Provider value={list}>
      {children}
      {into ? createPortal(drawn, into) : drawn}
    </Listed.Provider>
  );
}

/**
 * **What the Notices view's list finds by** (Q182, Q183): the project it lists (a Go to asks one
 * project's view, `windowControls.ts`), the causes dismissed in it (`dismissals.ts`), and the way
 * to show one again.
 */
export type NoticesFindable = {
  project: string;
  dismissed: ReadonlySet<string>;
  showAgain: (cause: string) => void;
};

/** The label of the Dismissed filter, with how many are dismissed (Q183). */
export const dismissedSays = (count: number): string => `Dismissed (${count})`;

/** **What Dismissed (N) lists** (Q234, R-334 as narrowed): the Notices whose dismissal purlis
 *  keeps until their cause changes. Any other comes back by itself when it happens again. */
export const DISMISSED_LISTS =
  "Listed here: what purlis keeps dismissed until it changes. Any other notice you dismiss shows again the next time it happens.";

/** What the Dismissed list says while it lists none. */
export const NONE_DISMISSED = "Nothing is kept dismissed here";

/** The search box's label and its placeholder: one, so what is read is what is shown. */
export const SEARCH_THE_NOTICES = "Search the notices";

/** The line a search that finds no Notice draws, as the window's other searches say it. */
export const noNoticeMatches = (typed: string): string => `No notice matches “${typed}”.`;

/** What the filters show: every kind, one kind (by its `many`), or what is dismissed. */
type Showing = { all: true } | { kind: string } | { dismissed: true };

const ALL: Showing = { all: true };
const DISMISSED: Showing = { dismissed: true };
const same = (a: Showing, b: Showing) =>
  ("all" in a && "all" in b) ||
  ("dismissed" in a && "dismissed" in b) ||
  ("kind" in a && "kind" in b && a.kind === b.kind);

/**
 * **A Notice's words, as a search reads them**: what it says, not its buttons' labels, nor what
 * is hidden (a closed Details), so "trust" finds the Notices that say it, not every one with
 * Trust on a button.
 */
function wordsOf(host: HTMLElement): string {
  const copy = host.cloneNode(true) as HTMLElement;
  for (const gone of copy.querySelectorAll('button, [hidden], [aria-hidden="true"]')) gone.remove();
  return copy.textContent ?? "";
}

/** `text`, as it reads inside a regular expression. */
const literally = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/**
 * **Whether `names`, a message's words, names what the Notice of `cause` is about** (R-333): its
 * folder by its path (`workspaces/smart-ide/repos/web`, or its last parts from a slash on,
 * `repos/web`), not inside a longer one; or its exact name in quotes or code (“web”, `web`).
 * Never by its bare name as a word: a folder named "clone" or "save" is in every message that
 * says "Trust this clone" or "save again".
 */
function namesIt(names: string, cause: string): boolean {
  const parts = aboutOf(cause)
    .split("/")
    .filter((part) => part !== "");
  if (parts.length === 0) return false;
  const byPath = parts
    .slice(0, -1)
    .map((_, at) => parts.slice(at).join("/"))
    .some((path) => new RegExp(`(^|[^\\w./-])${literally(path)}(?![\\w/-])`).test(names));
  const name = literally(parts[parts.length - 1]);
  return byPath || new RegExp(`["'“‘\`]${name}["'”’\`]`).test(names);
}

/**
 * **A findable list's parts** (Q182, Q183; R-331 to R-334): the search box and the filters over
 * the list, the Notices it lists in their groups with each group's head before them (`hosts`,
 * in their order), and what stands under it: the line that says a search found nothing, or the
 * Dismissed list. `reach` puts the keyboard on a control a message asked for, once its Notice
 * is drawn (`windowControls.ts`, R-333). `undefined` for a list that is not findable.
 *
 * Groups take the place of their first Notice in the list's order, so the most important kind
 * leads; within one, the list's order holds. A search finds a Notice by its words
 * ({@link wordsOf}), and a dismissed one by its kind and what it was about.
 *
 * `heldKept` are the causes dismissed here and waiting for their Undo, listed under Dismissed
 * from the press; Show again on one brings back that one alone (`showHeld`), the rest still
 * waiting for the same Undo.
 */
function useFinding(
  ordered: readonly Entry[],
  findable: NoticesFindable | undefined,
  listAt: HTMLElement | null,
  heldKept: readonly string[],
  showHeld: (cause: string) => void,
):
  | { hosts: readonly HTMLElement[]; bar: ReactNode; after: ReactNode; reach: () => void }
  | undefined {
  const [query, setQuery] = useState("");
  const [showing, setShowing] = useState<Showing>(ALL);
  /** Each group's head is drawn into an element of its own, which the list moves as a Notice. */
  const [heads] = useState(() => new Map<string, HTMLElement>());
  const asked = useAskedControl(findable?.project);
  const dismissedAt = useRef<HTMLUListElement>(null);
  const dismissedFilter = useRef<HTMLButtonElement>(null);
  /** Where the keyboard goes once a Show again's row has gone: the row now at that place. */
  const refocusAt = useRef<number | undefined>(undefined);

  const kept = findable?.dismissed;
  const dismissed = useMemo(() => [...new Set([...(kept ?? []), ...heldKept])], [kept, heldKept]);
  const typed = query.trim();

  // **Where a Go to lands** (R-333, never on a blank view): its kind while a Notice of that
  // family stands; Dismissed while one is dismissed; else every kind. Shown at once, the search
  // let go, as React's "information from previous renders" pattern does, so no frame draws
  // another kind.
  const target: Showing | undefined =
    findable === undefined || asked === undefined
      ? undefined
      : ordered.some((one) => familyOf(one.cause) === asked.control.family)
        ? { kind: kindOf(`${asked.control.family}:`).many }
        : dismissed.some((cause) => familyOf(cause) === asked.control.family)
          ? DISMISSED
          : ALL;
  if (target !== undefined && !same(showing, target)) setShowing(target);
  if (target !== undefined && query !== "") setQuery("");
  // **A kind picked that empties falls back to All** (R-332), always: its last Notice mended or
  // dismissed leaves no filter of its kind, so the list shows every kind, never a blank view.
  const picked = "kind" in showing ? showing.kind : undefined;
  if (picked !== undefined && !ordered.some((one) => kindOf(one.cause).many === picked))
    setShowing(ALL);

  const groups = useMemo(() => {
    const made: { many: string; members: Entry[] }[] = [];
    for (const one of ordered) {
      const many = kindOf(one.cause).many;
      const group = made.find((each) => each.many === many);
      if (group === undefined) made.push({ many, members: [one] });
      else group.members.push(one);
    }
    return made;
  }, [ordered]);
  const drawn = useMemo(
    () =>
      groups
        .filter(
          (group) =>
            !("dismissed" in showing) && !("kind" in showing && group.many !== showing.kind),
        )
        .map((group) => ({
          ...group,
          found: group.members.filter((one) => searchFinds(wordsOf(one.host), typed)),
        }))
        .filter((group) => group.found.length > 0),
    [groups, showing, typed],
  );
  const hosts = useMemo(() => {
    const headOf = (many: string) => {
      const there = heads.get(many);
      if (there !== undefined) return there;
      const made = document.createElement("div");
      heads.set(many, made);
      return made;
    };
    return drawn.flatMap((group) => [headOf(group.many), ...group.found.map((one) => one.host)]);
  }, [drawn, heads]);
  const listed = dismissed.filter((cause) => {
    const kind = kindOf(cause);
    return searchFinds(`${kind.one} ${kind.many} ${aboutSaid(cause) ?? ""}`, typed);
  });

  // **The keyboard never falls to the page after Show again** (WCAG 2.4.3): to the row that took
  // its place, the one before where it was the last, or the Dismissed filter where none is left.
  useLayoutEffect(() => {
    const at = refocusAt.current;
    if (at === undefined) return;
    refocusAt.current = undefined;
    const rows = dismissedAt.current?.querySelectorAll<HTMLElement>("button") ?? [];
    (rows[Math.min(at, rows.length - 1)] ?? dismissedFilter.current)?.focus();
  });

  if (findable === undefined) return undefined;
  const pick = (next: Showing) => setShowing(next);
  const again = (cause: string, index: number) => {
    refocusAt.current = index;
    if (findable.dismissed.has(cause)) findable.showAgain(cause);
    else showHeld(cause);
  };

  const bar = (
    <div className="notices-find">
      <TextField
        type="search"
        size="sm"
        icon={<Search aria-hidden="true" />}
        aria-label={SEARCH_THE_NOTICES}
        placeholder={SEARCH_THE_NOTICES}
        value={query}
        onChange={setQuery}
        keepsFocus={`notices-search:${findable.project}`}
      />
      <ButtonRow size="sm" role="group" aria-label="Show notices of a kind">
        <ToggleButton size="sm" isSelected={"all" in showing} onChange={() => pick(ALL)}>
          {`All · ${ordered.length}`}
        </ToggleButton>
        {groups.map((group) => {
          const on = "kind" in showing && showing.kind === group.many;
          return (
            <ToggleButton
              key={group.many}
              size="sm"
              isSelected={on}
              onChange={() => pick(on ? ALL : { kind: group.many })}
            >
              {`${group.many} · ${group.members.length}`}
            </ToggleButton>
          );
        })}
        <ToggleButton
          ref={dismissedFilter}
          size="sm"
          isSelected={"dismissed" in showing}
          onChange={() => pick("dismissed" in showing ? ALL : DISMISSED)}
        >
          {dismissedSays(dismissed.length)}
        </ToggleButton>
      </ButtonRow>
      {drawn.map((group) =>
        createPortal(
          <PanelSectionTitle
            level={4}
          >{`${group.many} · ${group.found.length}`}</PanelSectionTitle>,
          heads.get(group.many) as HTMLElement,
          group.many,
        ),
      )}
    </div>
  );

  const after =
    "dismissed" in showing ? (
      <div className="notices-find">
        <p>{DISMISSED_LISTS}</p>
        {listed.length === 0 ? (
          <EmptyState
            variant="panel"
            headline={
              typed !== "" && dismissed.length > 0 ? noNoticeMatches(typed) : NONE_DISMISSED
            }
          />
        ) : (
          <ul
            ref={dismissedAt}
            role="list"
            className="notices-dismissed"
            aria-label="Dismissed notices"
          >
            {listed.map((cause, index) => (
              <MessageRow key={cause} data-cause={cause} tone="plain" icon={<CircleCheck />}>
                <MessageTop title={kindOf(cause).one} />
                {aboutSaid(cause) !== undefined && <MessageBody>{aboutSaid(cause)}</MessageBody>}
                <MessageLinks>
                  <Button size="sm" variant="quiet-link" onPress={() => again(cause, index)}>
                    Show again
                  </Button>
                </MessageLinks>
              </MessageRow>
            ))}
          </ul>
        )}
      </div>
    ) : typed !== "" && hosts.length === 0 && ordered.length > 0 ? (
      <EmptyState variant="panel" headline={noNoticeMatches(typed)} />
    ) : null;

  const reach = () => {
    if (asked === undefined || listAt === null || target === undefined) return;
    // Not before its view is shown: a hidden view takes no keyboard (the Notices view opens a
    // frame after the Go to).
    if (listAt.parentElement?.closest("[hidden]") !== null) return;
    if ("kind" in target) {
      const controls = [...listAt.querySelectorAll<HTMLElement>("button")].filter(
        (button) => button.textContent?.trim() === asked.control.words,
      );
      // Its Notice is not in its place yet: the next drawing reaches it.
      if (controls.length === 0) return;
      const named = controls.find((button) => {
        const cause = button.closest("[data-cause]")?.getAttribute("data-cause");
        return cause != null && namesIt(asked.names, cause);
      });
      (named ?? controls[0]).focus();
    } else if ("dismissed" in target) {
      const rows = [...(dismissedAt.current?.querySelectorAll<HTMLElement>("[data-cause]") ?? [])];
      const row =
        rows.find((one) => namesIt(asked.names, one.getAttribute("data-cause") ?? "")) ??
        rows.find((one) => familyOf(one.getAttribute("data-cause") ?? "") === asked.control.family);
      row?.querySelector<HTMLElement>("button")?.focus();
    }
    // Every kind, where none of its family stands or is dismissed: the view's own first stop
    // takes the keyboard (`PlaneView`).
    controlReached(asked);
  };
  return { hosts, bar, after, reach };
}

/** What a dismissed row says it was about, in plain words, where its cause says it plainly: a
 *  clone's or a repo's folder, a workspace, a chat by its number. Not an id no one reads. */
function aboutSaid(cause: string): string | undefined {
  const about = aboutOf(cause);
  switch (familyOf(cause)) {
    case "clone-vouch":
    case "stale-clone":
    case "pin-dormant":
    case "project-gone":
      return about === "" ? undefined : about;
    case "chat-resumed":
    case "chat-guessed":
    case "chat-fresh":
    case "chat-untaken":
      return /^\d+$/.test(about) ? `Chat ${about}` : undefined;
    default:
      return undefined;
  }
}

/** **Whether a pane's row of Notices is drawn**: not while it shows none (the row's `data-empty`
 *  is `display: none`), which is how it starts, until it has counted what it holds (#3302). */
const rowDrawn = (row: HTMLElement) => !row.hasAttribute("data-empty");

/** The Notices of a pane's stack, read off the stack itself: each is one element of it (the
 *  notice family's root), however deep in the pane's components it is written. */
const boxesOf = (stack: HTMLElement) => [...stack.children] as HTMLElement[];

/** An ask (`waits`), which stays when the band is minimised and is never counted. */
const waitsOn = (box: Element) => box.hasAttribute("data-waits");

/** **What a band read off its stack**: what it holds, as its bell says it, how many notes
 *  Dismiss all clears, how many Notices it shows, and what its asks are about (#5215). */
type BandRead = BandHolds & { clears: number; drawn: number; waitsOn: readonly string[] };

const sameRead = (one: BandRead, other: BandRead): boolean =>
  sameHolds(one, other) &&
  one.clears === other.clears &&
  one.drawn === other.drawn &&
  one.waitsOn.join("\u0000") === other.waitsOn.join("\u0000");

/** What a pane's stack holds, as its bell says it, and how many notes Dismiss all clears. */
function heldIn(boxes: readonly HTMLElement[]): Omit<BandRead, "drawn"> {
  const notes = boxes.filter((box) => !waitsOn(box));
  return {
    waitsOn: boxes.filter(waitsOn).flatMap((box) => box.getAttribute("data-what") ?? []),
    notes: notes.length,
    trouble: notes.filter((box) => box.getAttribute("data-variant") === "trouble").length,
    asks: boxes.length - notes.length,
    clears: notes.filter((box) => box.querySelector(DISMISS) !== null).length,
  };
}

/** **The bell a pane's band folds into** (#4754, `NoticePaneRow`). */
export type PaneBandBell = {
  /** Whether every pane's notices are minimised now. */
  minimised: boolean;
  /** Minimises every pane's notices: the head's fold. */
  minimise: () => void;
  /** Expands every pane's notices: the folded band's line. */
  expand: () => void;
  /** Told what the band holds whenever that changes. */
  onHolds: (holds: BandHolds) => void;
  /** Hands the keyboard to the bell: where Dismiss all leaves it once the band is empty. */
  focus: () => void;
};

/**
 * **A pane's band: its Notices, under the pane's header and above its terminal** (#1647, #3142,
 * #4754, spec #4750 stories 33 to 39).
 *
 * Nothing purlis says is drawn over what the chat wrote: the band takes a row, and the terminal,
 * pinned to its bottom, gives that height up. The header is above it, so a Notice arriving never
 * moves the header.
 *
 * **Minimised or expanded, one choice for every pane** (`paneNotices.ts`), where the band has a
 * bell to fold into (`bell`, the pane header's bell, `PaneBell`):
 *
 * - **minimised** (the default), the notes fold into the bell's count, and only the asks stand,
 *   as full cards (`waits`: a consent or a blocking question keeps its words);
 * - **expanded**, every Notice stands, with a head row that says how many it holds, **Dismiss
 *   all**, which skips the asks and counts what it clears, and **Minimise**.
 *
 * A band with no bell (a browser's pane) shows every Notice and has no head row. There is no cap:
 * past its room the band scrolls inside. A new Notice never opens the band or takes the focus:
 * the bell says the count, politely.
 *
 * The order is the pane's own, the order its Notices are written in (`ChatNotices`). Every
 * Notice is drawn where it is written, and the band only hides the notes it folds (`hidden`), so
 * React keeps each one's state and nothing is moved out from under the keyboard. **A Notice the
 * keyboard is on is never hidden**: it folds once the focus leaves it.
 *
 * **The terminal is refitted on every change of the band's height** (its `ResizeObserver`), so
 * minimised, a note arriving changes only the bell's count.
 */
export function NoticePaneRow({
  children,
  band,
  bell,
}: {
  children: ReactNode;
  /** The pane this band is (`paneBand`): what stood in it is remembered when it is drawn anew. */
  band?: string;
  /** The bell the band folds into: whether every pane's notices are minimised now, how to
   *  minimise them (the head row's Minimise), and what to tell it the band holds whenever that
   *  changes. Without one the band never folds. The pane hands it what the window keeps
   *  (`paneNoticesPref.ts`), so the notice family reads no state of the window's. */
  bell?: PaneBandBell;
}) {
  const stackAt = useRef<HTMLDivElement>(null);
  const minimiseAt = useRef<HTMLButtonElement>(null);
  const showAt = useRef<HTMLButtonElement>(null);
  /** The band's own fold or line was pressed: the keyboard goes on to what takes its place. */
  const goesOn = useRef(false);
  // **What this pane's band was told to close** (#5215), kept with the pane's band as it is drawn
  // anew. A chat's band alone: only there is a Notice a row.
  const [closed, setClosed] = useState<ReadonlySet<string>>(() => closedIn(band));
  const hasBell = bell !== undefined;
  // How many Notices of each cause stand. Read a moment after one goes: a band drawn anew takes
  // every Notice down and up again, and only one that stays down has gone.
  const standing = useRef(new Map<string, number>());
  const drawnNow = useRef(false);
  useLayoutEffect(() => {
    drawnNow.current = true;
    return () => {
      drawnNow.current = false;
    };
  }, []);
  const stands = useCallback(
    (cause: string) => {
      const count = standing.current;
      count.set(cause, (count.get(cause) ?? 0) + 1);
      return () => {
        count.set(cause, (count.get(cause) ?? 1) - 1);
        queueMicrotask(() => {
          if (drawnNow.current && count.get(cause) === 0) setClosed(reopenIn(band, cause));
        });
      };
    },
    [band],
  );
  const rows = useMemo(
    () =>
      !hasBell
        ? null
        : {
            closed,
            close: (cause: string) => setClosed(closeIn(band, cause)),
            stands,
          },
    [hasBell, closed, band, stands],
  );
  // **A granting Notice's ways out wait for it to settle once it moved** (#1695, train 54's
  // review): a Notice above it answered or gone can move another's way out under the pointer.
  const { at: rowAt, allows, said } = useNoticeSettle<HTMLDivElement>("stands", band, rowDrawn);
  const onHolds = bell?.onHolds;
  const folds = bell?.minimised === true;
  const [held, setHeld] = useState<BandRead>({
    ...HOLDS_NOTHING,
    clears: 0,
    drawn: 0,
    waitsOn: [],
  });

  /** Reads the stack again: set while the band is drawn. */
  const placeAt = useRef<() => void>(() => undefined);
  // Read off the stack: what it holds, and which notes the band folds.
  useLayoutEffect(() => {
    const stack = stackAt.current;
    if (stack === null) return;
    const place = () => {
      const boxes = boxesOf(stack);
      const on = document.activeElement;
      let drawn = 0;
      for (const box of boxes) {
        const hide = folds && !(on !== null && box.contains(on));
        if (box.hidden !== hide) box.hidden = hide;
        if (!hide) drawn += 1;
      }
      const now = { ...heldIn(boxes), drawn };
      setHeld((was) => (sameRead(was, now) ? was : now));
    };
    place();
    placeAt.current = place;
    // A Notice that comes or goes, and one that comes to ask or changes its tone or its ways out
    // where it stands (an answered question says what was done in the same box).
    const watching = new MutationObserver(place);
    watching.observe(stack, { childList: true });
    const changing = new MutationObserver(place);
    changing.observe(stack, {
      subtree: true,
      childList: true,
      attributeFilter: ["data-waits", "data-variant", "data-way"],
    });
    // A note kept for the keyboard folds once the keyboard has left it: after the focus has
    // moved, which is after `focusout`.
    let later: ReturnType<typeof setTimeout> | undefined;
    const left = () => {
      clearTimeout(later);
      later = setTimeout(place, 0);
    };
    stack.addEventListener("focusout", left);
    return () => {
      placeAt.current = () => undefined;
      watching.disconnect();
      changing.disconnect();
      stack.removeEventListener("focusout", left);
      clearTimeout(later);
    };
  }, [folds]);

  // **A Notice that came or went is read in the commit that drew it** (#3437): told in that
  // commit (`BandChanged`), the band renders again before the frame is laid out, and reads its
  // stack here, after the Notice's own box has come or gone. What the observers above read, a
  // task later, is then what was already read.
  // And so is a row this band closed itself, or opened again (`closed`): its Notice stays
  // drawn, and draws nothing.
  const [rowsMoved, reread] = useReducer((count: number) => count + 1, 0);
  useLayoutEffect(() => placeAt.current(), [rowsMoved, closed]);

  const { notes, trouble, asks } = held;
  useEffect(() => onHolds?.({ notes, trouble, asks }), [onHolds, notes, trouble, asks]);

  // **Esc puts away the warning the keyboard is on, and the keyboard goes on to the next**
  // (#3142, rule 14): to the warning that took its place, or to the pane's terminal when it was
  // the last. Only a warning with a Dismiss: an ask has none, and Esc leaves it standing.
  useEffect(() => {
    const row = rowAt.current;
    const stack = stackAt.current;
    if (row === null || stack === null) return;
    const shownNow = () => boxesOf(stack).filter((box) => !box.hidden);
    const boxOf = (node: EventTarget | null) =>
      node instanceof Node ? boxesOf(stack).find((box) => box.contains(node)) : undefined;
    /** The warning the keyboard was last in, and where it stood among those shown. */
    let was: { box: Element; at: number } | undefined;
    const into = (event: FocusEvent) => {
      const box = boxOf(event.target);
      was = box === undefined ? undefined : { box, at: shownNow().indexOf(box) };
    };
    // A warning answered, dismissed or put away by Esc leaves the page with the focus on it:
    // the keyboard goes on from where it stood, never to the top of the window.
    const goOn = () => {
      if (was === undefined || was.box.isConnected) return;
      const at = was.at;
      was = undefined;
      const on = document.activeElement;
      if (on !== null && on !== document.body) return;
      const left = shownNow();
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
      if (event.key !== "Escape") return;
      // On the warning's own line only: Esc in what a way out opened (a field being written)
      // is that field's, and never puts the warning away.
      const box = boxOf(event.target);
      const line = event.target instanceof Element ? event.target.closest('[role="status"]') : null;
      if (box === undefined || line?.parentElement !== box) return;
      // **A message opened folds first** (#5215), and the keyboard stays on its toggle.
      const opened = box.querySelector<HTMLElement>('[data-way="whole"][aria-expanded="true"]');
      if (opened !== null) {
        event.preventDefault();
        event.stopPropagation();
        opened.click();
        opened.focus();
        return;
      }
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
  }, [rowAt]);

  /** **Dismiss all**: every note's own Dismiss, the asks left standing. The keyboard stays on
   *  the band's head while it is drawn, and goes to the pane's bell once it is not. */
  const dismissAll = () => {
    const stack = stackAt.current;
    if (stack === null) return;
    for (const box of boxesOf(stack))
      if (!waitsOn(box)) box.querySelector<HTMLElement>(DISMISS)?.click();
    setTimeout(() => {
      if (minimiseAt.current?.isConnected) minimiseAt.current.focus();
      else bell?.focus();
    }, 0);
  };

  const headed = bell !== undefined && !folds && notes + asks > 0;
  /** **Folded, one line** (#5215): how many wait and on what. Notes alone fold into the bell. */
  const foldedLine = bell !== undefined && folds && asks > 0;
  // Unfolding, the read of what it shows comes after: the head is what is drawn meanwhile, and the
  // fold can only take the keyboard if the band is drawn by then.
  const empty = held.drawn === 0 && !foldedLine && !headed;
  // **A band drawn anew starts at its top** (#3437): put away in the same layout as its last
  // warning, its stack was never laid out shrinking, and kept the scroll of what it held. The
  // next warning would be drawn scrolled out of it.
  const wasEmpty = useRef(empty);
  useLayoutEffect(() => {
    const stack = stackAt.current;
    if (wasEmpty.current && !empty && stack !== null) stack.scrollTop = 0;
    wasEmpty.current = empty;
  }, [empty]);
  // The keyboard goes from the fold to the line that took its place, and back (WCAG 2.4.3).
  useLayoutEffect(() => {
    if (!goesOn.current) return;
    goesOn.current = false;
    (folds ? showAt : minimiseAt).current?.focus();
  }, [folds]);
  return (
    <NoticeRow
      empty={empty}
      // The pane's notices, named as one place with its count (#3142, #5215), while it shows any.
      role={empty ? undefined : "region"}
      aria-label={
        empty
          ? undefined
          : bell === undefined
            ? "Pane notices"
            : bandRegionSays({ notes, trouble, asks })
      }
      ref={rowAt}
    >
      {foldedLine && (
        <NoticeRowFolded
          ref={showAt}
          says={foldedSays(asks, held.waitsOn)}
          show={SHOW_BAND}
          onPress={() => {
            goesOn.current = true;
            bell?.expand();
          }}
        />
      )}
      {headed && (
        <NoticeRowHead says={bandHeadSays({ notes, trouble, asks })}>
          {held.clears > 0 && (
            <Button size="sm" variant="ghost" onPress={dismissAll} data-way="dismiss-all">
              {dismissAllSays(held.clears)}
            </Button>
          )}
          <IconButton
            size="sm"
            variant="ghost"
            ref={minimiseAt}
            aria-label={FOLD_BAND}
            onPress={() => {
              goesOn.current = true;
              bell?.minimise();
            }}
            data-way="minimise"
            icon={<ChevronUp aria-hidden="true" />}
          />
        </NoticeRowHead>
      )}
      <NoticeStack ref={stackAt}>
        <BandChanged.Provider value={reread}>
          <BandRows.Provider value={rows}>
            <NoticeWayGuard value={allows}>{children}</NoticeWayGuard>
          </BandRows.Provider>
        </BandChanged.Provider>
      </NoticeStack>
      {said !== undefined && (
        <p className="inbox-refused" role="status">
          {said}
        </p>
      )}
    </NoticeRow>
  );
}
