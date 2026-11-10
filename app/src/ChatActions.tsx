// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Button } from "./ui/button/Button";

/**
 * **The same actions on every chat row** (ADR 0121 decision 21, #6013): Answer, when it asks; Go
 * to chat; Snooze; Stop, in that order, on a row of Asks, of All chats and (later, #6014) a
 * Stuck? item. One component, so no row can offer them in another order or other words.
 *
 * Go to chat, Snooze and Stop are always there, so the row never changes shape as its chat
 * moves: one that has nothing to snooze or stop is off, and says why. Answer is there only while
 * the chat asks something.
 */
export type ChatActionsProps = {
  /** The chat, by its name: what each action's name says it is about. */
  who: string;
  /** **Answer**, while the chat asks something: opens what answers it. `open` where that is
   *  in the row and open now, so the press reads Cancel; `controls` names what it opens. */
  answer?: { onPress: () => void; open?: boolean; controls?: string };
  /** Go to chat: the chat in front, its project brought forward where it is not. Nothing for
   *  a row about no chat (a sign-in's ask), which then offers its Answer alone. */
  onGo?: () => void;
  /** Snooze: its asks put away until it asks something new. Nothing where it asks nothing. */
  onSnooze?: () => void;
  /** Stop. Nothing where its program has ended. */
  onStop?: () => void;
};

/** Why Snooze is off: nothing to put away. */
export const NOTHING_TO_SNOOZE = "It asks nothing, so there is nothing to snooze.";
/** Why Stop is off: its program has ended. */
export const NOTHING_TO_STOP = "It has ended, so there is nothing to stop.";

/** A chat row's actions, in the one order. A place draws them in its own row of links. */
export function ChatActions({ who, answer, onGo, onSnooze, onStop }: ChatActionsProps) {
  return (
    <>
      {answer !== undefined && (
        // **A visible button, not a text link** (#5998): what it opens is answered there.
        <Button
          size="sm"
          variant="secondary"
          data-lead=""
          aria-label={answer.open === true ? `Cancel answering ${who}` : `Answer ${who}`}
          aria-expanded={answer.controls === undefined ? undefined : answer.open === true}
          aria-controls={answer.open === true ? answer.controls : undefined}
          onPress={answer.onPress}
        >
          {answer.open === true ? "Cancel" : "Answer"}
        </Button>
      )}
      {onGo !== undefined && (
        <>
          <Button
            size="sm"
            variant="quiet-link"
            aria-label={`Go to chat ${who}`}
            onPress={onGo}
            className="inbox-go"
          >
            Go to chat
          </Button>
          <Button
            size="sm"
            variant="quiet-link"
            aria-label={`Snooze ${who}`}
            isDisabled={onSnooze === undefined}
            tooltip={onSnooze === undefined ? NOTHING_TO_SNOOZE : "Until it asks something new"}
            onPress={onSnooze}
          >
            Snooze
          </Button>
          <Button
            size="sm"
            variant="quiet-link"
            aria-label={`Stop ${who}`}
            isDisabled={onStop === undefined}
            tooltip={onStop === undefined ? NOTHING_TO_STOP : undefined}
            onPress={onStop}
          >
            Stop
          </Button>
        </>
      )}
    </>
  );
}
