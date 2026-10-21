// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { harnessCardId, type Catalogued, type Offer } from "./actions";
import type { HarnessGlance } from "./bindings";
import { Button } from "./ui/button/Button";

/**
 * **The card's label as a link, at the end of an off control's reason** (#1134, ADR 0072 §3: "a
 * disabled control's tooltip is one such line, followed by the card's label as a link").
 *
 * The core writes the reason whole, the card's line and then *See What opencode can do here.*
 * (`harness_card::Card::lacks`), so the window never builds that sentence: it finds the label
 * of a card it already has at the reason's end and draws that label as a button that opens the
 * card's tab. Nothing here knows a harness or the label's wording, so a harness the project
 * declares is linked the same way as a built-in one.
 */

/** Where `reason` ends with the label of one of `cards`: the words before the label, and that
 *  card. Nothing where the reason names no card, which is then drawn as it was said. */
export function cardEnding<Card extends { label: string }>(
  reason: string,
  cards: Iterable<Card>,
): { before: string; card: Card } | undefined {
  const said = reason.trimEnd();
  for (const card of cards) {
    const tail = `${card.label}.`;
    if (card.label !== "" && said.endsWith(tail))
      return { before: said.slice(0, said.length - tail.length), card };
  }
  return undefined;
}

/**
 * **The catalogue's card rows that the off rows' reasons end with**, once each and in the order
 * the rows name them: what a menu draws under a disabled row, since a menu row that cannot run
 * cannot hold a link the keyboard reaches (D-1134-1). A card row is the catalogue's own
 * (`harnessCardId`), so pressing it does what the palette's *What opencode can do here* does.
 */
export function cardRowsFor(rows: readonly Offer[], offers: Catalogued): Offer[] {
  const prefix = harnessCardId("");
  const cards: Offer[] = [];
  for (const [id, offer] of offers) if (id.startsWith(prefix)) cards.push(offer);
  const found: Offer[] = [];
  if (cards.length === 0) return found;
  const labelled = cards.map((offer) => ({ label: offer.title, offer }));
  for (const row of rows) {
    if (row.available || !row.reason) continue;
    const ending = cardEnding(row.reason, labelled);
    if (ending !== undefined && !found.includes(ending.card.offer)) found.push(ending.card.offer);
  }
  return found;
}

/**
 * An off control's reason, drawn with the card's label as a link that opens its tab, or as it
 * was said where it names no card in `cards`.
 */
export function ReasonWithCard<Card extends { label: string }>({
  reason,
  cards,
  onOpen,
}: {
  reason: string;
  cards: Iterable<Card>;
  /** Opens the card's tab. */
  onOpen: (card: Card) => void;
}) {
  const ending = cardEnding(reason, cards);
  if (ending === undefined) return <>{reason}</>;
  return (
    <>
      {ending.before}
      <Button variant="link" onPress={() => onOpen(ending.card)} className="said-link">
        {ending.card.label}
      </Button>
      .
    </>
  );
}

/**
 * **The card's line, said once, where a chat is about to start on a harness that lacks
 * something** (#1134, DECISIONS W10): the first chat on that harness on this machine.
 *
 * "Once" is kept per harness in this window's web storage (D-1134-3): a per-viewer convenience,
 * since lost or unreadable storage costs only the note said again, and a record in a file purlis
 * keeps would be a format key nothing decided. It is kept when a chat **starts** on the harness
 * ({@link noteHarness}), not when the note is drawn: a picker cancelled has relied on nothing.
 */
const NOTED_KEY = "purlis.harness-noted";

/** The harnesses whose note this machine has said, by name; none where storage cannot say. */
function noted(): string[] {
  try {
    const held: unknown = JSON.parse(globalThis.localStorage?.getItem(NOTED_KEY) ?? "[]");
    return Array.isArray(held) ? held.filter((one): one is string => typeof one === "string") : [];
  } catch {
    return [];
  }
}

/** Keeps that a chat started on `harness`, so its note is not said again on this machine. */
export function noteHarness(harness: string): void {
  const was = noted();
  if (was.includes(harness)) return;
  try {
    globalThis.localStorage?.setItem(NOTED_KEY, JSON.stringify([...was, harness]));
  } catch {
    // Not kept: the note is said again next time, which is all lost storage costs.
  }
}

/** The note for `glance`, or nothing where it lacks nothing or this machine has said it. Read
 *  as it is drawn, so a surface with two starts (the First task's two chats) stops saying it
 *  once the first has started on that harness. */
export function FirstOnHarness({ glance }: { glance: HarnessGlance | null | undefined }) {
  if (!glance || glance.lines.length === 0 || noted().includes(glance.name)) return null;
  return (
    <p className="honest" role="note">
      {`You have not started a chat on ${glance.title} on this machine before. ${glance.lines.join(" ")}`}
    </p>
  );
}
