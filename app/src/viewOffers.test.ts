// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { ExtensionView } from "./bindings";
import type { ViewRef } from "./tabs";
import {
  dismissed,
  NO_OFFERS,
  type OfferWorld,
  offerSaid,
  offered,
  opened,
  stopped,
  type ViewOffer,
  withdrawn,
} from "./viewOffers";

/**
 * **An extension can only offer its view** (#4769; #4703 rule 2; prototype `view-open-beside`
 * Q3 A and Q4 A): one quiet note per view and subject, never a pane, a tab or the focus. A
 * dismissed offer stays quiet until relaunch, Stop offers holds every offer from that extension
 * until it is turned back on, opening the view by any route clears its offer, and a removed
 * extension's offers are withdrawn.
 */

const STATS: ExtensionView = {
  extension: "statistics",
  id: "stats",
  title: "Statistics",
  about: "personas",
};

const about = (key: string, chat?: number): ViewOffer => ({
  extension: "statistics",
  view: "stats",
  key,
  ...(chat === undefined ? {} : { chat }),
});

function world(change: Partial<OfferWorld> = {}): OfferWorld {
  return { declared: [STATS], off: [], open: () => false, ...change };
}

describe("an extension's offer (#4769)", () => {
  it("stands as one note", () => {
    const state = offered(NO_OFFERS, about("steward", 4), world());
    expect(state.standing).toEqual([about("steward", 4)]);
  });

  it("adds nothing when the same view and subject is offered again while it stands", () => {
    const once = offered(NO_OFFERS, about("steward", 4), world());
    expect(offered(once, about("steward", 7), world())).toBe(once);
  });

  it("adds nothing while the view it offers is open", () => {
    const open = (ref: ViewRef) => ref.from === "statistics" && ref.key === "steward";
    expect(offered(NO_OFFERS, about("steward"), world({ open }))).toBe(NO_OFFERS);
  });

  it("is about one subject: another persona's is another note", () => {
    const both = offered(offered(NO_OFFERS, about("steward"), world()), about("scout"), world());
    expect(both.standing).toEqual([about("steward"), about("scout")]);
  });

  it("is refused for a view the extension does not offer this window", () => {
    const other = { ...about("steward"), view: "secrets" };
    expect(offered(NO_OFFERS, other, world())).toBe(NO_OFFERS);
    expect(offered(NO_OFFERS, about("steward"), world({ declared: [] }))).toBe(NO_OFFERS);
  });

  it("stays quiet for that view and subject once dismissed, and only for those", () => {
    const standing = offered(NO_OFFERS, about("steward"), world());
    const gone = dismissed(standing, about("steward"));
    expect(gone.standing).toEqual([]);
    expect(offered(gone, about("steward"), world())).toBe(gone);
    expect(offered(gone, about("scout"), world()).standing).toEqual([about("scout")]);
  });

  it("is heard again after a relaunch, which starts with no quiet ones", () => {
    const gone = dismissed(offered(NO_OFFERS, about("steward"), world()), about("steward"));
    expect(gone.quiet.size).toBe(1);
    expect(NO_OFFERS.quiet.size).toBe(0);
    expect(offered(NO_OFFERS, about("steward"), world()).standing).toHaveLength(1);
  });

  it("is held while the person has stopped that extension's offers", () => {
    expect(offered(NO_OFFERS, about("steward"), world({ off: ["statistics"] }))).toBe(NO_OFFERS);
  });

  it("goes, with every other from that extension, when its offers are stopped", () => {
    const two = offered(offered(NO_OFFERS, about("steward"), world()), about("scout"), world());
    expect(stopped(two, "statistics").standing).toEqual([]);
  });

  it("goes once its view is opened by any route", () => {
    const standing = offered(NO_OFFERS, about("steward"), world());
    const ref: ViewRef = { from: "statistics", view: "stats", key: "steward" };
    expect(opened(standing, ref).standing).toEqual([]);
    // Opening it is no dismissal: it may be offered again once the view has closed.
    expect(opened(standing, ref).quiet.size).toBe(0);
  });

  it("is withdrawn without a word when its extension no longer offers the view", () => {
    const standing = offered(NO_OFFERS, about("steward"), world());
    expect(withdrawn(standing, []).standing).toEqual([]);
    expect(withdrawn(standing, [STATS])).toBe(standing);
  });
});

describe("what an offer says (#4769)", () => {
  it("says who offers which view, about what", () => {
    expect(offerSaid(about("steward"), [STATS])).toBe(
      "statistics offers a view · Statistics · steward",
    );
    expect(offerSaid(about(""), [STATS])).toBe("statistics offers a view · Statistics");
  });
});
