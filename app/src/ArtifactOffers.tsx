// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { listen, untilStopped } from "./here";
import type { Offer } from "./actions";
import { resolve, type PageOffer, type Resolved } from "./pageOffers";
import { Button } from "./ui/button/Button";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * **The actions an artifact's page offers, drawn in purlis's bar** (ADR 0122 §8, #5972).
 *
 * The page posts what it offers on its port; the core passes on only a set from the open load of
 * an approved page, in closed types (`artifacts/offers.rs`), and this draws each one the window's
 * own catalogue has (`pageOffers.ts`), in the catalogue's words. The person presses it here,
 * and the press runs the catalogue's row as the palette would (`onPress`). A delete asks first.
 * Nothing the page does presses one.
 */

/** What the window hears when an artifact's page changes what it offers. */
export const ARTIFACT_OFFERS = "artifact-offers";

/** What {@link ARTIFACT_OFFERS} carries. */
export type ArtifactOffersHeard = { label: string; offers: PageOffer[] };

/** **The offers of artifact webview `label`'s page**, pinned to `workspace`. */
export function ArtifactOffers({
  label,
  workspace,
  name,
  offerFor,
  onPress,
}: {
  label: string;
  workspace: string;
  /** The artifact's name, as the bar says it. */
  name: string;
  offerFor?: (id: string) => Offer | undefined;
  onPress?: (offer: Offer) => void;
}) {
  const offered = useOffered(label);
  if (offered.length === 0 || onPress === undefined) return null;
  return (
    <OffersDrawn
      drawn={offered.map((one) => resolve(one, workspace, offerFor))}
      name={name}
      onPress={onPress}
    />
  );
}

/** **The offers purlis drew for a page called `name`**, and the question a delete asks. */
export function OffersDrawn({
  drawn,
  name,
  onPress,
}: {
  drawn: readonly Resolved[];
  name: string;
  onPress: (offer: Offer) => void;
}) {
  const [asking, setAsking] = useState<{ offer: Offer; answer: string } | null>(null);
  return (
    <>
      <span role="group" aria-label={`Offered by ${name}`} className="artifact-bar-offers">
        {drawn.map((one) => (
          <OfferButton
            key={one.key}
            one={one}
            onPress={(offer, asks) =>
              asks === undefined ? onPress(offer) : setAsking({ offer, answer: asks })
            }
          />
        ))}
      </span>
      {asking === null ? null : (
        <OfferAsks
          offer={asking.offer}
          answer={asking.answer}
          name={name}
          onAnswer={() => {
            setAsking(null);
            onPress(asking.offer);
          }}
          onClose={() => setAsking(null)}
        />
      )}
    </>
  );
}

/** One offer, as a button in the catalogue's words, or the sentence that refused it. */
function OfferButton({
  one,
  onPress,
}: {
  one: Resolved;
  onPress: (offer: Offer, asks: string | undefined) => void;
}) {
  if ("refused" in one) {
    return (
      <span className="artifact-bar-standing" data-testid="artifact-offer-refused">
        {one.refused}
      </span>
    );
  }
  const { offer } = one;
  return (
    <Button
      size="sm"
      variant={one.asks === undefined ? undefined : "danger"}
      isDisabled={!offer.available}
      tooltip={offer.available ? offer.note : offer.reason}
      onPress={() => onPress(offer, one.asks)}
    >
      {one.asks === undefined ? offer.title : `${offer.title}…`}
    </Button>
  );
}

/** **The question a delete a page offered asks first**: only its answer does the row. */
export function OfferAsks({
  offer,
  answer,
  name,
  onAnswer,
  onClose,
}: {
  offer: Offer;
  answer: string;
  name: string;
  onAnswer: () => void;
  onClose: () => void;
}) {
  return (
    <Dialog
      title={`${offer.title}?`}
      description={`${name} offered this. ${offer.note ?? ""}`.trim()}
      main={{ children: answer, variant: "danger", onPress: onAnswer }}
      initialFocus="cancel"
      onClose={onClose}
      data-testid="artifact-offer-asks"
    />
  );
}

/** The offers artifact webview `label`'s page makes now, as its window last heard them. */
function useOffered(label: string): PageOffer[] {
  const [offered, setOffered] = useState<PageOffer[]>([]);
  useEffect(() => {
    return untilStopped(
      listen<ArtifactOffersHeard>(ARTIFACT_OFFERS, (event) => {
        if (event.payload.label === label) setOffered(event.payload.offers);
      }),
    );
  }, [label]);
  return offered;
}
