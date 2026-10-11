/** What a chat is doing, as the tab draws it, and the queue of chats asking for you. */
import { Hand } from "lucide-react";
import { useLayoutEffect, useRef } from "react";
import type { Offer } from "./actions";
import type { State } from "./chatState";
import type { PersonaMarkData } from "./PersonaMark";
import { useArrived } from "./lib/arrived";
import { moveAlong } from "./tabSequence";

/** The word beside a chat's name. */
const WORDS: Record<State, string> = {
  unknown: "unknown",
  running: "running",
  waiting: "waiting on you",
  done: "done",
  failed: "failed",
};

export function ChatState({ state }: { state: State }) {
  // A state this mark CHANGED to, rather than the one it was drawn in: only the first moves,
  // so a workspace's tabs coming back into view do not all pulse at once (`useArrived`).
  const arrived = useArrived(state);
  return (
    <span
      className={`state state-${state}${arrived ? " arrived" : ""}`}
      data-state={state}
      // The word, never only a colour: a state told apart by colour alone is no state at all
      // to anyone who cannot see it, and `title` alone is no use on a touch screen or to a
      // screen reader reading a list. The mark is decorative; the label carries the meaning.
      role="img"
      aria-label={WORDS[state]}
      title={WORDS[state]}
    />
  );
}

/** {@link ChatState} where a chat has a mark to draw, and nothing where it has none (`markOf`). */
export function ChatMark({ state }: { state: State | undefined }) {
  return state === undefined ? null : <ChatState state={state} />;
}

/** What a wrapping-up chat's tab and explorer row say about it, as their tooltip. */
export const WRAPPING_UP =
  "Wrapping up: writing its session record, then it closes. Typing into it, or Cancel smart close on its menu, stops this.";

/** What a tab in the background says, as its tooltip and its name (SI-8f): the chip draws no
 *  name of its own. */
export function chipSays(name: string): string {
  return `${name} — wrapping up`;
}

/**
 * **A chat wrapping up** — being smart-closed (ADR 0064): a mark that breathes, as a queued
 * pipeline's does, because the chat is doing its last turn and will go. `charter-breathe` reads
 * its timing from the motion tokens, so under reduced motion it stands still at full weight.
 * The word is its accessible name, never only the colour.
 */
export function WrappingUp({ held }: { held: boolean }) {
  if (!held) return null;
  return (
    <span
      className="wrapping-up breathing"
      data-mark="wrapping-up"
      role="img"
      aria-label="wrapping up"
      title={WRAPPING_UP}
    />
  );
}

/** What a stopping chat's tab says about it, as its tooltip (#1459): what its row says, in full. */
export const STOPPING = "Stopping: it is writing what it did, then it ends. Its menu ends it now.";

/**
 * **A chat being stopped** (#1448, #1459): its tab's mark, as its row in the Chats list says
 * "Stopping…". A square, the stop's own shape, so it is told from a state and from a wrap-up's
 * diamond without colour; it breathes while the last turn runs and stands still under reduced
 * motion. The word is its accessible name.
 */
export function StoppingMark({ held }: { held: boolean }) {
  if (!held) return null;
  return (
    <span
      className="stopping-mark breathing"
      data-mark="stopping"
      role="img"
      aria-label="stopping"
      title={STOPPING}
    />
  );
}

/** One chat asking for the operator, as its project reports it to the window. */
export type Asking = {
  session: number;
  /** What the chat is called there (`nameOf`), so a chat's name reads the same everywhere. */
  name: string;
  /** The workspace it is filed in, already said as the strip says it. */
  workspace: string;
  /** The persona it runs as, when it runs as one, and that persona's mark (#1449). Handed
   *  over whole, because the list is the window's and reads no one project's marks. */
  persona?: string | null;
  mark?: PersonaMarkData | null;
  /**
   * The chats that have reported back to this one and not been read, by name (charter-app#259).
   * A report is no item of its own (#1448); a chat that is here for another reason says
   * `<child> reported back` —
   * and Go still opens THIS chat, the one that asked, whose next turn is handed the report.
   */
  reported?: readonly string[];
  /** The chats it started that the operator stopped, by name (#1448): its row says
   *  `<child> was stopped`. */
  stoppedBelow?: readonly string[];
  /** Why the app found the chat needs the operator (#1448): a report of its own with nowhere to
   *  go. Said first, before anything about the chats it started. */
  needed?: string;
  /**
   * Why the chat needs the operator when it is not that it asked: a commit of its refused
   * (SQ-16). Its row then says `<name>: <why>`, and Go is still the chat. A Smart close that
   * stopped is an update in the Inbox (#1693).
   */
  why?: string;
  /** The catalogue's `needs.show:<session>`: the chat to the front, its workspace with it. */
  go?: Offer;
  /** The catalogue's `needs.ignore:<session>`. */
  ignore?: Offer;
};

/** The same, as the window lists it: with the project it is in, whose rows these are. */
export type Needing = Asking & {
  plane: string;
  /** The project, named as its tab names it. */
  project: string;
};

/** A chat that can be waiting on the operator without being able to say so (charter-app#52):
 *  a shell, or a harness without charter's hooks. */
export type Quiet = {
  name: string;
  /** The project it is in, named as its tab names it. */
  project: string;
  /** That project, and the chat's session in it: what the Inbox's Go to chat brings forward,
   *  in whichever window holds it (#1695). */
  plane: string;
  session: number;
};

/** What the faint hand says, in its name and its tooltip. */
export function quietSaid(quiet: readonly Quiet[]): string {
  return quiet.length === 1
    ? `Nothing has asked for you, but ${quiet[0].name} can't tell purlis it's waiting`
    : `Nothing has asked for you, but ${quiet.length} chats can't tell purlis they're waiting`;
}

/**
 * **The needs-you hand, in the title bar** (charter-app#249): a hand and a count, and nothing at
 * all when nothing needs you. **A press opens the Inbox** (#1692, I-2), where each thing that
 * waits is listed and answered: the hand is the Inbox's count and its way in.
 *
 * It dropped a list of its own, every chat asking across every project, until the Inbox took
 * that over (#1695, #1700): the list drew the window's reports beside the registry's asks, so a
 * chat with a held dispatch was one row more than the number counted. The Inbox lists from the
 * asks registry alone, so there is one list and one number.
 *
 * **Three states, and the middle one is the honest one** (the operator's ruling on #249):
 *
 * - something waits: the hand, and the count;
 * - nothing waits, but a chat that cannot report is open — a shell, a harness without purlis's
 *   hooks (charter-app#52): a **faint hand with no number**, whose name and tooltip say which
 *   chats those are. "Nothing needs you" would be a claim about a chat purlis cannot see, and a
 *   blank bar says it without words;
 * - neither: nothing at all.
 */
export function NeedsYouButton({
  count,
  chats = false,
  quiet,
  onInbox,
}: {
  /** How many things wait on the person: the asks registry's count (#1690). */
  count: number;
  /** Whether `count` counts chats in the queue rather than asks: what the hand counts where the
   *  registry has said nothing yet, said as what it is, never as a count of asks. */
  chats?: boolean;
  /** The chats that can be waiting without saying so, across every project. */
  quiet: readonly Quiet[];
  /** Opens the Inbox. */
  onInbox: () => void;
}) {
  const asking = count > 0;
  const none = !asking && quiet.length === 0;
  // The button appearing because something has just asked, as opposed to having been there when
  // the bar was drawn: only the first is a change worth drawing (`useArrived`).
  const arrived = useArrived(asking);
  /**
   * **The keyboard, when the button goes.** Focus on an element that goes is focus on the page,
   * where the next key does nothing: it goes to the next Tab stop after where the button was,
   * as Tab would (`tabSequence.moveAlong`). `held` is whether the keyboard was on the button.
   */
  const anchor = useRef<HTMLSpanElement>(null);
  const held = useRef(false);
  useLayoutEffect(() => {
    if (!none || !held.current) return;
    held.current = false;
    const lost = document.activeElement === null || document.activeElement === document.body;
    if (lost && anchor.current) moveAlong(anchor.current, false);
  }, [none]);
  const said = !asking
    ? quietSaid(quiet)
    : chats
      ? `${count} ${count === 1 ? "chat needs" : "chats need"} you`
      : `${count} ${count === 1 ? "thing waits" : "things wait"} on you`;
  return (
    // `display: contents`: a place to be next to, not a box in the bar's row.
    <span
      ref={anchor}
      className="needs-you-anchor"
      onFocus={() => {
        held.current = true;
      }}
      onBlur={(event) => {
        if (event.relatedTarget !== null) held.current = false;
      }}
    >
      {!none && (
        // `tabIndex={0}`: WebKit leaves a `<button>` out of the tab sequence unless its
        // `tabindex` is written down (`docs/ui-primitives.md`, charter-app#186), and Tauri's
        // drag handler stops at it either way because it is a `<button>`.
        <button
          type="button"
          className={`needs-you-button${asking ? "" : " muted"}${arrived && asking ? " arrived" : ""}`}
          data-testid="needs-you-button"
          tabIndex={0}
          aria-label={said}
          title={said}
          onClick={onInbox}
        >
          <Hand aria-hidden="true" />
          {asking && <span className="needs-you-number">{count}</span>}
        </button>
      )}
    </span>
  );
}
