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
 * `tone` is `news` (nothing is wrong, the operator is being told) or `trouble` (something went
 * wrong), and it is only the look: a Notice is always a polite `status`, because it stands
 * until it is dealt with and an `alert` would interrupt a screen reader on every relaunch.
 * `at` is where it stands: in the Inbox's Notices (`inbox`, the default, `NoticeList`, #1695), or
 * in its pane's row of Notices above the terminal, never over it (`pane`, `NoticePaneRow`,
 * #1647).
 *
 * **A Notice in a pane fits its pane, whatever it says** (#1481). It began as one short line;
 * it now also carries a long sentence, three long buttons and what one of them opens. So
 * `at="pane"` draws one box (`notice-pane-box`) holding the line and, under it, what a way out
 * opened. The sentence has the row; the ways out share it only when all of it fits unwrapped,
 * and otherwise go under the sentence; nothing is ever wider or taller than the pane
 * (`App.css`, `.pane-notices`).
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
  /** While what the press started is on its way: the button is disabled, so a second press
   *  sends nothing (an Undo in flight, #1190). */
  busy?: boolean;
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
};

/** At least one way out, said in the type: a Notice with none does not compile. */
type WayOut =
  | (Ways & { fixes: readonly [NoticeAction, ...NoticeAction[]] })
  | (Ways & { link: NoticeAction })
  | (Ways & { copy: string })
  | (Ways & { onDismiss: () => void });

export type NoticeProps = WayOut & {
  /** What the line is about, stably: what a dismissal will be keyed by. */
  cause: string;
  tone?: "news" | "trouble";
  at?: "inbox" | "pane";
  /** The accessible name, where the sentence alone would not make a good one. */
  label?: string;
  /** The persona the line names, when it names one: its mark is drawn before the sentence
   *  (#1449). */
  persona?: string | null;
  /** The sentence (and anything else the line says before its ways out). */
  children: ReactNode;
  /** What a way out opened (a fix's form, #1250): drawn right after the line, outside its live
   *  region, and moved with it by the Inbox's list. */
  under?: ReactNode;
};

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

export function Notice(props: NoticeProps) {
  const { cause, tone = "news", at = "inbox", label, persona, children, under } = props;
  const { fixes, link, copy, onDismiss } = props as Ways;
  const list = useContext(Listed);
  const provided = useContext(NoticeOf);
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
  const classes = ["notice", `notice-${at}`, tone === "trouble" ? "notice-trouble" : ""]
    .filter(Boolean)
    .join(" ");
  const line = (
    <div
      className={classes}
      role="status"
      aria-label={of === null || label === undefined ? label : `${of.whose}: ${label}`}
      data-cause={cause}
    >
      {/* In a pane the mark is the sentence's first word, so it is never left alone on a row
          above a sentence that takes the next one whole (#1481). */}
      {persona != null && at !== "pane" && <PersonaMark persona={persona} />}
      <div className="notice-says">
        {persona != null && at === "pane" && <PersonaMark persona={persona} />}
        {of !== null && <strong className="notice-whose">{of.whose}: </strong>}
        {children}
      </div>
      {/* The way to the chat it is about, first: its other ways out answer for that chat. */}
      {of !== null && (
        <button type="button" className="notice-link notice-go" tabIndex={0} onClick={of.onGo}>
          Go to it
        </button>
      )}
      {/* `tabIndex={0}` on every button, per `docs/ui-primitives.md` (charter-app#186). */}
      {fixes?.map((fix) => (
        <button
          key={fix.label}
          type="button"
          className="notice-fix"
          tabIndex={0}
          aria-expanded={fix.opens?.open}
          aria-controls={fix.opens?.open ? fix.opens.id : undefined}
          disabled={fix.busy}
          aria-busy={fix.busy}
          onClick={fix.onPress}
        >
          {fix.label}
        </button>
      ))}
      {link && (
        <button type="button" className="notice-link" tabIndex={0} onClick={link.onPress}>
          {link.label}
        </button>
      )}
      {copy !== undefined && (
        <button
          type="button"
          className="notice-copy"
          tabIndex={0}
          title={copy}
          onClick={() => void navigator.clipboard?.writeText(copy).catch(() => undefined)}
        >
          Copy command
        </button>
      )}
      {onDismiss && (
        <button type="button" className="notice-dismiss" tabIndex={0} onClick={onDismiss}>
          Dismiss
        </button>
      )}
    </div>
  );
  // What a way out opened is drawn after the line, never inside it: the line is a live region,
  // and a form in one would be read out again on every keystroke and every refusal.
  const opened =
    under === undefined ? null : <div className={`notice-under notice-under-${at}`}>{under}</div>;
  // **In a pane's corner the line and what it opened are one box** (#1481): the corner lays its
  // Notices out one under another, and a box of its own is what keeps what a way out opened
  // under its line and at its width, whatever the pane's width. In the Inbox the two stay
  // siblings, as the list moves them.
  const drawn =
    at === "pane" ? (
      <div
        className={opened === null ? "notice-pane-box" : "notice-pane-box notice-opened"}
        // What the pane's row stands first (`NoticePaneRow`): a Notice with something to do
        // about it, before one whose only way out is to put it away.
        data-asks={fixes !== undefined || link !== undefined ? "" : undefined}
      >
        {line}
        {opened}
      </div>
    ) : (
      <>
        {line}
        {opened}
      </>
    );
  return stacked ? createPortal(drawn, host) : drawn;
}

/**
 * **How important a Notice is, by the family of its cause** (V91i): trouble before news, then
 * this order, then the one drawn first. A family is the cause up to its first `:`; one not in
 * the list comes after every one that is.
 *
 * - an Undo that lasts a few seconds, before anything that waits;
 * - what the operator just did, before what charter found;
 * - what happened while the person was away (#1514, #1551), before everything else that waits:
 *   it is what they read first on coming back;
 * - an offer, before news about how things came back;
 * - a chat that lost its conversation, before one charter had to guess about, before one that
 *   came back as it was.
 */
export const IMPORTANCE: readonly string[] = [
  "window-trouble",
  "chat-did-not-start",
  "memory-deleted",
  "memory-moved",
  "pin-forgotten",
  "session-saved",
  "away-summary",
  "sandbox-offer",
  "sandbox-hosts",
  "sandbox-presets",
  "vaults-waiting",
  "pin-dormant",
  "chat-fresh",
  "chat-guessed",
  "chat-resumed",
];

/** How many Notices stand in a pane's row; the rest are behind "+N more" (V91i, #1647). */
export const SHOWN = 2;

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

/** Whether a Notice with this cause is counted by the status line's Notices button. */
export const countsInStatusBar = (cause: string): boolean => !FROM_THE_DOCTOR.has(familyOf(cause));

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

type Stacked = { cause: string; tone: "news" | "trouble"; host: HTMLElement };
type Entry = Stacked & { seq: number };

/** What the Inbox's list is to the Notices drawn inside it. */
const Listed = createContext<{
  register: (id: string, notice: Stacked) => () => void;
} | null>(null);

/**
 * **A project's Notices, listed in its Inbox** (#1695, V91i): every `at="inbox"` Notice drawn
 * anywhere inside it, the most important first ({@link IMPORTANCE}), trouble before news. They
 * stood under the strip, two at a time with the rest behind "+N more", until the band folded
 * into the Inbox (spec #1688, I-4): there every one is listed, with its ways out.
 *
 * Each Notice is drawn into an element of its own, and the list only moves those elements: the
 * order is the list's, what each says is still its Notice's. **Only what is out of its place
 * moves**, so a button the person is on keeps the focus as another Notice arrives (F2).
 *
 * `onCount` hears how many it lists that the status line counts ({@link countsInStatusBar});
 * `onAnswer` hears that a Notice of an {@link ANSWERS} family arrived.
 */
export function NoticeList({
  children,
  onCount,
  onAnswer,
}: {
  children: ReactNode;
  onCount?: (count: number) => void;
  onAnswer?: () => void;
}) {
  const [entries, setEntries] = useState<ReadonlyMap<string, Entry>>(() => new Map());
  const listAt = useRef<HTMLDivElement>(null);
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
          Number(b.tone === "trouble") - Number(a.tone === "trouble") ||
          rank(a.cause) - rank(b.cause) ||
          a.seq - b.seq,
      ),
    [entries],
  );

  // Before the frame is painted, so a Notice is never seen out of its place.
  useLayoutEffect(() => {
    const at = listAt.current;
    if (at === null) return;
    const hosts = ordered.map((one) => one.host);
    for (const child of [...at.children]) if (!hosts.includes(child as HTMLElement)) child.remove();
    hosts.forEach((host, index) => {
      const there = at.children.item(index);
      if (there !== host) at.insertBefore(host, there);
    });
  }, [ordered]);

  const counted = ordered.filter((one) => countsInStatusBar(one.cause)).length;
  useEffect(() => onCount?.(counted), [onCount, counted]);

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

  return (
    <Listed.Provider value={list}>
      {children}
      <div className="notice-list" ref={listAt} hidden={ordered.length === 0} />
    </Listed.Provider>
  );
}

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
export function NoticePaneRow({ children }: { children: ReactNode }) {
  const stackAt = useRef<HTMLDivElement>(null);
  const moreAt = useRef<HTMLButtonElement>(null);
  const rowAt = useRef<HTMLDivElement>(null);
  const [count, setCount] = useState(0);
  const [open, setOpen] = useState(false);
  const behind = Math.max(count - SHOWN, 0);
  // Nothing behind it: the list closes, so the next one to fall behind does not open it again.
  if (behind === 0 && open) setOpen(false);

  // Each Notice is one element of the stack (`notice-pane-box`). Read off the stack itself, so
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
  }, [open]);

  return (
    <div
      className={count === 0 ? "pane-notice-row pane-notice-row-empty" : "pane-notice-row"}
      ref={rowAt}
    >
      <div className="pane-notices" ref={stackAt}>
        {children}
      </div>
      {behind > 0 && (
        <button
          type="button"
          className="notice-more notice-more-pane"
          ref={moreAt}
          tabIndex={0}
          aria-expanded={open}
          onClick={() => setOpen((was) => !was)}
        >
          +{behind} more
        </button>
      )}
    </div>
  );
}
