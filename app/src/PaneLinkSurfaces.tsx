// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { LinkCandidate, LinkFrom, LinkOpened, LinkSeen } from "./bindings";
import { onAMac } from "./mac";
import {
  UNASKED,
  openLink,
  type HoveredLink,
  type LinkCore,
  type LinkHands,
  type ScreenLink,
} from "./paneLinks";
import { PaneLinksList } from "./PaneLinksList";
import { CodeBlock } from "./ui/code/CodeBlock";
import { Dialog } from "./ui/dialog/Dialog";
import {
  AnchoredTooltip,
  TOOLTIP_CLOSE_DELAY,
  TOOLTIP_DELAY,
  TooltipDetail,
} from "./ui/tooltip/Tooltip";

/**
 * **What a pane draws for its links** (spec #6208, #6215; the prototype's S2): a hovered link's
 * tooltip, and the window's answer to one ⌘-clicked that did not simply open.
 *
 * - **The tooltip has two lines** (D2): the target in full, in the code face, then what ⌘-click
 *   does. It opens after the tooltip family's delay, stands beside the link, and stays while the
 *   pointer is on it (WCAG 1.4.13).
 * - **A refusal is a dialog that says why** (#6208: "a link that cannot open says why"), the
 *   same "Link not opened" an artifact's refused link shows (`ArtifactLinks.tsx`).
 * - **An ask shows the link's text and where it really goes**, and opens only on the person's
 *   answer, which is sent back as a confirmed open of that target. No v1 web address asks; the
 *   file and OSC 8 kinds do (#6216, #6218), and draw their own asks there.
 */

/** The modifier word a link opens with, in the window's words (D12). */
export const modifierSaid = (mac: boolean) => (mac ? "⌘-click" : "Ctrl-click");

/** **What ⌘-click on a link does**, the tooltip's second line. */
export function linkDoesSaid(seen: LinkSeen, mac: boolean): string {
  const click = modifierSaid(mac);
  switch (seen.does.does) {
    case "opens":
      return `${click} opens it in your browser`;
    case "asks":
      return `${click} asks first: ${seen.does.reason}`;
    case "refused":
      return `${sentence(seen.does.reason)}.`;
  }
}

/** **A hovered link's tooltip**: its target in full, then what ⌘-click does. */
export function LinkTip({ seen, mac }: { seen: LinkSeen; mac: boolean }) {
  return (
    <span className="pane-link-tip">
      <code className="pane-link-target">{seen.target}</code>
      <TooltipDetail>{linkDoesSaid(seen, mac)}</TooltipDetail>
    </span>
  );
}

/** What the window answers for a link ⌘-clicked, while it is up. */
type Answering = { answer: LinkOpened; candidate: LinkCandidate };

/**
 * **A pane's link surfaces, and the hands its link hook is handed** (`paneLinks.ts`): `core`
 * decides its links, found `from` where it is, drawn in `colour`. With no core there are no
 * hands, and so no links. The hands are made once per core: `from` and `colour` are read as
 * they are when asked.
 *
 * **`openLinksList` opens *Links in this pane…*** (#6220, `PaneLinksList.tsx`): the links
 * `onScreen` reads, newest first. Its key (`pane.links`) and the terminal menu's row call it.
 */
export function usePaneLinks({
  from,
  core,
  colour,
  onScreen,
}: {
  from: LinkFrom;
  core: LinkCore | undefined;
  colour: () => string;
  /** The links on the pane's screen, newest first (`LinkedPane.onScreen`). */
  onScreen?: () => Promise<ScreenLink[]>;
}): {
  hands: LinkHands | undefined;
  surfaces: ReactNode;
  /** Opens *Links in this pane…*. Stable for the pane's life. */
  openLinksList: () => void;
} {
  const mac = onAMac();
  const now = useRef({ from, colour, onScreen });
  useEffect(() => {
    now.current = { from, colour, onScreen };
  });
  /** The list while it is up: the links read, or `undefined` while they are read. */
  const [listing, setListing] = useState<{ links: ScreenLink[] | undefined } | null>(null);
  /** Which opening of the list a read answers, so one closed before it is read stays closed. */
  const listed = useRef(0);
  const openLinksList = useCallback(() => {
    const read = now.current.onScreen;
    listed.current += 1;
    const asked = listed.current;
    setListing({ links: read === undefined ? [] : undefined });
    if (read === undefined) return;
    read().then(
      (links) => listed.current === asked && setListing({ links }),
      (why: unknown) => {
        console.warn("purlis could not read the links on screen", why);
        if (listed.current === asked) setListing({ links: [] });
      },
    );
  }, []);
  const closeLinksList = useCallback(() => {
    listed.current += 1;
    setListing(null);
  }, []);
  const [shown, setShown] = useState<HoveredLink | null>(null);
  const [answering, setAnswering] = useState<Answering | null>(null);
  const pending = useRef<HoveredLink | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const onTip = useRef(false);
  const later = (then: () => void, wait: number) => {
    clearTimeout(timer.current);
    timer.current = setTimeout(then, wait);
  };
  useEffect(() => () => clearTimeout(timer.current), []);

  const hovered = useCallback((link: HoveredLink | null) => {
    pending.current = link;
    if (link !== null) later(() => setShown(pending.current), TOOLTIP_DELAY);
    // Kept a moment, so the pointer can reach it (WCAG 1.4.13).
    else later(() => !onTip.current && setShown(null), TOOLTIP_CLOSE_DELAY);
  }, []);
  const answered = useCallback((answer: LinkOpened, candidate: LinkCandidate) => {
    if (answer.answer === "opened") return;
    setAnswering({ answer, candidate });
  }, []);
  const hands = useMemo(
    () =>
      core === undefined
        ? undefined
        : {
            core,
            from: () => now.current.from,
            hovered,
            answered,
            colour: () => now.current.colour(),
            mac: onAMac,
          },
    [core, hovered, answered],
  );

  const anchor = useRef<HTMLSpanElement>(null);
  const surfaces = (
    <>
      {shown !== null && (
        <>
          <span
            ref={anchor}
            className="pane-link-anchor"
            aria-hidden="true"
            style={{
              left: shown.place.left,
              top: shown.place.top,
              width: shown.place.width,
              height: shown.place.height,
            }}
          />
          <AnchoredTooltip
            anchor={anchor}
            onPointerEnter={() => {
              onTip.current = true;
              clearTimeout(timer.current);
            }}
            onPointerLeave={() => {
              onTip.current = false;
              hovered(pending.current);
            }}
          >
            <LinkTip seen={shown.seen} mac={mac} />
          </AnchoredTooltip>
        </>
      )}
      {listing !== null && (
        <PaneLinksList
          links={listing.links}
          onClose={closeLinksList}
          onPick={(link) => {
            closeLinksList();
            if (hands !== undefined) void openLink(link.candidate, hands);
          }}
          onCopy={(link) => {
            void navigator.clipboard?.writeText(link.seen.target).catch((why: unknown) => {
              console.warn("purlis could not copy a link", why);
            });
          }}
        />
      )}
      {answering !== null && (
        <LinkAnswer
          answering={answering}
          onClose={() => setAnswering(null)}
          onConfirm={(target) => {
            setAnswering(null);
            if (hands !== undefined) void openLink(answering.candidate, hands, target);
          }}
        />
      )}
    </>
  );
  return { hands, surfaces, openLinksList };
}

/** **The window's answer to a link that did not open**: why not, or the question first. */
export function LinkAnswer({
  answering: { answer, candidate },
  onClose,
  onConfirm,
}: {
  answering: Answering;
  onClose: () => void;
  onConfirm: (target: string) => void;
}) {
  if (answer.answer === "ask")
    return (
      <Dialog
        title="Open this link?"
        description={`${sentence(answer.reason)}.`}
        main={{ children: "Open link", onPress: () => onConfirm(answer.target) }}
        initialFocus="cancel"
        onClose={onClose}
        data-testid="pane-link-asks"
      >
        <CodeBlock label="Its text" text={answer.shown} />
        <CodeBlock label="Where it goes" text={answer.target} />
      </Dialog>
    );
  const reason = answer.answer === "refused" ? answer.reason : UNASKED;
  return (
    <Dialog
      title="Link not opened"
      description={`${sentence(reason)}.`}
      cancel={{ children: "Close" }}
      initialFocus="cancel"
      onClose={onClose}
      data-testid="pane-link-refused"
    >
      <CodeBlock label="The link" text={candidate.text} />
    </Dialog>
  );
}

/**
 * A sentence the core said, as the window starts one: its first letter capital, but purlis's
 * name, which is lowercase wherever it stands (`docs/ui-copy.md`).
 */
export function sentence(said: string): string {
  if (/^purlis\b/u.test(said)) return said;
  return said.charAt(0).toUpperCase() + said.slice(1);
}
