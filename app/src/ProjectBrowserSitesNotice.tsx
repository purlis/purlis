// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type BrowserSitesChanged, type PlaneId } from "./bindings";
import { useChatStarts } from "./chatStarts";
import { Notice } from "./Notice";
import { SETTINGS, usePlaneChanged } from "./planeChanged";
import { sandboxCommandReturned } from "./sandboxAsked";
import { useWindowFocused } from "./windowFocused";

/**
 * **The project's browser sites changed** (#2443, ruled 2026-10-17): the one-time Notice each
 * teammate sees when the project's settings let every chat's browser reach a site, for everyone
 * in the project. Unlike the project's hosts, a site added here reaches no browser on this
 * machine until it was shown: "Got it" records each site the Notice named as added
 * (`acknowledge_project_browser_sites`), and from then on every chat's browser here reaches it
 * from its next act. A site is the browsers' alone: chats' commands still ask for that host.
 * "Review the browser sites" opens the Sandbox group, where Granted lists each one. A site this
 * window's own Allow wrote is recorded as seen there, so it is never told back to its writer.
 */
export function ProjectBrowserSitesNotice({
  plane,
  onReview,
}: {
  plane: PlaneId;
  /** Opens Settings at the project's Sandbox group. */
  onReview: () => void;
}) {
  const [changed, setChanged] = useState<BrowserSitesChanged>();

  // Read again when the project's settings change on disk (a pull, a branch switched), when
  // the window comes back into focus, and when a chat starts, as the hosts' Notice is.
  const onDisk = usePlaneChanged([plane], SETTINGS);
  const focused = useWindowFocused();
  const started = useChatStarts();

  useEffect(() => {
    let live = true;
    void commands
      .sandboxState(plane)
      .then((said) => {
        if (live && said.status === "ok") setChanged(said.data?.browser_sites_changed ?? undefined);
      })
      // A project whose state cannot be read says nothing here, and no site reaches a browser.
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [plane, onDisk, focused, started]);

  if (changed === undefined) return null;

  const read = () =>
    void commands
      .acknowledgeProjectBrowserSites(plane, changed.added)
      .then((said) => {
        if (said.status !== "ok") return;
        setChanged(said.data.browser_sites_changed ?? undefined);
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
