// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext, type ReactNode } from "react";
import { PanelRight } from "lucide-react";
import type { Offer } from "./actions";
import type { ExtensionView, PlaneId } from "./bindings";
import { Groups } from "./Menus";
import type { GroupDrawn } from "./menuModel";
import { Notice } from "./Notice";
import { Button, IconButton } from "./ui/button/Button";
import { ContextMenu } from "./ui/menu/ContextMenu";
import { Menu } from "./ui/menu/Menu";
import type { ViewRef } from "./tabs";
import { dismissOffer, stopOffers, useOffers } from "./offerStore";
import { useBeside } from "./paneSize";
import { type Beside, besideLabel } from "./viewBeside";
import { offerKey, offerSaid, offerTitle, viewOf, type ViewOffer } from "./viewOffers";

/**
 * **How an extension's view is opened** (#4769; prototype `view-open-beside`, accepted on #4705):
 * a press opens it in a tab of its own, and Open beside opens it in a split beside a pane. Open
 * beside is the beside icon right after the view's button, ⌥ (Alt off a Mac) held on the press,
 * or the press's own menu, headed by the view's name. Where the split would leave a side under
 * the minimum, the icon and the menu row grey out with the reason as their label, before
 * anything is pressed (`viewBeside.ts`).
 */

/** How a press opens a view. */
export type OpenHow = "tab" | "beside";

/** The press's menu: Open in a tab ↵, and Open beside ⌥↵ or why it can't. */
function openMenu(view: ViewRef, name: string, beside: Beside): GroupDrawn[] {
  const tab: Offer = {
    id: "view.press:tab",
    title: "Open in a tab",
    available: true,
    reason: "",
    does: { verb: "openView", view, title: name },
  };
  const aside: Offer = {
    id: "view.press:beside",
    title: "Open beside",
    available: beside.ok,
    reason: beside.ok ? "" : beside.why,
    does: { verb: "openViewBeside", view, title: name },
  };
  return [
    {
      label: name,
      named: true,
      entries: [
        { kind: "row", offer: tab, said: tab.title, danger: false, keys: "↵" },
        { kind: "row", offer: aside, said: aside.title, danger: false, keys: "⌥↵" },
      ],
    },
  ];
}

/**
 * **A view's button, with its beside icon after it** (#4769 Q1 A), on a view's heading: Open
 * beside goes beside `pane`, the pane the button sits on.
 */
export function ViewOpenButton({
  view,
  name,
  icon,
  children,
  tooltip,
  pane,
  onOpen,
}: {
  view: ViewRef;
  /** The view's name as its tab says it: "Statistics · steward". */
  name: string;
  icon?: ReactNode;
  /** The button's words. */
  children: ReactNode;
  tooltip?: string;
  /** The pane the button sits on. */
  pane?: number;
  onOpen: (how: OpenHow) => void;
}) {
  const beside = useBeside(pane);
  return (
    <span className="view-open">
      <ContextMenu
        menu={() => (
          <Menu aria-label={name} className="item-menu">
            <Groups
              groups={openMenu(view, name, beside)}
              onPress={(offer) => onOpen(offer.does.verb === "openViewBeside" ? "beside" : "tab")}
            />
          </Menu>
        )}
      >
        <Button
          size="sm"
          tooltip={tooltip}
          icon={icon}
          // ⌥ held on the press opens it beside: the VS Code convention (#4699's addendum).
          onPress={(press) => onOpen(press.altKey ? "beside" : "tab")}
        >
          {children}
        </Button>
      </ContextMenu>
      <IconButton
        size="sm"
        variant="ghost"
        className="view-open-beside"
        aria-label={besideLabel(name, beside)}
        isDisabled={!beside.ok}
        icon={<PanelRight aria-hidden="true" />}
        onPress={() => onOpen("beside")}
      />
    </span>
  );
}

/**
 * **An extension's offer, as one quiet note** (#4769; #4584's rules): "statistics offers a view
 * · Statistics · steward", with Open, Open beside, Stop offers from statistics and Dismiss. On a
 * pane's band (`at="pane"`) it is a note: counted on the bell, folded into the count while the
 * band is minimised, never opening the band or taking the focus. In the Notices centre it is a
 * line of the list. It is a polite status, as every Notice is, so its coming is heard as the
 * count's change and nothing more.
 */
export function OfferNotice({
  offer,
  declared,
  at,
  pane,
  onOpen,
  onDismiss,
  onStop,
}: {
  offer: ViewOffer;
  declared: readonly ExtensionView[];
  at: "pane" | "inbox";
  /** The pane whose band it is on: Open beside goes beside it. Else the focused pane. */
  pane?: number;
  onOpen: (view: ViewRef, name: string, how: OpenHow) => void;
  onDismiss: () => void;
  onStop: () => void;
}) {
  const beside = useBeside(pane);
  const name = offerTitle(offer, declared);
  const view = viewOf(offer);
  return (
    <Notice
      cause={`view-offer:${offerKey(offer)}`}
      at={at}
      fixes={[
        { label: "Open", onPress: () => onOpen(view, name, "tab") },
        {
          label: "Open beside",
          onPress: () => onOpen(view, name, "beside"),
          off: beside.ok ? undefined : beside.why,
        },
        { label: `Stop offers from ${offer.extension}`, onPress: onStop },
      ]}
      onDismiss={onDismiss}
      dismissSays="Quiet until purlis restarts"
    >
      {offerSaid(offer, declared)}
    </Notice>
  );
}

/** **What an offer note opens with** (#4769), provided by the project's window: the views in
 *  force, for the note's words, and the window's open and Open beside. */
export type OfferOpening = {
  plane: PlaneId;
  declared: readonly ExtensionView[];
  onOpen: (view: ViewRef, name: string, how: OpenHow, pane?: number) => void;
};

export const OfferOpeningContext = createContext<OfferOpening | null>(null);

/**
 * **The offers about chat `session`, on its pane's band** (#4769): drawn wherever the chat's
 * notices are, which is its pane while it is on screen. Open beside goes beside `pane`. Every
 * offer is in the Notices centre too ({@link OfferNotices}).
 */
export function ChatOffers({ session, pane }: { session: number; pane?: number }) {
  const opening = useContext(OfferOpeningContext);
  const standing = useOffers(opening?.plane ?? "");
  if (opening === null) return null;
  return standing
    .filter((offer) => offer.chat === session)
    .map((offer) => (
      <OfferNotice
        key={offerKey(offer)}
        offer={offer}
        declared={opening.declared}
        at="pane"
        pane={pane}
        onOpen={(view, name, how) => opening.onOpen(view, name, how, pane)}
        onDismiss={() => dismissOffer(opening.plane, offer)}
        onStop={() => stopOffers(offer.extension)}
      />
    ));
}

/** **Every offer standing in the project, in the Notices centre** (#4769): Open beside goes
 *  beside the focused pane. */
export function OfferNotices() {
  const opening = useContext(OfferOpeningContext);
  const standing = useOffers(opening?.plane ?? "");
  if (opening === null) return null;
  return standing.map((offer) => (
    <OfferNotice
      key={offerKey(offer)}
      offer={offer}
      declared={opening.declared}
      at="inbox"
      onOpen={opening.onOpen}
      onDismiss={() => dismissOffer(opening.plane, offer)}
      onStop={() => stopOffers(offer.extension)}
    />
  ));
}
