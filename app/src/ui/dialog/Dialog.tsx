// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The dialog family** (#2910, spec #2897, ADR 0104): every dialog and alert dialog in the
 * window, on React Aria's `ModalOverlay`, `Modal` and `Dialog`, wrapped here once. A screen
 * imports these and never `react-aria-components` (`eslint.config.js`), and never writes
 * `role="dialog"` itself (`purlis/dialog-family`).
 *
 * **The dialog rules** (#2798, #2789, the operator's ruling in #2910), each held by construction:
 *
 * - **Three rows**: the header (the title, and a line under it), the body, which is the only part
 *   that scrolls, and the footer with the answers, always in view. A focused control in the body
 *   is never under the footer (WCAG 2.4.11): the two never overlap.
 * - **The width is the size's, never the content's**: `standard` (600 px) or `wide` (800 px,
 *   only for a list beside its details; the operator's ruling on #2918), each at most the window's width less its gutters.
 * - **The height follows the content up to the window less its top less 24 px**; a dialog given
 *   a `height` takes the lesser of the two and keeps it whatever its content.
 * - **Anchored at a fixed top**, `max(24px, 6vh)` down (R2), so a change in its content moves
 *   only its bottom edge.
 * - **At most three answers, each in its place**: Cancel leads, the main action trails, and one
 *   other answer may stand between them. They are props, not children, so a fourth, or a main
 *   action anywhere but last, cannot be written.
 * - **⏎ is the main action and Esc closes.** The body and the footer are one form, so Return in a
 *   field answers with the main action, as a native form does, and a button with the keyboard is
 *   pressed by its own Return. An answer that grants (`variant: "grants"`) is never the one
 *   Return finds from a field: it is pressed, or not taken. An act that cannot be taken back
 *   (`variant: "danger"`) is not either (#1210), unless it says so (`byReturn`, a confirm by
 *   typed name, D-1210-9).
 * - **No dialog opens another.** One opened while another is up says so (`console.error`).
 * - **It starts on the right thing**: a dialog on its first field, else the first thing it holds
 *   the keyboard can reach; an alert dialog on its safe answer, Cancel (#2980), or on the dialog
 *   itself while Cancel cannot be pressed, never on the main action. `initialFocus`
 *   names another, or the dialog itself while what it holds is still being read. Closed, the
 *   keyboard goes back to what had it; `afterClose` can send it on.
 * - **DialogFrame only while its project is in front** (`projectShown.tsx`): a project kept behind
 *   another never puts a dialog over the one in front.
 *
 * **DialogFrame by its parts** (ADR 0103): the scrim, the root (`data-variant` `dialog` or `alert`,
 * `data-size`), the header, title, subtitle, description, body, separator, footer, summary and
 * actions, each `data-scope="dialog"` with its `data-part`. `dialog.css` draws the family default
 * from them, and a theme's parts reach the same attributes. A screen gives a dialog a hook for
 * what is inside it (`className`), never its size or its place.
 */
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type FormEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  type RefObject,
} from "react";
import {
  Dialog as AriaDialog,
  Modal as AriaModal,
  ModalOverlay as AriaModalOverlay,
} from "react-aria-components";
import { createPortal } from "react-dom";
import { LockOpen } from "lucide-react";
import { useProjectShown } from "../../projectShown";
import { Button, ButtonRow, type ButtonProps, type ButtonVariant } from "../button/Button";
import { Form } from "../field/Field";
import { submits as chordSends } from "../field/submitKey";
import { Reserved } from "./Reserved";
import { entryOf, nextStop, stopsIn } from "./tabOrder";

/** The family's sizes: 600 and 800 px wide (`dialog.css`). */
export const DIALOG_SIZES = ["standard", "wide"] as const;
export type DialogSize = (typeof DIALOG_SIZES)[number];

/**
 * **One answer in a dialog's footer**: a button of the family, with its words as `children`. Its
 * size is the footer's. `reserve` lists every word it can say (`Create`, `Creating…`), so it is
 * as wide as the widest whatever it says now, and nothing beside it moves.
 */
export type DialogAction = Omit<ButtonProps, "size" | "type" | "onPress" | "variant"> & {
  onPress?: () => void;
  reserve?: readonly string[];
  /**
   * An answer that is a grant for some choices and not for others (Start a chat, approving a
   * profile's command): it holds the grant's lock's room in every look, so it never widens as
   * the choice changes.
   */
  reserveGrant?: boolean;
  /**
   * For a main action Return never finds (`byReturn: false`): whether ⌘⏎ (Ctrl+Enter off
   * macOS) presses it, from anywhere in the dialog. A chord is never a stray Return. A grant or
   * an act that cannot be taken back is never pressed by it.
   */
  byChord?: boolean;
} & (
    | {
        variant?: Exclude<ButtonVariant, "grants">;
        /**
         * For the main action: whether Return in a field answers with it. It does, unless it
         * cannot be taken back (`danger`, #1210) or says it does not: an act that runs a command
         * with nothing between the key and the exec (Start a chat) is pressed, never found by
         * Return. A confirm by typed name says it does (D-1210-9): Return in the name box
         * deletes, once the delete is enabled by the name typed exactly.
         */
        byReturn?: boolean;
      }
    | {
        /** An answer that grants: it is pressed, never found by Return, whatever it holds. */
        variant: "grants";
        byReturn?: false;
      }
  );

/** Where the keyboard starts in a dialog. */
export type InitialFocus =
  | "field"
  | "cancel"
  | "main"
  | "dialog"
  | RefObject<HTMLElement | null>
  | ((dialog: HTMLElement) => HTMLElement | null | undefined);

interface Surface {
  /** Whether it is up; `true` when not given, for a dialog drawn while its state says so. */
  isOpen?: boolean;
  /** Esc, Cancel, or any other way out the person takes. */
  onClose: () => void;
  size?: DialogSize;
  /** A height of its own, in px, taken whatever its content, at most the window's less its top less 24 px. */
  height?: number;
  /** Where the keyboard starts (see the family's rules). */
  initialFocus?: InitialFocus;
  /**
   * Called once it has closed and the keyboard has gone back to what had it: where a dialog
   * sends the keyboard somewhere else instead (a row that went into Settings).
   */
  afterClose?: () => void;
  /**
   * Whether the keyboard goes back to what had it as it opened, asked as it closes; it does when
   * not given. A dialog whose answer hands the keyboard on (a start, to the new chat) says no
   * for that answer.
   */
  returnFocus?: () => boolean;
  /** A hook for what is inside it, and for the tests that find it. Never its size or place. */
  className?: string;
  "aria-describedby"?: string;
  "data-testid"?: string;
}

export interface DialogProps extends Surface {
  title: ReactNode;
  /** The title's id, for what names the dialog by it. */
  titleId?: string;
  /** The line under the title: where it is, what it is about. It describes the dialog. */
  subtitle?: ReactNode;
  /** What the question is, said first in the body. It describes the dialog. */
  description?: ReactNode;
  /** What the footer says before its answers: a summary of what the main action will do. */
  summary?: ReactNode;
  /** The way out, leading. `Cancel` and `onClose` unless it says otherwise; `false` for none. */
  cancel?: Partial<DialogAction> | false;
  /** The main action, trailing: what ⏎ does. `primary` unless it says otherwise. */
  main?: DialogAction;
  /** One more answer, between the two. */
  alternative?: DialogAction;
  children?: ReactNode;
}

/** What React Aria asks before it hands the keyboard back as a modal goes: it gives up when told
 *  to (`preventDefault`). */
const RESTORE_EVENT = "react-aria-focus-scope-restore";

/** How many of the family's dialogs are up, to say when one opens over another. */
let up = 0;

/** The words of an answer, kept as wide as the widest it can say. */
function words(action: Pick<DialogAction, "children" | "reserve">): ReactNode {
  const { children, reserve } = action;
  return reserve && reserve.length > 0 ? <Reserved texts={reserve}>{children}</Reserved> : children;
}

/** The first field a person types in, in `root`, that can take the keyboard. */
const FIELD =
  'input:not([type="hidden"]):not([type="checkbox"]):not([type="radio"]):not([type="button"]):not([type="submit"]):not(:disabled), textarea:not(:disabled), [contenteditable="true"]';

function reachable(element: Element): element is HTMLElement {
  return element instanceof HTMLElement && !element.closest("[inert], [aria-hidden='true']");
}

/**
 * **The keyboard in a dialog: where it starts, and where it goes back to.**
 *
 * - **It starts once React Aria holds it in the dialog.** React Aria starts holding the keyboard
 *   in a modal one render after it first draws it (its focus scope is told to contain in a
 *   layout effect), and from then on it remembers what had the keyboard last, to put it back
 *   there when something outside takes it: the menu a dialog was opened from, handing the
 *   keyboard back to its trigger as it closes. So this waits for that render (`armed`) and then
 *   moves the keyboard, which is the move it remembers. Until then React Aria puts the keyboard
 *   on the dialog itself.
 * - **It goes back to what had it as the dialog opened**, if that is still there and nothing
 *   else has taken it, as the dialog goes (a microtask after its last effect), and then
 *   `afterClose` may send it on. The window is reachable again by then; React Aria's own return,
 *   a frame later, finds the keyboard placed and leaves it. A dialog hidden with its project
 *   behind another (`gone` false) has not closed, and hands nothing back.
 */
function DialogKeyboard({
  root,
  initialFocus,
  alert,
  closing,
}: {
  root: RefObject<HTMLElement | null>;
  initialFocus: InitialFocus | undefined;
  alert: boolean;
  /**
   * Read as it goes: whether it goes because it closed, and what to call then. One record for
   * the dialog's life, its fields written by each render.
   */
  closing: RefObject<{ gone: boolean; afterClose?: () => void; returnFocus?: () => boolean }>;
}) {
  // What had the keyboard as it opened, if it can take it back: told as it opens, while the
  // window's globals are surely there.
  const [opener] = useState(() => {
    const had = document.activeElement;
    return had instanceof HTMLElement ? had : null;
  });
  const [armed, setArmed] = useState(false);
  // eslint-disable-next-line react-hooks/set-state-in-effect -- the one render after the first, when React Aria holds the keyboard (above)
  useLayoutEffect(() => setArmed(true), []);
  useEffect(() => {
    const dialog = root.current;
    if (!armed || !dialog) return;
    const answer = (which: "cancel" | "main") =>
      dialog.querySelector<HTMLElement>(`[data-dialog-action="${which}"]:not(:disabled)`);
    const how = initialFocus ?? (alert ? "cancel" : "field");
    let target: HTMLElement | null;
    if (typeof how === "function") target = how(dialog) ?? null;
    else if (typeof how === "object") target = how.current;
    // Never the main action in Cancel's place: a Cancel that cannot be pressed yet leaves the
    // keyboard on the dialog itself, so the first answer taken is the person's.
    else if (how === "cancel") target = answer("cancel") ?? dialog;
    else if (how === "main") target = answer("main");
    else if (how === "dialog") target = dialog;
    else {
      const body = dialog.querySelector('[data-scope="dialog"][data-part="body"]') ?? dialog;
      target =
        ([...body.querySelectorAll(FIELD)].find(reachable) as HTMLElement | undefined) ??
        (stopsIn(dialog).map(entryOf)[0] as HTMLElement | undefined) ??
        null;
    }
    // **A grant never takes the keyboard as a dialog opens** (#2980): a key pressed by habit
    // never answers yes. Asked to start on one, it starts on Cancel, or on the dialog itself.
    if (target?.closest('[data-variant="grants"]')) target = answer("cancel") ?? dialog;
    target?.focus();
    // Once, as it opens: what the person does next is theirs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [armed]);
  // **The window is reachable again the moment the dialog goes**, in the commit that takes it
  // away. React Aria makes the rest of the window inert while a modal is up and gives it back a
  // moment later, after the commit; a control that takes the keyboard as the dialog goes (a tab's
  // rename box a palette row opened) would find it still inert, and its focus would be lost.
  // No dialog is up while another is, so whatever is inert outside this one is this one's.
  useLayoutEffect(() => {
    const dialog = root.current;
    return () => {
      for (const held of document.querySelectorAll<HTMLElement>("[inert]"))
        if (!dialog?.contains(held)) held.inert = false;
    };
  }, [root]);
  // Whether it is still drawn: React's strict mode takes every effect down and puts it back as
  // a dialog first draws, which is not a close, so what a close does waits a microtask and is
  // done only if the dialog did not come back.
  const drawn = useRef(false);
  useEffect(() => {
    drawn.current = true;
    // What the close reads as it goes: the modal's one record of it, whose fields its last
    // render wrote (the record itself is never replaced), so they are the close's own.
    const state = closing.current;
    // **The document the dialog is drawn in**, kept for the close's last steps, which run after
    // it has gone: the global `document` may be gone by then (a webview, a test's window, taken
    // down). A document with no window left has nothing to hand the keyboard back to.
    const doc = root.current?.ownerDocument ?? document;
    return () => {
      drawn.current = false;
      const { gone, afterClose, returnFocus } = state;
      queueMicrotask(() => {
        if (drawn.current || !gone || doc.defaultView === null) return;
        if (returnFocus?.() === false) {
          // Nor does React Aria's own return, a frame later.
          const keep = (event: Event) => event.preventDefault();
          doc.addEventListener(RESTORE_EVENT, keep, true);
          setTimeout(() => doc.removeEventListener(RESTORE_EVENT, keep, true), 100);
        } else {
          const now = doc.activeElement;
          if ((now === null || now === doc.body) && opener?.isConnected === true) opener.focus();
        }
        afterClose?.();
      });
    };
  }, [closing, opener, root]);
  return null;
}

/**
 * **Tab moves through the dialog's stops** (`tabOrder.ts`), round from the last to the first,
 * and Esc closes it from the page as from inside it,
 * listened for on the window before anything else is, so the move is the dialog's whatever the
 * engine or a library inside it would do. A control that takes Tab as its own (an editor) is left
 * alone.
 */
function useTabbing(
  open: boolean,
  root: RefObject<HTMLElement | null>,
  onClose: () => void,
  own: boolean,
) {
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  });
  useEffect(() => {
    if (!open) return;
    // **Esc closes one layer** (the family's own, not React Aria's): the dialog, from anywhere in
    // it, and from the page, where a pressed button that went away left the keyboard, or a
    // button pressed and disabled while it works (a Start). A layer of its own over the dialog,
    // a menu or a select's list drawn in the top layer, takes its Esc first and alone: a key from
    // outside the dialog closes nothing of it. Heard last, on the window, so what answered the
    // key itself (a box that clears on Esc, the tooltip's first Esc) keeps it.
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented || event.isComposing) return;
      const dialog = root.current;
      if (!dialog) return;
      const now = document.activeElement;
      const fromItself = event.target instanceof Node && dialog.contains(event.target);
      const fromThePage = now === null || now === document.body || now.matches(":disabled");
      if (!fromItself && !fromThePage) return;
      event.preventDefault();
      close.current();
    };
    window.addEventListener("keydown", escape);
    const tab = (event: KeyboardEvent) => {
      // A surface with an anatomy of its own (the palette) keeps Tab for itself.
      if (own) return;
      if (event.key !== "Tab" || event.altKey || event.ctrlKey || event.metaKey) return;
      if (event.isComposing || event.defaultPrevented) return;
      const dialog = root.current;
      const at = document.activeElement;
      if (!dialog || !(at instanceof Element) || !dialog.contains(at)) return;
      // An editor (its editable content) and a terminal keep Tab for what is typed in them. Named
      // by what they are, not CodeMirror's class, which marks a chunk as holding the editor.
      if (at.closest('[contenteditable="true"], .xterm')) return;
      const next = nextStop(dialog, at, event.shiftKey);
      if (next === null) return;
      event.preventDefault();
      event.stopPropagation();
      next.focus();
      // Wholly in sight, clear of the body's edges and so of the footer (WCAG 2.4.11): a focus
      // scrolls a control only until some of it shows.
      next.scrollIntoView?.({ block: "nearest", inline: "nearest" });
      if (
        next instanceof HTMLInputElement &&
        /^(text|search|url|tel|email|password|)$/.test(next.type)
      )
        next.select();
    };
    window.addEventListener("keydown", tab, true);
    return () => {
      window.removeEventListener("keydown", tab, true);
      window.removeEventListener("keydown", escape);
    };
  }, [open, root, own]);
}

/** Says when a dialog opens while another is up, and counts it while it is. */
function useOneAtATime(open: boolean) {
  useEffect(() => {
    if (!open) return;
    if (up > 0)
      console.error(
        "purlis: a dialog was opened while another is up, and no dialog opens another " +
          "(spec #2897): close the first, or answer it in place.",
      );
    up += 1;
    return () => {
      up -= 1;
    };
  }, [open]);
}

/**
 * **The modal itself**: the scrim, the window behind it made unreachable, the keyboard held in
 * it, Esc, the fixed top, the width and the height. `Dialog` and `AlertDialog` draw the family's
 * three rows in it; `DialogSurface` lets a surface with an anatomy of its own (the palette) draw
 * its own.
 */
function DialogModal({
  isOpen = true,
  onClose,
  size = "standard",
  height,
  className,
  role,
  label,
  labelledBy,
  describedBy,
  testId,
  initialFocus,
  afterClose,
  returnFocus,
  own = false,
  children,
}: Omit<Surface, "aria-describedby" | "data-testid"> & {
  /** Whether what is inside lays itself out (`DialogSurface`). */
  own?: boolean;
  role: "dialog" | "alertdialog";
  label?: string;
  labelledBy?: string;
  describedBy?: string;
  testId?: string;
  children: ReactNode;
}) {
  const shown = useProjectShown();
  const open = isOpen && shown;
  useOneAtATime(open);
  const root = useRef<HTMLElement>(null);
  useTabbing(open, root, onClose, own);
  // Whether the dialog goes because it closed, as its keyboard reads it as it goes.
  const closing = useRef<{
    gone: boolean;
    afterClose?: () => void;
    returnFocus?: () => boolean;
  }>({ gone: true });
  // Written in place, never replaced: the keyboard holds this one record from the start and
  // reads it as it goes, so what it reads is the last render's.
  useLayoutEffect(() => {
    Object.assign(closing.current, { gone: shown, afterClose, returnFocus });
  });
  if (!shown) return null;
  const style =
    height === undefined ? undefined : ({ "--dialog-height": `${height}px` } as CSSProperties);
  return (
    <AriaModalOverlay
      isOpen={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      // A click outside answers nothing (`docs/ui-primitives.md`): Esc or an answer. Esc is the
      // family's own (`useTabbing`), so it closes one layer at a time.
      isDismissable={false}
      isKeyboardDismissDisabled
      data-scope="dialog"
      data-part="scrim"
    >
      <AriaModal data-scope="dialog" data-part="modal">
        <AriaDialog
          ref={root}
          role={role}
          aria-label={label}
          aria-labelledby={labelledBy}
          aria-describedby={describedBy}
          className={className}
          style={style}
          data-testid={testId}
          data-scope="dialog"
          data-part="root"
          data-variant={role === "alertdialog" ? "alert" : "dialog"}
          data-size={size}
          data-fixed-height={height === undefined ? undefined : ""}
          data-own-anatomy={own ? "" : undefined}
        >
          <DialogKeyboard
            root={root}
            initialFocus={initialFocus}
            alert={role === "alertdialog"}
            closing={closing}
          />
          {children}
        </AriaDialog>
      </AriaModal>
    </AriaModalOverlay>
  );
}

/**
 * **A surface over the window with an anatomy of its own** (the palette): the family's modal,
 * width, fixed top and height, and nothing inside it of the family's. Named by `aria-label`.
 */
export function DialogSurface({
  "aria-label": label,
  "aria-describedby": describedBy,
  "data-testid": testId,
  children,
  ...surface
}: Surface & { "aria-label": string; children: ReactNode }) {
  return (
    <DialogModal
      {...surface}
      own
      role="dialog"
      label={label}
      describedBy={describedBy}
      testId={testId}
    >
      {children}
    </DialogModal>
  );
}

/** What a dialog's answers are: the three places of its footer, and what it says before them. */
export type Answers = Pick<DialogProps, "summary" | "cancel" | "main" | "alternative">;

/**
 * The footer's place for answers drawn from inside the body ({@link DialogAnswers}), and the
 * main action ⏎ answers with once they are.
 */
type Place = {
  actions: HTMLElement | null;
  summary: HTMLElement | null;
  onClose: () => void;
  /** Sets the main action ⏎ takes from the answers drawn from inside, when they are. */
  setSubmit: (main: (() => void) | undefined) => void;
  claim: (by: number) => void;
};

const FooterPlace = createContext<Place | null>(null);

/**
 * Whether `main` is the one ⏎ finds: an act that grants never is, and one that cannot be taken
 * back is not unless it says so (#1210).
 */
const submits = (main: DialogAction | undefined): boolean =>
  main !== undefined && main.variant !== "grants" && (main.byReturn ?? main.variant !== "danger");

/** The room of a grant's lock, held and never drawn, on an answer that may be a grant. */
function LockRoom() {
  return <LockOpen aria-hidden="true" focusable="false" data-scope="reserved" data-part="lock" />;
}

/** An answer's button props: all but what the dialog reads itself. */
function buttonOf(action: Partial<DialogAction> | undefined): Omit<ButtonProps, "onPress"> {
  const props: Partial<DialogAction> = { ...action };
  delete props.onPress;
  delete props.reserve;
  delete props.reserveGrant;
  delete props.byChord;
  delete props.byReturn;
  return props as Omit<ButtonProps, "onPress">;
}

/** The answers in their places: Cancel, the other, the main action. */
function DialogAnswerRow({
  cancel,
  main,
  alternative,
  onClose,
}: Answers & { onClose: () => void }) {
  const out = cancel === false ? undefined : cancel;
  const mainRest = buttonOf(main);
  const outRest = buttonOf(out);
  const otherRest = buttonOf(alternative);
  const submitting = submits(main);
  return (
    <ButtonRow size="md">
      {cancel !== false && (
        <Button {...outRest} data-dialog-action="cancel" onPress={out?.onPress ?? onClose}>
          {out?.children !== undefined ? words(out as DialogAction) : "Cancel"}
        </Button>
      )}
      {alternative !== undefined && (
        <Button {...otherRest} data-dialog-action="alternative" onPress={alternative.onPress}>
          {words(alternative)}
        </Button>
      )}
      {main !== undefined && (
        <Button
          variant="primary"
          {...mainRest}
          data-dialog-action="main"
          type={submitting ? "submit" : "button"}
          onPress={submitting ? undefined : main.onPress}
          icon={main.reserveGrant && main.variant !== "grants" ? <LockRoom /> : mainRest.icon}
        >
          {words(main)}
        </Button>
      )}
    </ButtonRow>
  );
}

/**
 * **A dialog's answers, drawn from inside its body**: for a part of a dialog that holds its own
 * steps (a guided sign-in, a form shared with a pane), whose answers are its own state. They are
 * drawn in the footer, in their places, instead of the dialog's own; ⏎ is their main action.
 * DialogFrame outside a dialog of the family, it draws nothing.
 */
export function DialogAnswers(answers: Answers) {
  const place = useContext(FooterPlace);
  const main = answers.main;
  useLayoutEffect(() => {
    if (!place) return;
    place.claim(1);
    return () => place.claim(-1);
  }, [place]);
  useLayoutEffect(() => {
    if (!place) return;
    place.setSubmit(
      main !== undefined && submits(main) && !main.isDisabled ? main.onPress : undefined,
    );
  });
  if (!place) return null;
  return (
    <>
      {place.actions &&
        createPortal(<DialogAnswerRow {...answers} onClose={place.onClose} />, place.actions)}
      {place.summary &&
        answers.summary !== undefined &&
        createPortal(answers.summary, place.summary)}
    </>
  );
}

/**
 * Whether what calls it is drawn in a dialog of the family's body, whose answers belong in its
 * footer (`DialogAnswers`): a part drawn in a dialog and elsewhere too keeps its own row only
 * elsewhere.
 */
export function useInADialog(): boolean {
  return useContext(FooterPlace) !== null;
}

/** The ids a dialog is named and described by. */
function useNames({
  titleId,
  subtitle,
  description,
  "aria-describedby": describedBy,
}: Pick<DialogProps, "titleId" | "subtitle" | "description" | "aria-describedby">) {
  const made = useId();
  const titled = titleId ?? `${made}-title`;
  const subtitled = `${made}-subtitle`;
  const described = `${made}-description`;
  const describedByAll =
    describedBy ??
    ([subtitle !== undefined && subtitled, description !== undefined && described]
      .filter(Boolean)
      .join(" ") ||
      undefined);
  return { titled, subtitled, described, describedByAll };
}

/** **The three rows**: the header, the body and the footer, with the form that is both. */
function DialogRows({ props, names }: { props: DialogProps; names: ReturnType<typeof useNames> }) {
  const { title, subtitle, description, summary, cancel, main, alternative, children, onClose } =
    props;
  const { titled, subtitled, described } = names;
  const [actions, setActions] = useState<HTMLElement | null>(null);
  const [summaryAt, setSummaryAt] = useState<HTMLElement | null>(null);
  const [claimed, setClaimed] = useState(0);
  const submitFromInside = useRef<(() => void) | undefined>(undefined);
  const place = useMemo<Place>(
    () => ({
      actions,
      summary: summaryAt,
      onClose,
      setSubmit: (main) => {
        submitFromInside.current = main;
      },
      claim: (by) => setClaimed((was) => was + by),
    }),
    [actions, summaryAt, onClose],
  );
  const inside = claimed > 0;
  const submit = (event: FormEvent) => {
    // Only the dialog's own form: a submit from anything inside it is that thing's.
    if (event.target !== event.currentTarget) return;
    event.preventDefault();
    if (inside) submitFromInside.current?.();
    else if (main !== undefined && submits(main) && !main.isDisabled) main.onPress?.();
  };
  const chord = (event: ReactKeyboardEvent<HTMLFormElement>) => {
    if (!chordSends(event.nativeEvent) || !event.currentTarget.contains(event.target as Node))
      return;
    if (
      inside ||
      main?.byChord !== true ||
      main.isDisabled === true ||
      main.variant === "grants" ||
      main.variant === "danger"
    )
      return;
    event.preventDefault();
    main.onPress?.();
  };
  return (
    <>
      <header data-scope="dialog" data-part="header">
        <h2 id={titled} data-scope="dialog" data-part="title">
          {title}
        </h2>
        {subtitle !== undefined && (
          <p id={subtitled} data-scope="dialog" data-part="subtitle">
            {subtitle}
          </p>
        )}
      </header>
      {/* The field family's form (#2905): its fields check themselves before it is sent, and
          ⌘⏎ sends its main action from anywhere in it, as Return does from a field. */}
      <Form data-scope="dialog" data-part="form" onSubmit={submit} onKeyDown={chord}>
        <FooterPlace.Provider value={place}>
          <div data-scope="dialog" data-part="body">
            {description !== undefined && (
              <div id={described} data-scope="dialog" data-part="description">
                {description}
              </div>
            )}
            {children}
          </div>
        </FooterPlace.Provider>
        <div data-scope="dialog" data-part="separator" aria-hidden="true" />
        <footer data-scope="dialog" data-part="footer">
          <div ref={setSummaryAt} data-scope="dialog" data-part="summary">
            {inside ? null : summary}
          </div>
          <div ref={setActions} data-scope="dialog" data-part="actions">
            {inside ? null : (
              <DialogAnswerRow
                cancel={cancel}
                main={main}
                alternative={alternative}
                onClose={onClose}
              />
            )}
          </div>
        </footer>
      </Form>
    </>
  );
}

/** What of a dialog's props is its modal's. */
function surfaceOf(props: DialogProps): Omit<Surface, "aria-describedby" | "data-testid"> {
  const { isOpen, onClose, size, height, initialFocus, afterClose, returnFocus, className } = props;
  return { isOpen, onClose, size, height, initialFocus, afterClose, returnFocus, className };
}

function DialogFrame({ alert, props }: { alert: boolean; props: DialogProps }) {
  const names = useNames(props);
  const surface = surfaceOf(props);
  return (
    <DialogModal
      {...surface}
      role={alert ? "alertdialog" : "dialog"}
      labelledBy={names.titled}
      describedBy={names.describedByAll}
      testId={props["data-testid"]}
    >
      <DialogRows props={props} names={names} />
    </DialogModal>
  );
}

/**
 * **A dialog drawn in place**, not over the window: the family's look and rows, and nothing of
 * the modal (no scrim, no held keyboard, no portal). For the theme specimen's picture of a dialog
 * and for the family's stories; never for a question the window asks.
 */
export function DialogInPlace({ alert = false, ...props }: DialogProps & { alert?: boolean }) {
  const names = useNames(props);
  const style =
    props.height === undefined
      ? undefined
      : ({ "--dialog-height": `${props.height}px` } as CSSProperties);
  return (
    <section
      role={alert ? "alertdialog" : "dialog"}
      aria-labelledby={names.titled}
      aria-describedby={names.describedByAll}
      className={props.className}
      style={style}
      data-scope="dialog"
      data-part="root"
      data-variant={alert ? "alert" : "dialog"}
      data-size={props.size ?? "standard"}
      data-fixed-height={props.height === undefined ? undefined : ""}
      data-in-place=""
    >
      <DialogRows props={props} names={names} />
    </section>
  );
}

/** **A dialog**: a form, a choice, a view of something, answered from its footer. */
export function Dialog(props: DialogProps) {
  return <DialogFrame alert={false} props={props} />;
}

/**
 * **An alert dialog**: a question that interrupts, usually about something that cannot be taken
 * back. It starts on its safe answer (#2980), and is announced as an interruption.
 */
export function AlertDialog(props: DialogProps) {
  return <DialogFrame alert props={props} />;
}
