// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A closed or finished chat's card** (#6214, parent #5820; the prototype's S6 on
 * `prototype/pane-links`, accepted 2026-11-11): "#641 · Signed commits #5607", its outcome as a
 * badge, when it ended, who started it, where it ran and its report cut at four lines, with
 * *Read it* and *Reopen*.
 *
 * - **A popover beside what it came from** (D7): the palette's finished row now, and a pane's
 *   `chat 641` link later (#6217). It is a glance with two ways on.
 * - ***Read it* is the primary answer**: the task's kept last screen, read-only, in a view tab.
 *   Where nothing was kept it is disabled and says "Its screen was not kept." (the person's
 *   ruling of 2026-11-11). ***Reopen* is plain**, because it starts a program: the chat's
 *   conversation started again as an ordinary chat. Where it cannot, it is disabled with why.
 * - Built from the families: `HeldPopover`, `Facts`, `Badge`, `ButtonRow` and `Button`.
 */
import type { RefObject } from "react";
import type { ChatCard } from "./bindings";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { Fact, Facts } from "./ui/card/Facts";
import { HeldPopover } from "./ui/popover/Popover";
import { NO_REPORT, SCREEN_NOT_KEPT, cardTitle, cardWhen, outcomeOf } from "./closedChat";
import "./closedChatCard.css";

/** Why *Reopen* does nothing on a card whose task names no conversation. */
export const NO_CONVERSATION =
  "It cannot be reopened: its harness named no conversation to resume.";

/** The card's own words and answers, inside whatever holds it. */
export function ClosedChatCard({
  card,
  titleId,
  onRead,
  onReopen,
}: {
  card: ChatCard;
  /** The title's id, which names the popover holding it. */
  titleId: string;
  onRead: () => void;
  onReopen: () => void;
}) {
  const outcome = outcomeOf(card.task.how);
  const cannotReopen = card.task.reopens
    ? (card.task.not_reopened ?? undefined)
    : (card.task.not_reopened ?? NO_CONVERSATION);
  return (
    <div className="closed-chat-card" data-testid="closed-chat-card">
      <div className="closed-chat-card-head">
        <h3 id={titleId} className="closed-chat-card-title">
          {cardTitle(card)}
        </h3>
        <Badge variant={outcome.variant}>{outcome.badge}</Badge>
      </div>
      <p className="closed-chat-card-when ui-text-secondary">{cardWhen(card)}</p>
      <Facts aria-label={`About chat #${card.number}`}>
        <Fact label="Started by">{card.started_by}</Fact>
        <Fact label="Ran in">{card.task.place}</Fact>
        <Fact label="Report">
          {card.task.report === "" ? (
            <span className="ui-text-secondary">{NO_REPORT}</span>
          ) : (
            <span className="closed-chat-card-report">{card.task.report}</span>
          )}
        </Fact>
      </Facts>
      <ButtonRow>
        <Button
          variant="primary"
          onPress={onRead}
          isDisabled={!card.screen_kept}
          tooltip={card.screen_kept ? undefined : SCREEN_NOT_KEPT}
        >
          Read it
        </Button>
        <Button onPress={onReopen} isDisabled={cannotReopen !== undefined} tooltip={cannotReopen}>
          Reopen
        </Button>
      </ButtonRow>
      {!card.screen_kept && (
        <p className="closed-chat-card-note ui-text-secondary">{SCREEN_NOT_KEPT}</p>
      )}
    </div>
  );
}

/** **The card in its popover**, beside `triggerRef`, held open by its owner. */
export function ClosedChatCardPopover({
  card,
  triggerRef,
  isOpen,
  onOpenChange,
  onRead,
  onReopen,
}: {
  card: ChatCard;
  triggerRef: RefObject<Element | null>;
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  onRead: () => void;
  onReopen: () => void;
}) {
  const titleId = `closed-chat-card-title-${card.number}`;
  return (
    <HeldPopover
      triggerRef={triggerRef}
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      aria-labelledby={titleId}
      side="bottom"
      className="closed-chat-card-popover"
    >
      <ClosedChatCard card={card} titleId={titleId} onRead={onRead} onReopen={onReopen} />
    </HeldPopover>
  );
}
