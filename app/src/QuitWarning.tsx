// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type OwnWork } from "./bindings";
import { type State } from "./chatState";
import { counted, shownState } from "./shownState";
import { listed } from "./taskAsks";
import { ChatMark } from "./ui/chat-mark/ChatMark";
import { AlertDialog } from "./ui/dialog/Dialog";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * One chat a quit is about to end, whichever project it is in.
 *
 * **Its state travels with it, already looked up.** A session number names a chat only inside
 * its own project — every project numbers its chats from one — so a dialog handed one
 * `ChatStates` and a flat list of sessions would paint project A's `running` onto project B's
 * chat 1 and tell the operator that a chat nobody is running is mid-turn. The pair is the
 * identity, and resolving it here would mean this component holding a map per project for no
 * reason: the window already has both halves.
 */
export type Ending = {
  /** Unique across projects, because a session number is not. */
  key: string;
  /** Which project it is in, drawn when the window holds more than one. */
  project?: string;
  name: string;
  harness: string | null;
  cwd: string | null;
  /** The workspace it is filed in, as the strip names it — for a question that says where. */
  workspace?: string;
  state: State;
  /**
   * **What it waits on of its own** (#3878), as the board says, while its turn has ended on
   * helpers, monitors, background shells or a wake-up still in flight. Ending its program ends
   * them too, so such a chat is asked about as one mid-turn is (#5748).
   */
  ownWork?: OwnWork;
};

/**
 * What quitting asks before it ends anything.
 *
 * It lists the sessions that are about to end and what each one is doing. Until M1.3 it said
 * charter could not tell whether a session was mid-turn; now a harness's own hooks say so
 * (spec decision 3), and the ones that still cannot are named rather than lumped in with the
 * rest. Nothing here is guessed from a session's output, which is the one thing the app never
 * does (ADR 0018).
 *
 * **Every project the window holds, not the one in front.** Quit ends the process, and the
 * process holds them all — a warning that counted only what was on screen would be a warning
 * that understated what it was about to end by however many projects the operator had merged
 * into the window.
 *
 * **Radix's `AlertDialog`** (`docs/ui-primitives.md`, #630): its answer ends every chat, so it
 * is the question whose answer destroys something, and a screen reader is handed the sentence
 * about what is lost first. It is what makes "over everything" true rather than drawn: the rest
 * of the window is inert and out of the accessibility tree while it is up, and the keyboard
 * cannot leave it for a pane behind it. Its title counts chats, the word the window uses for
 * them (CONTEXT.md).
 *
 * **Escape answers it now, and did not before.** It answers what Cancel answers — nothing is
 * ended, and the core is told, so the next quit warns again rather than going straight out. A
 * click outside answers nothing at all.
 */
export function QuitWarning({
  chats,
  onQuit,
  onCancel,
}: {
  chats: readonly Ending[];
  onQuit: () => void;
  onCancel: () => void;
}) {
  // A click outside answers nothing, as an alert dialog's never does. Cancel and Escape are the
  // two ways out, and Cancel leads with the keyboard on it: the destructive answer is never the
  // one a stray Return key finds.
  return (
    <AlertDialog
      onClose={onCancel}
      title={chats.length === 1 ? "Quitting ends 1 chat" : `Quitting ends ${chats.length} chats`}
      titleId="quit-warning"
      aria-describedby="quit-warning-said"
      main={{ children: "Quit purlis", variant: "danger", onPress: onQuit }}
    >
      <EndingList chats={chats} />
      <div id="quit-warning-said">
        <MidTurnSaid chats={chats} />
      </div>
    </AlertDialog>
  );
}

/**
 * The rows of the chats an act is about to end, each with what it is doing — the quit warning's
 * list, and Restart to update's (`Updates.tsx`), so the two cannot drift apart.
 */
export function EndingList({ chats }: { chats: readonly Ending[] }) {
  // Only when there is more than one: naming the project on every row of a window holding one
  // is a column that says the same thing all the way down.
  const several = new Set(chats.map((chat) => chat.project ?? "")).size > 1;
  return (
    <ul className="ending">
      {chats.map((chat) => (
        <li key={chat.key}>
          <span className="what">{chat.harness ?? "shell"}</span>
          <span>{chat.name}</span>
          <EndingState state={chat.state} ownWork={chat.ownWork} />
          {several && chat.project && <code className="where">{chat.project}</code>}
          {chat.cwd && <code className="where">{chat.cwd}</code>}
        </li>
      ))}
    </ul>
  );
}

/** What a chat about to end is doing: its board's word on the one mark (#5340). */
function EndingState({ state, ownWork }: { state: State; ownWork: OwnWork | undefined }) {
  // All this list knows is the board's word and its own work: a turn that ended waits on the
  // person, unless it waits on work of its own (#3878).
  const waitsOnItsOwn = state === "waiting" && ownWork !== undefined;
  const shown = shownState({
    board: state,
    needsYou: state === "waiting" && !waitsOnItsOwn,
    task: null,
    harness: null,
    ownWork: waitsOnItsOwn ? ownWork : undefined,
  });
  if (shown === undefined) return null;
  return (
    <Tooltip tip={shown.word}>
      <span role="img" aria-label={shown.word}>
        <ChatMark state={shown.mark} />
      </span>
    </Tooltip>
  );
}

/**
 * Whether ending `chat` could interrupt something: it says it is mid-turn, it says nothing at
 * all, or its turn ended on work of its own still in flight (#5748).
 */
export function wouldInterrupt(chat: Ending): boolean {
  return chat.state === "running" || chat.state === "unknown" || waitsOnItsOwnWork(chat);
}

/** Whether `chat`'s turn ended on work of its own still in flight (#3878). */
function waitsOnItsOwnWork(chat: Ending): chat is Ending & { ownWork: OwnWork } {
  return chat.state === "waiting" && chat.ownWork !== undefined;
}

/**
 * **What ending these chats' own work ends**, added up (#5748): `1 monitor and 2 background
 * shells`, in the board's words for each kind (`ownWorkSaid`). A wake-up is counted, not timed:
 * across several chats a list of clock times says less than how many are lost.
 */
export function ownWorkEnded(works: readonly OwnWork[]): string {
  const sum = (kind: "helpers" | "workflows" | "monitors" | "shells") =>
    works.reduce((total, work) => total + work[kind], 0);
  const parts = [
    counted(sum("helpers"), "helper"),
    counted(sum("workflows"), "workflow"),
    counted(sum("monitors"), "monitor"),
    counted(sum("shells"), "background shell"),
    counted(works.filter((work) => work.wakes_at != null).length, "wake-up"),
  ].filter((part): part is string => part !== undefined);
  return listed(parts);
}

/** The sentence for the chats about to end whose turn ended on work of their own (#5748). */
function ownWorkSentence(chats: readonly (Ending & { ownWork: OwnWork })[]): string {
  const ended = ownWorkEnded(chats.map((chat) => chat.ownWork));
  const who =
    chats.length === 1
      ? `${chats[0].name} is waiting on its own work`
      : `${chats.length} chats are waiting on their own work`;
  return ended === "" ? `${who}, which will end.` : `${who}: ${ended} will end.`;
}

/**
 * **What ending one chat's program would interrupt**, in the one wording every question that
 * ends a chat uses: the quit warning's rows below, and Start fresh's question (`ChatAsk`, #1246).
 * A chat that says it is mid-turn, and one that reports no state, so charter cannot tell. Nothing
 * for a chat that is waiting, done or failed.
 */
export function oneChatMidTurn(name: string, state: State): string | undefined {
  if (state === "running") return `${name} is mid-turn and will be interrupted.`;
  if (state === "unknown")
    return `${name} reports no state, so purlis cannot tell whether it is mid-turn.`;
  return undefined;
}

/**
 * What the chats about to be ended are doing: which are mid-turn, which cannot say, and what
 * of their own work ends with those whose turn ended on it (#5748).
 */
export function MidTurnSaid({ chats }: { chats: readonly Ending[] }) {
  const running = chats.filter((chat) => chat.state === "running");
  const unknown = chats.filter((chat) => chat.state === "unknown");
  const onTheirOwn = chats.filter(waitsOnItsOwnWork);
  return (
    <>
      {running.length > 0 && (
        <p className="honest mid-turn" role="alert">
          {running.length === 1
            ? oneChatMidTurn(running[0].name, "running")
            : `${running.length} chats are mid-turn and will be interrupted.`}
        </p>
      )}
      {unknown.length > 0 && (
        <p className="honest">
          {/* Named, not counted into the reassuring number. A harness that reports nothing
            could be mid-turn and purlis would never know — saying "nothing is running"
            over the top of it would be the app claiming something it cannot see. */}
          {unknown.length === 1
            ? oneChatMidTurn(unknown[0].name, "unknown")
            : `${unknown.length} chats report no state, so purlis cannot tell whether they are mid-turn.`}
        </p>
      )}
      {onTheirOwn.length > 0 && (
        // Said as the mid-turn line is: what it names ends with the chat, and nothing brings
        // it back (#5748). It warns; the answer stays the person's.
        <p className="honest mid-turn" role="alert">
          {ownWorkSentence(onTheirOwn)}
        </p>
      )}
      {running.length === 0 && unknown.length === 0 && onTheirOwn.length === 0 && (
        <p className="honest">No chat is mid-turn.</p>
      )}
    </>
  );
}

/**
 * What a restart asks when a chat could be mid-turn: restart now, or wait. Restart to update
 * asks it, and so does the restart onto the session bus (`SessionBusNotice.tsx`).
 *
 * The quit warning's rows and sentences (above), because it is the same act for
 * those chats — they are ended — with one difference the words carry: the restart offers them
 * back. A chat that reports no state is asked about too, for the quit warning's reason: it could
 * be mid-turn and charter would never know.
 *
 * **The family's `AlertDialog`**, per `docs/ui-primitives.md`: it arrives because of what the
 * operator pressed, a click outside answers nothing, and its way out — **Wait** — leads and has
 * the keyboard, so the answer a stray Return or Escape finds interrupts nothing. The list is
 * live: a chat that finishes its turn while this is up leaves it.
 */
export function MidTurn({
  chats,
  onWait,
  onRestart,
  title = "Restart to update",
}: {
  chats: readonly Ending[];
  onWait: () => void;
  onRestart: () => void;
  /** Which restart it is: to update, or onto the session bus (`SessionBusNotice.tsx`). */
  title?: string;
}) {
  return (
    <AlertDialog
      onClose={onWait}
      title={title}
      aria-describedby="restart-mid-turn-said"
      cancel={{ children: "Wait" }}
      main={{ children: "Restart now", variant: "danger", onPress: onRestart }}
    >
      <EndingList chats={chats} />
      <div id="restart-mid-turn-said">
        <MidTurnSaid chats={chats} />
        <p className="honest">
          Every chat is offered back when purlis starts again. Wait to let a turn or a chat&apos;s
          own work finish, or restart now.
        </p>
      </div>
    </AlertDialog>
  );
}
