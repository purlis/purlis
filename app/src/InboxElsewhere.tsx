// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId } from "react";
import * as RovingFocusGroup from "@radix-ui/react-roving-focus";
import type { Offer } from "./actions";
import type { Shown } from "./bindings";
import { backSaid } from "./chatState";
import { capMarks } from "./combiningMarks";
import { KindMark, kindOfAsk, RowAge } from "./InboxRow";
import { Notice } from "./Notice";
import type { Needing, Quiet } from "./NeedsYou";
import { PersonaMark } from "./PersonaMark";
import { Button, ButtonRow } from "./ui/button/Button";
import { Card } from "./ui/card/Card";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { Tooltip } from "./ui/tooltip/Tooltip";

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
  /**
   * **The projects of this window with something waiting** (#1700, 2026-10-10 21:09 item 5),
   * each with how many asks its registry holds. Each Inbox names the others, one line each:
   * the Inbox is one project's, and the ✋ opens only one of them.
   */
  waitingHere?: readonly WaitingHere[];
  /** Go to project: that project in front, with its Inbox open. */
  onOpen?: (plane: string) => void;
};

/** A project of this window with something waiting, and how many asks. */
export type WaitingHere = {
  plane: string;
  project: string;
  count: number;
  /**
   * **What waits there, ask by ask** (qw359): what each says and who asks, as that project's
   * registry derived them, for the Inbox's All projects. Read here and answered there: no
   * answer of an ask is carried to another project's Inbox, so none is offered on it.
   */
  asks?: readonly WaitingAsk[];
};

/** One ask of another project of this window, as much of it as is read here: data, as text. */
export type WaitingAsk = Pick<Shown, "session" | "ask" | "says" | "source" | "chain" | "since">;

/** What an ask is cut down to before it is told to the other projects' Inboxes. */
export const waitingAsk = ({ session, ask, says, source, chain, since }: Shown): WaitingAsk => ({
  session,
  ask,
  says,
  source,
  chain,
  since,
});

/** What the lines for this window's other projects are called. */
export const OTHER_PROJECTS = "Other projects in this window";

/** The key of a line for another project of this window, for its stops. */
export const hereKey = (plane: string) => `here:${plane}`;

/** The projects other than `plane` with something waiting, as their lines draw them. */
export const othersWaiting = (plane: string, elsewhere: Elsewhere | undefined) =>
  (elsewhere?.waitingHere ?? []).filter((one) => one.plane !== plane && one.count > 0);

/** The stops of those lines, in the order they are drawn. */
export const hereStops = (others: readonly WaitingHere[]) =>
  others.flatMap((one) => [hereKey(one.plane), `${hereKey(one.plane)}:go`]);

/**
 * **One line for each other project of this window that has something waiting** (#1700): its
 * name, how many asks wait there, and Go to project. It answers nothing: what waits is read and
 * answered in that project's own Inbox, where it is drawn whole. Where the window says what
 * each of them is (qw359), they are listed under the line: what it says, who asks, its kind and
 * its age, as text.
 */
export function OtherProjects({
  others,
  onOpen,
  now,
}: {
  others: readonly WaitingHere[];
  onOpen: (plane: string) => void;
  /** The time an ask's age is said against. */
  now?: number;
}) {
  return (
    <section className="inbox-chat inbox-elsewhere" aria-label={OTHER_PROJECTS}>
      <PanelSectionTitle level={3} className="inbox-chain">
        {OTHER_PROJECTS}
      </PanelSectionTitle>
      <ul>
        {others.map((one) => (
          <RovingFocusGroup.Item asChild tabStopId={hereKey(one.plane)} key={one.plane}>
            <Card as="li" variant="ask" className="inbox-ask" data-source="here">
              <p className="inbox-says">
                <KindMark kind="elsewhere" />
                {/* A project's name is data: drawn as text. */}
                <span className="ask-says inbox-primary ui-text-body ui-text-strong">{`${one.project} · ${one.count} waiting on you`}</span>
              </p>
              {one.asks !== undefined && one.asks.length > 0 && (
                <ul className="inbox-there">
                  {one.asks.map((ask) => {
                    const says = capMarks(ask.says).text;
                    const who = ask.chain.join(" › ");
                    return (
                      <li key={`${ask.session}\u0000${ask.ask}`} className="inbox-says">
                        <KindMark kind={kindOfAsk(ask.source)} />
                        <Tooltip tip={says}>
                          <span className="ask-says inbox-primary ui-text-body ui-text-strong inbox-clamped">
                            {says}
                          </span>
                        </Tooltip>
                        {who !== "" && (
                          <Tooltip tip={who}>
                            <span className="inbox-where inbox-secondary ui-text-secondary">
                              {who}
                            </span>
                          </Tooltip>
                        )}
                        {typeof ask.since === "number" && now !== undefined && (
                          <span className="inbox-meta ui-text-meta">
                            <RowAge when={ask.since * 1000} now={now} since="Waiting since" />
                          </span>
                        )}
                      </li>
                    );
                  })}
                </ul>
              )}
              <RovingFocusGroup.Item asChild tabStopId={`${hereKey(one.plane)}:go`}>
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={`Go to project ${one.project}`}
                  onPress={() => onOpen(one.plane)}
                  className="inbox-go"
                >
                  Go to project
                </Button>
              </RovingFocusGroup.Item>
            </Card>
          </RovingFocusGroup.Item>
        ))}
      </ul>
    </section>
  );
}

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
          <Card
            as="li"
            variant="ask"
            className="inbox-ask"
            data-source="elsewhere"
            aria-labelledby={`${id}-says`}
            aria-describedby={`${id}-whose`}
          >
            <p className="inbox-says">
              <KindMark kind="elsewhere" />
              <Tooltip tip={says}>
                <span
                  id={`${id}-says`}
                  className="ask-says inbox-primary ui-text-body ui-text-strong inbox-clamped"
                >
                  {says}
                </span>
              </Tooltip>
              <Tooltip tip={where}>
                <span className="inbox-where inbox-secondary ui-text-secondary">{where}</span>
              </Tooltip>
              <span className="inbox-meta ui-text-meta">
                <span id={`${id}-whose`} className="inbox-source">
                  {IN_ANOTHER_WINDOW}
                </span>
              </span>
            </p>
            <ButtonRow size="sm" className="inbox-acts">
              <RovingFocusGroup.Item asChild tabStopId={`${key}:go`}>
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={`Go to chat ${item.name} · ${where}, in another window`}
                  onPress={() => onPress(item.plane, showOffer(item.session, item.name, item.go))}
                  className="inbox-go"
                >
                  Go to chat
                </Button>
              </RovingFocusGroup.Item>
              {item.ignore?.available && (
                <RovingFocusGroup.Item asChild tabStopId={`${key}:ignore`}>
                  <Button
                    size="sm"
                    variant="ghost"
                    aria-label={item.ignore.title}
                    tooltip={item.ignore.title}
                    onPress={() => item.ignore && onPress(item.plane, item.ignore)}
                    className="inbox-go"
                  >
                    Ignore
                  </Button>
                </RovingFocusGroup.Item>
              )}
            </ButtonRow>
          </Card>
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
