// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef, useState } from "react";
import { ENDS_IT, type Offer } from "./actions";
import { Choice, SettingRow } from "./settings/components";
import type { WindowDraft } from "./bindings";
import { keepSays } from "./themeDrafts";
import { AlertDialog } from "./ui/dialog/Dialog";

/** The two answers about the chats at work below a closing chat. */
const KEEP = "keep";
const STOP = "stop";

/**
 * What charter asks before it ends a chat.
 *
 * **The operator asked for it in the same breath as the pane controls**: *"closing session
 * should ask confirmation"*. Until now the only guard was the words — `End chat 3 steward` on
 * the `×`, and a tooltip saying *"Ends the program it runs. There is no undo."* That was the
 * fix for charter-app#130, where a glyph reading "hide this tab" was ending live harnesses,
 * and it was the right fix for a *name*. It is not a guard against the press itself, and the
 * press is now one of several: a `×` on a tab, a `×` on a pane that appears under the pointer
 * on hover, and a row of the palette.
 *
 * **Every route through it, because there is one list of actions.** This is asked in
 * `PlaneView`'s `run`, which is what carries out a row from whichever surface pressed it — so
 * the palette's `End chat 3 steward` asks exactly as the tab's `×` does. A confirmation on one
 * surface and not another is the second answer this app's catalogue exists to not have.
 *
 * **Radix's `AlertDialog` and not its `Dialog`**, which is not a style choice: an alert dialog
 * is `role="alertdialog"`, it is announced as an interruption rather than as a surface, and
 * the primitive requires a `Cancel` that the focus goes to. The four dialogs in
 * `docs/ui-primitives.md` are questions the operator went looking for; this one arrives
 * *because of* something they did, which is the distinction the role exists for.
 *
 * The two house rules for a modal here are the four dialogs' (`docs/ui-primitives.md`), and
 * this primitive keeps both without being told:
 *
 * - **A click outside answers nothing.** The four `Dialog`s prevent `onInteractOutside` by
 *   hand; `AlertDialogContent` does not take the prop at all, because it refuses outside
 *   interaction itself.
 * - **Escape answers, with the non-destructive answer** — the primitive's own `Cancel`.
 *
 * And one that is this dialog's alone: **Cancel is first, and the focus starts on the answer the
 * operator ruled the default** (ADR 0064). Since Smart close there are three answers — Cancel,
 * Close, Smart close — and the focus goes to Smart close on a chat with turns behind it, to
 * Close on one that has had at most one turn (little to record), and to Cancel wherever
 * charter cannot say (`firstAnswer`). Before Smart close, Cancel was always focused; a stray
 * Return on a chat charter knows nothing about still cancels.
 *
 * **Every answer carries `tabIndex={0}`.** Radix's `FocusScope`
 * intercepts Tab only at the EDGES of the scope: on the first tabbable it acts on Shift+Tab and
 * moves the focus to the last itself, on the last it acts on Tab and moves to the first, and in
 * between it does nothing and the engine decides. **The engine here is WebKit on both platforms
 * charter ships to, and WebKit leaves a `<button>` out of the tab sequence** unless "tab to all
 * controls" is on — **or the button's `tabindex` is written down**, which is the whole of the
 * fix and is the engine's own rule rather than a workaround
 * (`HTMLFormControlElement::isKeyboardFocusable`; `docs/ui-primitives.md` cites the change).
 *
 * Until charter-app#186 this dialog was whole for a narrower reason: it had two tabbables and
 * the confirm was the second, so Cancel WAS the first edge and the confirm the last, and
 * Shift+Tab from Cancel was Radix's own `focus()` call rather than the engine's tab sequence.
 * That was a property of the *number of buttons*, and the third one Smart close added is why it
 * had to stop being one. `App.test.tsx` and `SmartClose.test.tsx` pin the order and the focus.
 *
 * **What the above is NOT is the reason a scenario cannot press these buttons**, and an earlier
 * version of this comment said it was. Measured in charter-app#176 with a keydown trace in the
 * real WebView: `Enter` on a focused `Cancel` arrives AT that button, unprevented, and does not
 * activate it — WebDriver key actions carry no implicit activation. That is the harness, not the
 * engine and not this dialog, and it is why the keyboard half of the claim is tested in
 * `App.test.tsx` and not in `palette.e2e.ts`. Two findings, one true of the product and one true
 * only of the test rig; keeping them apart is the whole point of writing them down.
 */
/** The one line a close says of the tasks among what it closes. */
export const BACK_SAYS = (back: readonly string[]) =>
  back.length === 1
    ? `${back[0]} is a task: its tab goes, it is not ended, and it stays in the Chats list.`
    : `${back.join(", ")} are tasks: their tabs go, they are not ended, and they stay in the Chats list.`;

export function EndingChat({
  offer,
  smart,
  running = [],
  closing = [],
  keeps = false,
  shows,
  back = [],
  ownTabs = 0,
  drafts = [],
  fromALink = false,
  onEnd,
  onSmartClose,
  onCancel,
}: {
  /** The catalogue row waiting on an answer — `tab.close:<id>` or `pane.close`. Its title is
   *  what the dialog is about, so there is no second wording of what is being ended. */
  offer: Offer;
  /** Whether the chat is offered **Smart close** (ADR 0064), as the core answered — or none,
   *  when charter could not say, which offers Close only. */
  smart?: SmartAsk;
  /**
   * The chats at work below this chat, by name: its persona chats that have not reported, the
   * chats it handed work to that are mid-turn, and the same below those. With any, the dialog
   * asks once what becomes of them, and the answer rides whichever close is pressed.
   */
  running?: readonly string[];
  /** Its persona chats that have reported and close with it, by name. */
  closing?: readonly string[];
  /** For a close of several chats at once, which asks nothing about them: whether any has
   *  chats at work below it. They keep running, and the dialog says so. */
  keeps?: boolean;
  /** The task the tab shows in place of its session's own chat, where it shows one (#1486):
   *  the dialog says the close is the session's, since the task is what is on screen. */
  shows?: { task: string; session: string };
  /** The tasks among what is being closed, by name (#1488): a close ends no task. Their tabs
   *  go, they go back to the Chats list, and the dialog says so in one line. */
  back?: readonly string[];
  /** How many tasks below the closing session have a tab of their own (#1489): the dialog
   *  says so, since those tabs go with the answer about the tasks. */
  ownTabs?: number;
  /** The chat's theme drafts, by theme folder (#2332): the dialog asks Keep or Revert, and
   *  neither close is offered until it is answered, so nothing is kept or dropped silently. */
  drafts?: readonly DraftClosing[];
  /** Whether a link asked for this close (#2761): the dialog says so, and closes nothing until
   *  it is answered here. */
  fromALink?: boolean;
  /** `stop` is the answer about the running persona chats: stop them, or keep them running.
   *  `keepDrafts` is the answer about its theme drafts, where it has any. */
  onEnd: (stop: boolean, keepDrafts?: boolean) => void;
  onSmartClose: (stop: boolean, keepDrafts?: boolean) => void;
  onCancel: () => void;
}) {
  // Keep is where it starts: the answer that ends nothing more than was asked for.
  const [stop, setStop] = useState(false);
  // No answer about the drafts until the person gives one.
  const [drafted, setDrafted] = useState<"" | typeof KEEP_DRAFTS | typeof REVERT_DRAFTS>("");
  const unanswered = drafts.length > 0 && drafted === "";
  const keepDrafts = drafts.length > 0 ? drafted === KEEP_DRAFTS : undefined;
  const cancel = useRef<HTMLButtonElement>(null);
  const close = useRef<HTMLButtonElement>(null);
  const smartClose = useRef<HTMLButtonElement>(null);
  // A close a link asked for starts on Cancel: a Return the person meant for something else
  // never ends a chat they did not ask to close here.
  const first = fromALink ? "cancel" : firstAnswer(smart);
  return (
    <AlertDialog
      onClose={onCancel}
      title={`${offer.title}?`}
      // The catalogue's own sentence, not a second one written here. It is the same string the
      // `×`'s tooltip has carried since charter-app#130.
      description={
        <p className="honest mid-turn">
          {ENDS_IT} {smart?.available ? SMART_CLOSE_SAYS : null}
        </p>
      }
      // **The focus is put on the default answer** (`firstAnswer`): Smart close for a chat with
      // turns behind it, Close for one with at most one (the operator's ruling, ADR 0064), and
      // Cancel wherever charter cannot say — so a stray Return never ends a chat charter knows
      // nothing about.
      initialFocus={{ cancel, close, smart: smartClose }[first]}
      // Cancel first and Smart close last, at the edge where the main answer sits.
      cancel={{ ref: cancel }}
      alternative={{
        children: "Close",
        variant: "danger",
        ref: close,
        isDisabled: unanswered,
        tooltip: unanswered ? CHOOSE_DRAFTS : undefined,
        onPress: () => (keepDrafts === undefined ? onEnd(stop) : onEnd(stop, keepDrafts)),
      }}
      main={{
        children: "Smart close",
        ref: smartClose,
        isDisabled: !smart?.available || unanswered,
        tooltip: unanswered ? CHOOSE_DRAFTS : undefined,
        "aria-describedby": smart?.why ? "smart-close-why" : undefined,
        onPress: () =>
          keepDrafts === undefined ? onSmartClose(stop) : onSmartClose(stop, keepDrafts),
        className: "smart-close",
      }}
    >
      {fromALink && <p className="honest">{FROM_A_LINK_SAYS}</p>}
      {smart?.why && (
        <p className="honest" id="smart-close-why">
          {smart.why}
        </p>
      )}
      {/* The tab shows a task: what ends is the session, not what is on screen. */}
      {shows !== undefined && (
        <p className="honest">
          This tab is showing {shows.task}, a task of {shows.session}. Closing the tab ends{" "}
          {shows.session}, not {shows.task}.
        </p>
      )}
      {/* Tasks among them are not ended by a close: said, so the count is not a surprise. And
          its tasks in tabs of their own: no tab of theirs outlives the session's (#1489). One
          line for both: they are the same thing, a task whose tab goes. */}
      {(back.length > 0 || ownTabs > 0) && <p className="honest">{TASK_TABS_SAY(back, ownTabs)}</p>}
      {/* What else this close closes, said before it is answered. */}
      {closing.length > 0 && <p className="honest">{CLOSING_SAYS(closing)}</p>}
      {/* Several chats at once: nothing is asked about what each started, so it is said. */}
      {keeps && <p className="honest">{KEEPS_SAYS}</p>}
      {/* **Asked once, here, with both answers** (`RUNNING_SAYS`): the chats this one asked for
          are not ended by its close unless the person says so. */}
      {running.length > 0 && (
        // The settings set's radio group (#630, DS-8), each answer saying what it does under
        // itself.
        <SettingRow
          label={RUNNING_SAYS(running)}
          grouped
          control={(ids) => (
            <Choice
              ids={ids}
              kind="radio"
              options={[
                { value: KEEP, label: "Keep them running", says: KEEP_SAYS },
                { value: STOP, label: "Stop them", says: STOP_SAYS },
              ]}
              value={stop ? STOP : KEEP}
              onValueChange={(to) => setStop(to === STOP)}
            />
          )}
        />
      )}
      {/* **Its theme drafts are asked about here** (#2332): Keep puts each in the folder it is a
          draft of, Revert deletes it. */}
      {drafts.length > 0 && (
        <SettingRow
          label={DRAFTS_SAY(drafts.map((one) => one.folder))}
          grouped
          control={(ids) => (
            <Choice
              ids={ids}
              kind="radio"
              options={[
                { value: KEEP_DRAFTS, label: "Keep", says: KEEP_DRAFTS_SAY(drafts) },
                { value: REVERT_DRAFTS, label: "Revert", says: REVERT_DRAFTS_SAYS },
              ]}
              value={drafted === "" ? undefined : drafted}
              onValueChange={(to) => setDrafted(to === KEEP_DRAFTS ? KEEP_DRAFTS : REVERT_DRAFTS)}
            />
          )}
        />
      )}
    </AlertDialog>
  );
}

/** What the dialog says of a close a link asked for (#2761). */
export const FROM_A_LINK_SAYS =
  "A link asked for this close. Nothing closes until you answer here.";

/**
 * **What the dialog says of the tasks whose tabs go with this close, in one line** (#1488,
 * #1489): the tasks among what is being closed, by name, and how many of the closing
 * session's tasks have a tab of their own. Neither is ended by a close.
 */
/** The two answers about a closing chat's theme drafts (#2332). */
const KEEP_DRAFTS = "keep-drafts";
const REVERT_DRAFTS = "revert-drafts";

export function DRAFTS_SAY(drafts: readonly string[]): string {
  return drafts.length === 1
    ? `This chat has a theme draft, ${drafts[0]}.`
    : `This chat has theme drafts: ${drafts.join(", ")}.`;
}

/** A closing chat's theme draft, as the close question says what Keep does with it. */
export type DraftClosing = Pick<
  WindowDraft,
  "folder" | "kept_to" | "workspace" | "replaces" | "changed"
>;

/** What Keep does with each draft: where it puts it, and what it replaces there; then the one
 *  question who draws it. */
export function KEEP_DRAFTS_SAY(drafts: readonly DraftClosing[]): string {
  const each = drafts.map((one) =>
    drafts.length === 1 ? keepSays(one) : `${one.folder}: ${keepSays(one)}`,
  );
  return `${each.join(" ")} Then you are asked who draws it.`;
}

export const REVERT_DRAFTS_SAYS = "Each is deleted, and the theme as it was kept is drawn.";

export const CHOOSE_DRAFTS = "Choose Keep or Revert for the theme drafts first.";

export function TASK_TABS_SAY(back: readonly string[], ownTabs: number): string {
  return [back.length > 0 ? BACK_SAYS(back) : "", ownTabs > 0 ? OWN_TABS_SAYS(ownTabs) : ""]
    .filter((said) => said !== "")
    .join(" ");
}

/**
 * What a close says of the closing session's tasks that have a tab of their own (#1489,
 * V100-39): how many, and that their tabs go either way. A task kept running is in the Chats
 * list; one that is stopped ends.
 */
export function OWN_TABS_SAYS(count: number): string {
  const tasks =
    count === 1
      ? "1 of its tasks has a tab of its own"
      : `${count} of its tasks have tabs of their own`;
  const those = count === 1 ? "That tab closes" : "Those tabs close";
  const kept = count === 1 ? "a task that goes on working" : "tasks that go on working";
  return `${tasks}. ${those} with this one: ${kept} stay in the Chats list.`;
}

/** What the dialog asks about the chats at work below a closing chat. */
export function RUNNING_SAYS(running: readonly string[]): string {
  const count = running.length === 1 ? "1 chat" : `${running.length} chats`;
  return `${count} it started ${running.length === 1 ? "is" : "are"} still at work: ${running.join(", ")}.`;
}

/** What keeping them does, said under the choice. */
export const KEEP_SAYS =
  "They go on working. A task's report goes to this chat's workspace, where the next chat to start reads it.";

/** What stopping them does, said under the choice. */
export const STOP_SAYS =
  "They are stopped now, and so are the chats they started: each gets one short turn to write what it did, then ends. There is no undo.";

/** What the dialog says of the reported tasks that close with a closing chat. */
export function CLOSING_SAYS(closing: readonly string[]): string {
  const count =
    closing.length === 1 ? "1 reported task closes" : `${closing.length} reported tasks close`;
  return `${count} with it, each once its session record is written: ${closing.join(", ")}.`;
}

/** What a close of several chats says where any has chats at work below it. */
export const KEEPS_SAYS =
  "The chats these chats started keep running. Close one chat at a time to be asked about them.";

/** What **Smart close** does, said beside Close's cost when it is offered. */
export const SMART_CLOSE_SAYS =
  "Smart close first asks the chat to write its session record, and closes it once the record is saved.";

/** Whether a chat is offered Smart close, as the dialog draws it: the core's answer
 *  (`smart_close_offer`), or the window's own reason where the close is about more than one chat. */
export type SmartAsk = {
  available: boolean;
  why: string | null;
  /** Close is the default: the chat has had at most one turn. */
  close_first: boolean;
};

/** The answer the dialog's focus starts on (ADR 0064). */
export function firstAnswer(smart: SmartAsk | undefined): "smart" | "close" | "cancel" {
  if (smart === undefined) return "cancel";
  if (smart.close_first) return "close";
  return smart.available ? "smart" : "cancel";
}
