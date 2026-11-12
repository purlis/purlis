// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type {
  LinkCandidate,
  LinkFrom,
  LinkHome,
  LinkOpened,
  LinkSeen,
  OpensIn,
} from "./bindings";
import { sentence } from "./coreSentence";
import { onAMac } from "./mac";
import {
  UNASKED,
  openLink,
  type HoveredLink,
  type LinkCore,
  type LinkHands,
  type ScreenLink,
} from "./paneLinks";
import { openFileLink } from "./paneLinkFiles";
import { PaneLinksList } from "./PaneLinksList";
import { Fact, Facts } from "./ui/card/Facts";
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

/** Where a link that opens without asking opens, as the tooltip says it. */
const OPENS_IN: Record<OpensIn, string> = {
  browser: "in your browser",
  "editor-tab": "in an editor tab",
};

/** **What ⌘-click on a link does**, the tooltip's second line. A file says where it is first
 *  (D2): "Outside smart-ide · ⌘-click asks first". */
export function linkDoesSaid(seen: LinkSeen, mac: boolean): string {
  const click = modifierSaid(mac);
  switch (seen.does.does) {
    case "opens":
      return `${click} opens it ${OPENS_IN[seen.does.into]}`;
    case "asks":
      return `${sentence(seen.does.reason)} · ${click} asks first`;
    case "refused":
      return `${sentence(seen.does.reason)}.`;
  }
}

/** **A hovered link's tooltip**: where it goes (a file in the chat's workspace from the
 *  workspace, D2; everything else in full), then what ⌘-click does. */
export function LinkTip({ seen, mac }: { seen: LinkSeen; mac: boolean }) {
  return (
    <span className="pane-link-tip">
      <code className="pane-link-target">{seen.shown}</code>
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
 * **`list` opens *Links in this pane…*** (#6220, `PaneLinksList.tsx`): the links `onScreen`
 * reads, newest first. Its key (`pane.links`) and the terminal menu's row (#6219) call it. A
 * pane with no core has no links, and so no list (`undefined`).
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
  /** **Opens the pane's *Links in this pane…* list** (#6220), where the pane has one. */
  list: (() => void) | undefined;
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
    // A file opens in an editor tab of the pane's project (#6216); with none, it cannot.
    if (answer.answer === "file") {
      const { plane, session } = now.current.from;
      if (plane !== null) return openFileLink(plane, session, answer);
      setAnswering({ answer: { answer: "refused", reason: UNASKED }, candidate });
      return;
    }
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
          onConfirm={(token) => {
            setAnswering(null);
            if (hands !== undefined) void openLink(answering.candidate, hands, token);
          }}
        />
      )}
    </>
  );
  return { hands, surfaces, list: core === undefined ? undefined : openLinksList };
}

/** **The window's answer to a link that did not open**: why not, or the question first. An
 *  accepted ask is answered with the token the core issued with it (#6216). */
export function LinkAnswer({
  answering: { answer, candidate },
  onClose,
  onConfirm,
}: {
  answering: Answering;
  onClose: () => void;
  onConfirm: (token: string) => void;
}) {
  if (answer.answer === "ask" && answer.about.about === "file-outside")
    return (
      <FileOutsideAsk
        target={answer.target}
        home={answer.about.home}
        onClose={onClose}
        onOpen={() => onConfirm(answer.token)}
      />
    );
  if (answer.answer === "ask")
    return (
      <Dialog
        title="Open this link?"
        description={`${sentence(answer.reason)}.`}
        main={{ children: "Open link", onPress: () => onConfirm(answer.token) }}
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
 * **The ask for a file outside the workspace** (#6216, the prototype's S7, D9): the file and the
 * workspace, and *Don't open*, *Copy path* and *Open file*. It starts on *Open file*, and Return
 * opens it: opening only shows it in an editor tab, so the ask is a notice more than a guard.
 * That starting place is the person's ruling on the prototype (D9, accepted on #6211), over
 * the family's rule that an ask starts on its safe answer. `home` is the chat's workspace, or
 * the project for a chat at its root; with none, its row is left out.
 */
export function FileOutsideAsk({
  target,
  home,
  onClose,
  onOpen,
}: {
  target: string;
  home: LinkHome | null;
  onClose: () => void;
  onOpen: () => void;
}) {
  const workspace = home?.is === "workspace";
  const within = home?.is === "workspace" ? home.name : "the project";
  return (
    <Dialog
      title={workspace ? "Open a file outside this workspace?" : "Open a file outside the project?"}
      description={`This chat's output names a file that is not in ${within}.`}
      cancel={{ children: "Don't open" }}
      alternative={{
        children: "Copy path",
        onPress: () => {
          void navigator.clipboard?.writeText(target).catch(() => undefined);
          onClose();
        },
      }}
      main={{ children: "Open file", onPress: onOpen }}
      initialFocus="main"
      onClose={onClose}
      data-testid="pane-link-file-outside"
    >
      <Facts aria-label="The file">
        <Fact label="File">
          <span className="pane-link-path">{target}</span>
        </Fact>
        {home !== null && (
          <Fact label={workspace ? "Workspace" : "Project"}>
            <span className="pane-link-path">{home.folder}</span>
          </Fact>
        )}
      </Facts>
      <p className="ui-text-secondary">It opens in an editor tab. Nothing is run.</p>
    </Dialog>
  );
}

export { sentence };
