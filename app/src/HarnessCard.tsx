// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { HarnessGlance } from "./bindings";
import { Button } from "./ui/button/Button";

/**
 * **A harness's capability card, at a glance** (HP-19, W10, ADR 0072 §3): labelled *What
 * <product> can do here*, with one line for each thing it lacks, said where the operator meets
 * the harness — in the picker, under the row that is picked, and in a chat's header, where it
 * opens the whole card as a view tab (`tabs.harnessCardView`), drawn by the core in the panel
 * vocabulary.
 *
 * **Every word comes from the core** (`purlis_core::harness_card`), which reads it off the
 * harness's declaration and the adapter charter ships for it, in the first hour's words. Nothing
 * here knows a harness by name: one this window has never heard of is drawn the same way.
 */

/** What a card with nothing missing says in place of its lines. */
const NOTHING_LACKING = "Everything purlis asks of it.";

/** The card's lines as one run of sentences: a tooltip's words. */
export function glanceSaid(glance: HarnessGlance): string {
  return glance.lines.length === 0 ? NOTHING_LACKING : glance.lines.join(" ");
}

/** What the picker says of the harness of the row that is picked: the card, at a glance. */
export function HarnessSummary({ glance }: { glance: HarnessGlance }) {
  return (
    <section className="harness-glance" aria-label={glance.label} data-testid="harness-glance">
      <p className="what-here">{glance.label}</p>
      {glance.lines.length === 0 ? (
        <p className="came-back">{NOTHING_LACKING}</p>
      ) : (
        <ul className="came-back">
          {glance.lines.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      )}
    </section>
  );
}

/**
 * The harness in a chat's header, by its product's name, with its card one press away: a ghost
 * button of the family, small as the line's other chips are (#3129). Its accessible name is the
 * card's label; its tooltip is the card's lines, which a screen reader hears as the button's
 * description.
 */
export function HarnessChip({
  glance,
  onOpen,
}: {
  glance: HarnessGlance;
  /** Opens the harness's card tab. */
  onOpen: () => void;
}) {
  return (
    <Button
      variant="ghost"
      size="sm"
      className="pane-harness"
      tooltip={glanceSaid(glance)}
      aria-label={glance.label}
      onPress={onOpen}
    >
      {glance.title}
    </Button>
  );
}
