// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type HostsChanged, type PlaneId, type SandboxState } from "./bindings";
import { useChatStarts } from "./chatStarts";
import { Notice } from "./Notice";
import { ProjectBrowserSitesNotice } from "./ProjectBrowserSitesNotice";
import { SETTINGS, usePlaneChanged } from "./planeChanged";
import { useWindowFocused } from "./windowFocused";

/**
 * **The project's own hosts changed** (ADR 0067 §1 as amended, #1341): the one-time Notice each
 * teammate sees when the hosts a project's committed settings let every chat reach change, so
 * nothing widens unseen. The hosts apply with no approval; this tells, and asks nothing.
 *
 * It names what was added and what was taken away, and stands until it is read: "Got it" records
 * the list as it was shown, on this machine (`acknowledge_project_hosts`), so a change made after
 * it was shown is told again. "Review the hosts" opens the Sandbox group, where the hosts are
 * listed. A change made in this window's own Settings is recorded as seen there, so it is never
 * told back to the person who made it.
 */
export function ProjectHostsNotice({
  plane,
  onReview,
}: {
  plane: PlaneId;
  /** Opens Settings at the project's Sandbox group. */
  onReview: () => void;
}) {
  const [changed, setChanged] = useState<HostsChanged>();
  // The project's browser sites (#2443), from the same read, so a switch asks nothing more.
  const [sites, setSites] = useState<SandboxState["browser_sites_changed"]>();
  const take = (state: SandboxState | null | undefined) => {
    setChanged(state?.hosts_changed ?? undefined);
    setSites(state?.browser_sites_changed ?? undefined);
  };

  // Read again when the project's settings change on disk (a pull, a branch switched, a hand's
  // edit, #1550), so a change is told while the window is open, before more chats start.
  const onDisk = usePlaneChanged([plane], SETTINGS);
  // And each time the window comes back into focus (#1407): the cover for what no watcher
  // reports.
  const focused = useWindowFocused();
  // And when a chat starts (#1405, #1407): the moment the hosts reach one, so a list changed
  // from another window is told before it is in force unseen.
  const started = useChatStarts();

  useEffect(() => {
    let live = true;
    void commands
      .sandboxState(plane)
      .then((said) => {
        if (live && said.status === "ok") take(said.data);
      })
      // A project whose state cannot be read says nothing here: the hosts still apply.
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [plane, onDisk, focused, started]);

  const browserSites =
    sites == null ? null : (
      <ProjectBrowserSitesNotice plane={plane} changed={sites} onRead={take} onReview={onReview} />
    );
  if (changed === undefined) return browserSites;

  const read = () =>
    void commands
      .acknowledgeProjectHosts(plane, changed.now)
      .then((said) => {
        if (said.status === "ok") take(said.data);
      })
      // Not recorded: it is told again next time, which is the safe way to be wrong.
      .catch(() => {});

  return (
    <>
      <Notice
        cause="sandbox-hosts"
        label="The project's hosts changed"
        fixes={[
          { label: "Got it", onPress: read },
          { label: "Review the hosts", onPress: onReview },
        ]}
      >
        <p>
          The hosts chats in this project may reach have changed in the project's settings, which
          everyone who opens it follows.
          {changed.added.length > 0 && <> Added: {changed.added.join(", ")}.</>}
          {changed.removed.length > 0 && <> Taken away: {changed.removed.join(", ")}.</>}
        </p>
      </Notice>
      {browserSites}
    </>
  );
}
