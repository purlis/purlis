// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { listen, untilStopped } from "./here";
import type { Offer } from "./actions";
import { placeOffer, type Goes } from "./artifactPlaces";
import { commands } from "./bindings";
import { CodeBlock } from "./ui/code/CodeBlock";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * **A link followed in an artifact's page** (ADR 0122 §5, #5956).
 *
 * The page never goes there itself: purlis's own script in it reports a person's click on a
 * link, and the core (`artifact_links.rs`) refuses it, opens it (a host the person chose to
 * always open), or asks here. The question shows the host and the whole address, and offers
 * Open and Always open; what opens is what the page reported, held by the core, never an
 * address this window names. A refused link says why.
 *
 * A `purlis://` link (#5967) asks nothing: the core has read it into a place in purlis and
 * checked it is there, and the window goes to it by its own row ({@link placeOffer}), which only
 * navigates.
 */

/** What the window hears when a person follows a link in an artifact's page. */
export const ARTIFACT_LINK = "artifact-link";

/** The link to ask about (its whole address and host), the place in purlis to go to, or the
 *  sentence that refused it. */
export type ArtifactLinkFollowed = {
  label: string;
  url: string | null;
  host: string | null;
  refused: string | null;
  goes?: Goes | null;
};

/**
 * **The link question for artifact webview `label`'s page**: nothing until it follows one.
 * `onPress` runs the row a link into purlis goes by, as the palette runs one.
 */
export function ArtifactLinks({
  label,
  onPress,
}: {
  label: string;
  onPress?: (offer: Offer) => void;
}) {
  const [link, setLink] = useState<ArtifactLinkFollowed | null>(null);
  const press = useRef(onPress);
  useEffect(() => {
    press.current = onPress;
  }, [onPress]);
  useEffect(() => {
    return untilStopped(
      listen<ArtifactLinkFollowed>(ARTIFACT_LINK, (event) => {
        if (event.payload.label !== label) return;
        const goes = event.payload.goes;
        if (goes !== null && goes !== undefined) press.current?.(placeOffer(goes));
        else setLink(event.payload);
      }),
    );
  }, [label]);
  if (link === null) return null;
  // Keyed by the link, so a second one followed starts afresh.
  return (
    <ArtifactLinkDialog
      key={`${link.url ?? ""}\u0000${link.refused ?? ""}`}
      link={link}
      onClose={() => setLink(null)}
    />
  );
}

/** **The question about one link**, or the sentence that refused it. */
export function ArtifactLinkDialog({
  link,
  onClose,
}: {
  link: ArtifactLinkFollowed;
  onClose: () => void;
}) {
  const [failed, setFailed] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  if (link.url === null || link.host === null) {
    return (
      <Dialog
        title="Link not opened"
        description={`${sentence(link.refused ?? "purlis did not open that link")}.`}
        cancel={{ children: "Close" }}
        initialFocus="cancel"
        onClose={onClose}
        data-testid="artifact-link-refused"
      />
    );
  }
  const host = link.host;
  const open = (always: boolean) => {
    setBusy(true);
    setFailed(null);
    commands.artifactLinkOpen(link.label, always).then(
      (answer) => {
        setBusy(false);
        if (answer.status === "error") setFailed(answer.error);
        else onClose();
      },
      (why: unknown) => {
        setBusy(false);
        setFailed(String(why));
      },
    );
  };
  return (
    <Dialog
      title={`Open a link to ${host}?`}
      description={`This artifact links to a page on ${host}, outside purlis. It opens in your browser.`}
      main={{ children: "Open", isDisabled: busy, onPress: () => open(false) }}
      alternative={{
        children: `Always open ${host}`,
        variant: "grants",
        isDisabled: busy,
        onPress: () => open(true),
      }}
      initialFocus="cancel"
      onClose={onClose}
      data-testid="artifact-link-asks"
    >
      <CodeBlock label="Whole address" text={link.url} />
      {failed !== null && (
        <p role="alert" className="artifact-refused">
          {failed}
        </p>
      )}
    </Dialog>
  );
}

/** A sentence the core said, as the window starts one: its first letter capital. */
function sentence(said: string): string {
  return said.charAt(0).toUpperCase() + said.slice(1);
}
