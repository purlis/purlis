// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { PoolLink, PoolRow, PoolVersion } from "../bindings";
import * as AlertDialog from "../overlays/alert-dialog";
import { AnswerBar } from "../AnswerBar";
import { Notice } from "../Notice";
import { askLink, askVersion, removePool, startAddingAPool, usePools } from "../pools";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **You › This machine › Pools** (#2679, the person's rulings of 2026-10-20): the Docker pools
 * added on this machine, each with its image, its limits and Remove, and Add a Docker pool….
 * Offered only while this machine keeps a pool (`SettingsTab`'s You level, and the palette): the
 * first is added from the palette's Add a Docker pool….
 *
 * **Each pool's runner is asked once whether it answers with the key pinned for it**, as the
 * group comes on screen. One whose key changed is refused on every link, so its row has a
 * Notice with Remove, which removes its runner whole, the pool with it. A runner that does not
 * answer says why, in Docker's or purlis's words.
 *
 * **A runner of another version than this machine's says so** ({@link versionSaid}, #2749):
 * a pool of purlis's own image moves its new chats to a runner of this version on its own, and
 * its older runner keeps its chats until they end; a pool of your own image does not, so its
 * row says what to do.
 *
 * **Every Remove asks first** ({@link RemovePool}): it says what goes and that there is no undo,
 * and Remove pool removes it.
 */
export function poolsGroup(): SettingsGroup {
  return {
    id: POOLS,
    label: "Pools",
    help: "The Docker pools on this machine. Each runs a runner: a container of its image on your Docker, held to its limits and paired with this machine.",
    sub: true,
    settings: [poolsList],
  };
}

/** The group's address. */
export const POOLS = "you.machine.pools";

/** The limits a row says, in a line. */
function limitsSaid(pool: PoolRow): string {
  const { cpus, memory_mib, processes, tmp_mib } = pool.limits;
  const cpusSaid = cpus === "1" ? "1 CPU" : `${cpus} CPUs`;
  return `${cpusSaid}, ${memory_mib} MiB of memory, ${processes} processes, ${tmp_mib} MiB of /tmp`;
}

/** What a row says of its runner, beside its name. */
function linkSaid(link: PoolLink | "asking" | undefined): string | undefined {
  if (link === undefined || link === "asking") return "asking its runner…";
  if (link.state === "linked") return "linked";
  if (link.state === "key_changed") return "key changed";
  return "not linked";
}

/** What a row says of its runner's version, where it is not this machine's; nothing where it
 *  is. */
export function versionSaid(version: PoolVersion | undefined): string | undefined {
  if (version === undefined) return undefined;
  const lines: string[] = [];
  if (version.runner === null) {
    lines.push(
      `purlis could not read which version its runner runs, so it may not be ${version.desktop}: ${version.why ?? "it gave no reason"}`,
    );
  } else if (version.runner !== version.desktop) {
    lines.push(
      version.follows
        ? `Its runner runs purlis ${version.runner}, and this machine runs ${version.desktop}. New chats start on a runner of ${version.desktop} once that version's runner image is published.`
        : `Its runner runs purlis ${version.runner}, and this machine runs ${version.desktop}. Build your image from purlis ${version.desktop}, then add the pool again.`,
    );
  }
  if (version.draining > 0) {
    lines.push(
      version.draining === 1
        ? "1 older runner keeps its chats until they end, then purlis removes it."
        : `${version.draining} older runners keep their chats until they end, then purlis removes them.`,
    );
  }
  return lines.length === 0 ? undefined : lines.join(" ");
}

/**
 * **Remove pool {name}?**, asked before any pool is removed: what goes, and that there is no
 * undo. Cancel changes nothing.
 */
export function RemovePool({
  name,
  onRemove,
  onCancel,
}: {
  name: string;
  onRemove: () => void;
  onCancel: () => void;
}) {
  return (
    <AlertDialog.Root
      open
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="ui-scrim" />
        <AlertDialog.Content className="ui-dialog warning">
          <AlertDialog.Title>Remove pool {name}?</AlertDialog.Title>
          <AlertDialog.Description className="came-back">
            Its runner&apos;s container and home are removed from your Docker, with everything chats
            left in them, and this machine is taken out of the runner&apos;s peers. There is no
            undo.
          </AlertDialog.Description>
          <AnswerBar>
            <AlertDialog.Cancel asChild>
              <button className="ui-button" type="button" tabIndex={0} onClick={onCancel}>
                Cancel
              </button>
            </AlertDialog.Cancel>
            <button
              type="button"
              className="ui-button ui-button-danger ends-it"
              tabIndex={0}
              onClick={onRemove}
            >
              Remove pool
            </button>
          </AnswerBar>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

function PoolEntry({
  pool,
  link,
  version,
  removing,
  onAsk,
}: {
  pool: PoolRow;
  link: PoolLink | "asking" | undefined;
  version: PoolVersion | undefined;
  removing: boolean;
  /** Asks whether to remove it. */
  onAsk: () => void;
}) {
  const remove = onAsk;
  return (
    <li className="ui-machine-entry">
      <span className="ui-machine-name">{pool.name}</span>
      <span className="ui-machine-path" title={pool.image}>
        {limitsSaid(pool)}
      </span>
      <span className="ui-setting-badge">{linkSaid(link)}</span>
      <button
        type="button"
        className="ui-button ui-button-sm"
        // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
        tabIndex={0}
        aria-label={removing ? `Removing pool ${pool.name}` : `Remove pool ${pool.name}`}
        aria-disabled={removing}
        onClick={removing ? undefined : remove}
      >
        {removing ? "Removing…" : "Remove"}
      </button>
      {typeof link === "object" && link.state === "key_changed" && (
        <Notice
          cause={`pool-key-changed:${pool.name}`}
          tone="trouble"
          fixes={[{ label: "Remove", onPress: remove, busy: removing }]}
        >
          Pool {pool.name}&apos;s runner answered with another key than the one this machine paired
          with, so purlis no longer links to it. Remove it, then add the pool again.
        </Notice>
      )}
      {typeof link === "object" && link.state === "down" && (
        <p className="ui-setting-help">{link.why}</p>
      )}
      {versionSaid(version) !== undefined && (
        <p className="ui-setting-help">{versionSaid(version)}</p>
      )}
    </li>
  );
}

const poolsList: LiveSetting = {
  id: `${POOLS}.list`,
  label: "Docker pools",
  help: "Remove takes this machine out of the runner's peers while it answers, then removes its container and home. There is no undo.",
  useControl: function usePoolsList() {
    const now = usePools();
    const [asking, setAsking] = useState<string | undefined>();
    const names = now.read.state === "read" ? now.read.shown.pools.map((one) => one.name) : [];
    const asked = names.join("\u0000");
    // Each runner is asked once, as the group comes on screen or a pool is added.
    useEffect(() => {
      for (const name of asked === "" ? [] : asked.split("\u0000")) {
        void askLink(name);
        void askVersion(name);
      }
    }, [asked]);
    return {
      grouped: true,
      error: now.said === undefined ? undefined : [now.said],
      control: (ids) => (
        <div role="group" aria-labelledby={ids.labelledBy} aria-describedby={ids.describedBy}>
          {now.read.state === "reading" && <p className="ui-setting-help">Reading…</p>}
          {now.read.state === "trouble" && (
            <p className="ui-setting-help" role="alert">
              {now.read.trouble}
            </p>
          )}
          {now.read.state === "read" &&
            (now.read.shown.pools.length === 0 ? (
              <p className="ui-setting-help">No pool is on this machine.</p>
            ) : (
              <ul className="ui-machine-list">
                {now.read.shown.pools.map((pool) => (
                  <PoolEntry
                    key={pool.name}
                    pool={pool}
                    link={now.links[pool.name]}
                    version={now.versions[pool.name]}
                    removing={now.removing === pool.name}
                    onAsk={() => setAsking(pool.name)}
                  />
                ))}
              </ul>
            ))}
          {asking !== undefined && (
            <RemovePool
              name={asking}
              onCancel={() => setAsking(undefined)}
              onRemove={() => {
                setAsking(undefined);
                void removePool(asking);
              }}
            />
          )}
          <button type="button" className="ui-button" tabIndex={0} onClick={startAddingAPool}>
            Add a Docker pool…
          </button>
        </div>
      ),
    };
  },
};
