// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Plus } from "lucide-react";
import { useContext, type ComponentType, type ReactNode } from "react";
import { createPortal } from "react-dom";
import type { Offer } from "./actions";
import { HeadActions } from "./viewHead";
import { IconButton } from "./ui/button/Button";
import {
  PanelSection as Section,
  PanelSectionHead,
  PanelSectionTitle,
  PanelTitle,
} from "./ui/panel/Panel";

/**
 * One section of the Attention region: **its heading, and whatever it holds under it.**
 *
 * **One component, so every section is told apart the same way.** charter's own panels, an
 * extension's and the plane's vaults all draw through this, so a divider, a title's look or its
 * spacing cannot be given to charter's panels and not to a stranger's (ADR 0043: a contributed
 * panel is declared data, and drawing it is charter's job).
 *
 * **A title, not a row.** The heading keeps the panel's mark, because the mark is the one place
 * a panel's declared `mark` is drawn; its look (the panel family's section title, `ui/panel`) is
 * what makes it read as the start of a section rather than the first item in one.
 *
 * **In a region the heading is the region's** (#1738 P-11, #3330): one line, `RegionFrame`'s,
 * with the view's title and the control that puts the region away. This section's provenance
 * and actions go on that line ({@link HeadActions}) and it draws no heading of its own, so the
 * title is said once, in a half of the Panel and in a sidebar alike.
 *
 * **As a view** (`view`, #3340): the section is the panel family's view, one scroll with its top
 * held in place. In a region the top holds `top` (a box to add one, tabs, chips) and each list's
 * search field; drawn on its own (a story) its heading line is the top's first line too.
 */
export function PanelSection({
  testid,
  from,
  mark: Mark,
  title,
  provenance,
  actions,
  view = false,
  top,
  children,
}: {
  testid: string;
  /** Who contributed it, for `data-panel-from`; left off a section that is not a panel. */
  from?: string;
  mark: ComponentType<{ className?: string; "aria-hidden"?: boolean | "true" }>;
  title: string;
  /** Whose panel this is, beside the title, when it is not charter's own. */
  provenance?: string;
  /** Buttons on the heading, such as a view's. */
  actions?: ReactNode;
  /** Drawn as a view: one scroll, its fields held at the top (#3340). */
  view?: boolean;
  /** What a view holds in its top: a box to add one, tabs, chips. */
  top?: ReactNode;
  children: ReactNode;
}) {
  const head = useContext(HeadActions);
  if (head !== undefined) {
    return (
      <Section data-testid={testid} data-panel-from={from} view={view} top={view ? top : undefined}>
        {head !== null &&
          createPortal(
            <>
              {provenance !== undefined && <Provenance of={provenance} />}
              {actions}
            </>,
            head,
          )}
        {!view && top}
        {children}
      </Section>
    );
  }
  const heading = (
    <PanelSectionHead>
      {view ? (
        // A view's one title, as a region's head draws it (the prototype's Q1 A, #3340): 13px,
        // strong, in the primary tier and its own case.
        <PanelTitle>
          {title}
          {provenance !== undefined && <Provenance of={provenance} />}
        </PanelTitle>
      ) : (
        <PanelSectionTitle mark={Mark}>
          {title}
          {provenance !== undefined && <Provenance of={provenance} />}
        </PanelSectionTitle>
      )}
      {actions}
    </PanelSectionHead>
  );
  if (view)
    return (
      <Section
        data-testid={testid}
        data-panel-from={from}
        view
        top={
          <>
            {heading}
            {top}
          </>
        }
      >
        {children}
      </Section>
    );
  return (
    <Section data-testid={testid} data-panel-from={from}>
      {heading}
      {top}
      {children}
    </Section>
  );
}

/**
 * **Whose panel it is, beside its title: what is in force, after approval and not only at it**
 * (ADR 0041 item 5). An operator has to be able to tell a panel his own charter draws from one a
 * stranger's extension contributed, without opening a dialog to find out, so it goes onto the
 * region's head with the actions where that head is the title.
 */
function Provenance({ of }: { of: string }) {
  return <span className="panel-from ui-text-secondary">{` · ${of}`}</span>;
}

/**
 * A `+` on a section's heading: **the catalogue's row that makes one more of what the section
 * lists** — `vault.create` on the Vaults section, `persona.create` on the Personas panel (SI-3).
 *
 * The row is the catalogue's, so the palette and the heading cannot disagree about what it does
 * or whether it can run; a row the catalogue does not offer draws no button. Its mark alone, with
 * the row's title as its accessible name — the one exception `docs/design-system.md` makes for a
 * glyph without words, and the operator's own ask for a `+` here.
 */
export function HeadingOffer({
  offer,
  onPress,
  mark: Mark = Plus,
}: {
  offer?: Offer;
  onPress: (offer: Offer) => void;
  /** The glyph, when the row is not one that makes something: a store's archive (KN-4). */
  mark?: ComponentType<{ className?: string; "aria-hidden"?: boolean | "true" }>;
}) {
  if (!offer) return null;
  return (
    <IconButton
      size="sm"
      aria-label={offer.title}
      tooltip={offer.available ? (offer.note ?? offer.title) : offer.reason}
      isDisabled={!offer.available}
      onPress={() => onPress(offer)}
      icon={<Mark className="node-icon" aria-hidden="true" />}
    />
  );
}
