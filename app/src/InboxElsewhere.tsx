import { useId } from "react";
import * as RovingFocusGroup from "@radix-ui/react-roving-focus";
import type { Offer } from "./actions";
import { backSaid } from "./chatState";
import { Notice } from "./Notice";
import type { Needing, Quiet } from "./NeedsYou";
import { PersonaMark } from "./PersonaMark";

/**
 * **What waits on the person where this project's asks registry cannot see** (#1695): what the
 * ✋'s list named until it was retired, and the Inbox lists now, so retiring the list lost
 * nothing.
 *
 * - **A chat waiting in another window's project.** That window's registry reads its asks; this
 *   window hears of the chat from what that window tells the others (`windows.ts`), and the
 *   hand's number counts it. So it is an ask here too, listed after this project's own, one
 *   group per chat with **Go to chat** (that window comes forward with the chat in front) and,
 *   where its queue offers it, **Ignore**. It is answered there, where what it asks is shown.
 * - **A chat that cannot tell purlis it is waiting** (charter-app#52): a shell, or a harness
 *   without purlis's hooks, in any project of any window. Nothing has asked, so it is no ask:
 *   it is a Notice, with Go to chat, as the faint hand names it. The status line's count leaves
 *   it out (`FROM_THE_HAND`).
 */
export type Elsewhere = {
  /** The chats waiting in the projects of the window's other windows. */
  asking: readonly Needing[];
  /** Every chat that can be waiting without saying so, in every project of every window. */
  quiet: readonly Quiet[];
  /** Carries a row out in the project it is about, in whichever window holds it. */
  onPress: (plane: string, offer: Offer) => void;
};

/** The row that brings chat `session` of `plane` to the front: the queue's own Go, where it is
 *  offered, else the same verb. */
export function showOffer(session: number, name: string, go?: Offer): Offer {
  if (go?.available) return go;
  return {
    id: `needs.show:${session}`,
    title: `Show ${name}`,
    available: true,
    reason: "",
    does: { verb: "showChat", session },
    name,
  };
}

/** The key of a chat elsewhere, for its stops. */
export const elsewhereKey = (item: { plane: string; session: number }) =>
  `elsewhere:${item.plane}#${item.session}`;

/** The stops of the groups for chats elsewhere, in the order they are drawn. */
export function elsewhereStops(asking: readonly Needing[]): string[] {
  return asking.flatMap((item) => {
    const key = elsewhereKey(item);
    return [key, `${key}:go`, ...(item.ignore?.available ? [`${key}:ignore`] : [])];
  });
}

/** What a group for a chat elsewhere says it is. */
export const IN_ANOTHER_WINDOW = "In another window";

/** One chat waiting in another window's project, as an Inbox group. */
export function ElsewhereGroup({
  item,
  onPress,
}: {
  item: Needing;
  onPress: (plane: string, offer: Offer) => void;
}) {
  const id = useId();
  const key = elsewhereKey(item);
  // Every word here is what that window said: a chat's name, its reason, drawn as text.
  const says =
    item.needed ?? backSaid(item.reported ?? [], item.stoppedBelow) ?? item.why ?? "Waiting on you";
  const where = `${item.workspace} · ${item.project}`;
  return (
    <section
      className="inbox-chat inbox-elsewhere"
      aria-label={`${item.name} · ${where}`}
      // Not `data-session`: a notification lands on this project's own group by that
      // (`landOnGroup`), and a chat elsewhere can have the same number in its project.
      data-elsewhere={`${item.plane}#${item.session}`}
    >
      <h3 className="inbox-chain">
        {item.persona != null && <PersonaMark persona={item.persona} mark={item.mark} />}
        {item.name}
      </h3>
      <ul>
        <RovingFocusGroup.Item asChild tabStopId={key}>
          <li
            className="inbox-ask"
            data-source="elsewhere"
            aria-labelledby={`${id}-says`}
            aria-describedby={`${id}-whose`}
          >
            <p className="inbox-says">
              <span id={`${id}-whose`} className="inbox-source">
                {IN_ANOTHER_WINDOW}
              </span>
              <span id={`${id}-says`} className="ask-says">
                {says}
              </span>
              <span className="inbox-where">{where}</span>
            </p>
            <RovingFocusGroup.Item asChild tabStopId={`${key}:go`}>
              <button
                type="button"
                className="inbox-go"
                aria-label={`Go to chat ${item.name} · ${where}, in another window`}
                onClick={() => onPress(item.plane, showOffer(item.session, item.name, item.go))}
              >
                Go to chat
              </button>
            </RovingFocusGroup.Item>
            {item.ignore?.available && (
              <RovingFocusGroup.Item asChild tabStopId={`${key}:ignore`}>
                <button
                  type="button"
                  className="inbox-go"
                  aria-label={item.ignore.title}
                  title={item.ignore.title}
                  onClick={() => item.ignore && onPress(item.plane, item.ignore)}
                >
                  Ignore
                </button>
              </RovingFocusGroup.Item>
            )}
          </li>
        </RovingFocusGroup.Item>
      </ul>
    </section>
  );
}

/** Every chat that cannot tell purlis it is waiting, each a Notice with Go to chat. */
export function QuietNotices({
  plane,
  quiet,
  onPress,
}: {
  plane: string;
  quiet: readonly Quiet[];
  onPress: (plane: string, offer: Offer) => void;
}) {
  return (
    <>
      {quiet.map((one) => (
        <Notice
          key={`${one.plane}#${one.session}`}
          cause={`chat-quiet:${one.plane}#${one.session}`}
          link={{
            label: "Go to chat",
            onPress: () => onPress(one.plane, showOffer(one.session, one.name)),
          }}
        >
          <strong>{one.name}</strong>
          {one.plane === plane ? "" : ` in ${one.project}`} can&apos;t tell purlis it&apos;s
          waiting.
        </Notice>
      ))}
    </>
  );
}
