// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands, type BrowserSitesChanged, type PlaneId, type SandboxState } from "./bindings";
import { Notice } from "./Notice";
import { sandboxCommandReturned } from "./sandboxAsked";

/**
 * **The project's browser sites changed** (#2443, ruled 2026-10-17): the one-time Notice each
 * teammate sees when the project's settings let every chat's browser reach a site, for everyone
 * in the project. Unlike the project's hosts, a site added here reaches no browser on this
 * machine until it was shown: "Got it" records each site the Notice named as added
 * (`acknowledge_project_browser_sites`), and from then on every chat's browser here reaches it
 * from its next act. A site is the browsers' alone: chats' commands still ask for that host.
 * "Review the browser sites" opens the Sandbox group, where Granted lists each one. A site this
 * window's own Allow wrote is recorded as seen there, so it is never told back to its writer.
 *
 * It is read with the project's hosts, in one read of the project's sandbox state
 * (`ProjectHostsNotice`), so a project switch asks the core nothing more for it.
 */
export function ProjectBrowserSitesNotice({
  plane,
  changed,
  onRead,
  onReview,
}: {
  plane: PlaneId;
  /** How the project's browser sites changed, as the project's sandbox state says it. */
  changed: BrowserSitesChanged;
  /** The project's sandbox state once the Notice was read. */
  onRead: (state: SandboxState) => void;
  /** Opens Settings at the project's Sandbox group. */
  onReview: () => void;
}) {
  const read = () =>
    void commands
      .acknowledgeProjectBrowserSites(plane, changed.added)
      .then((said) => {
        if (said.status !== "ok") return;
        onRead(said.data);
        // What reads a chat's reach (its tab's mark, Granted) reads it again.
        sandboxCommandReturned();
      })
      // Not recorded: it is told again next time, and the sites reach no browser meanwhile.
      .catch(() => {});

  return (
    <Notice
      cause="sandbox-browser-sites"
      label="The project's browser sites changed"
      fixes={[
        { label: "Got it", onPress: read },
        { label: "Review the browser sites", onPress: onReview },
      ]}
    >
      <p>
        {changed.added.length > 0 && (
          <>
            The project's settings let every chat's browser reach {changed.added.join(", ")}, for
            everyone in the project. Your browsers reach{" "}
            {changed.added.length === 1 ? "it" : "them"} once you press Got it; chats' commands
            still ask.
          </>
        )}
        {changed.removed.length > 0 && (
          <> No longer for browsers here: {changed.removed.join(", ")}.</>
        )}
      </p>
    </Notice>
  );
}
