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
 * - **A chat no dispatch started** (#6304) has no task: its badge says Closed, it has no report
 *   line, and *Read it* is disabled, as its screen is never kept. *Reopen* starts it again on
 *   its conversation from the record kept as it closed, and is disabled with why where that
 *   record kept none (#6485). A handoff, or a task that ended owing no report, is drawn the
 *   same way from its chat's record, and *Read it* opens its screen where one was kept.
 * - Built from the families: `HeldPopover`, `Facts`, `Badge`, `ButtonRow` and `Button`.
 */
import type { RefObject } from "react";
import type { ChatCard } from "./bindings";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { Fact, Facts } from "./ui/card/Facts";
import { HeldPopover } from "./ui/popover/Popover";
import {
  NO_REPORT,
  READ_IT,
  REOPEN,
  SCREEN_NOT_KEPT,
  canRead,
  cannotReopen as whyNot,
  cardOutcome,
  cardTitle,
  cardWhen,
} from "./closedChat";

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
  const outcome = cardOutcome(card);
  // A chat no dispatch started has no task: no report, and no Reopen (#6304).
  const task = card.task;
  const cannotReopen = whyNot(card);
  const readable = canRead(card);
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
        <Fact label="Ran in">{card.place}</Fact>
        {task !== null && (
          <Fact label="Report">
            {task.report === "" ? (
              <span className="ui-text-secondary">{NO_REPORT}</span>
            ) : (
              <span className="closed-chat-card-report">{task.report}</span>
            )}
          </Fact>
        )}
      </Facts>
      <ButtonRow>
        <Button
          variant="primary"
          onPress={onRead}
          isDisabled={!readable}
          tooltip={readable ? undefined : SCREEN_NOT_KEPT}
        >
          {READ_IT}
        </Button>
        <Button onPress={onReopen} isDisabled={cannotReopen !== undefined} tooltip={cannotReopen}>
          {REOPEN}
        </Button>
      </ButtonRow>
      {!readable && <p className="closed-chat-card-note ui-text-secondary">{SCREEN_NOT_KEPT}</p>}
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
