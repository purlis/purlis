// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { LinkCandidate, LinkFrom, LinkOpened, LinkSeen } from "./bindings";
import { onAMac } from "./mac";
import { UNASKED, openLink, type HoveredLink, type LinkCore, type LinkHands } from "./paneLinks";
import { openFileLink } from "./paneLinkFiles";
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

/** **What ⌘-click on a link does**, the tooltip's second line. A file says where it is first
 *  (D2): "Outside smart-ide · ⌘-click asks first". */
export function linkDoesSaid(seen: LinkSeen, mac: boolean): string {
  const click = modifierSaid(mac);
  const file = seen.kind === "file";
  switch (seen.does.does) {
    case "opens":
      return file ? `${click} opens it in an editor tab` : `${click} opens it in your browser`;
    case "asks":
      return file
        ? `${sentence(seen.does.reason)} · ${click} asks first`
        : `${click} asks first: ${seen.does.reason}`;
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
 */
export function usePaneLinks({
  from,
  core,
  colour,
}: {
  from: LinkFrom;
  core: LinkCore | undefined;
  colour: () => string;
}): {
  hands: LinkHands | undefined;
  surfaces: ReactNode;
} {
  const mac = onAMac();
  const now = useRef({ from, colour });
  useEffect(() => {
    now.current = { from, colour };
  });
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
      const plane = now.current.from.plane;
      if (plane !== null) return openFileLink(plane, answer);
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
  return { hands, surfaces };
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
        within={answer.about.within}
        folder={answer.about.folder}
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
 * `within` is the workspace's name, or "the project" for a chat at its root.
 */
export function FileOutsideAsk({
  target,
  within,
  folder,
  onClose,
  onOpen,
}: {
  target: string;
  within: string;
  folder: string;
  onClose: () => void;
  onOpen: () => void;
}) {
  const workspace = within !== "the project";
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
        <Fact label={workspace ? "Workspace" : "Project"}>
          <span className="pane-link-path">{folder}</span>
        </Fact>
      </Facts>
      <p className="ui-text-secondary">It opens in an editor tab. Nothing is run.</p>
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
