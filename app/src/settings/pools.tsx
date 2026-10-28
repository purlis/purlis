// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, type ReactNode } from "react";
import type { PoolLink, PoolRow, PoolVersion, ProjectNotHeld } from "../bindings";
import { Notice } from "../Notice";
import { useProjectsNotHeld } from "../notheld";
import { askLink, askVersion, removePool, startAddingAPool, usePools } from "../pools";
import type { LiveSetting, SettingsGroup } from "./groups";
import { Button } from "../ui/button/Button";
import { Tooltip } from "../ui/tooltip/Tooltip";
import { AlertDialog } from "../ui/dialog/Dialog";
import { Badge } from "../ui/badge/Badge";
import { PoolProfiles } from "./PoolProfiles";

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
 * **A project a runner would not hold again is marked here** (#2846): a Notice on the row of
 * each pool its chats run on, in the runner's own words (`<bdi>`, so they cannot turn the
 * sentence around them), until a hold of it is taken. One whose pool this machine cannot name
 * is marked under the list. Every command that needs the hold asks again and says why it was
 * refused; Dismiss puts the Notice away until the runner says otherwise.
 *
 * **Every Remove asks first** ({@link RemovePool}): it says what goes and that there is no undo,
 * and Remove pool removes it.
 */
export function poolsGroup(plane?: string): SettingsGroup {
  return {
    id: POOLS,
    label: "Pools",
    help: "The Docker pools on this machine. Each runs a runner: a container of its image on your Docker, held to its limits and paired with this machine.",
    sub: true,
    settings: [poolsList(plane)],
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
  if (link.state === "suspended") return "suspended";
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
      !version.follows
        ? `Its runner runs purlis ${version.runner}, and this machine runs ${version.desktop}. Build your image from purlis ${version.desktop}, then add the pool again.`
        : version.named === false
          ? // The release came out without its runner image (#2880): the pool stays put.
            `Its runner runs purlis ${version.runner}, and this machine runs ${version.desktop}. The release of ${version.desktop} has no runner image, so the pool does not move to ${version.desktop} and stays on its runner.`
          : `Its runner runs purlis ${version.runner}, and this machine runs ${version.desktop}. New chats start on a runner of ${version.desktop} once that version's runner image is published.`,
    );
  }
  if (version.draining > 0) {
    lines.push(
      version.draining === 1
        ? "1 older runner keeps its chats until they end, then purlis removes it."
        : `${version.draining} older runners keep their chats until they end, then purlis removes them.`,
    );
    // What they still wait on, as purlis last found it (#2865).
    if (version.waiting > 0) {
      lines.push(
        version.waiting === 1
          ? "1 chat still runs there: end it to let purlis remove the older runner."
          : `${version.waiting} chats still run there: end them to let purlis remove the older runners.`,
      );
    }
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
    <AlertDialog
      onClose={onCancel}
      title={`Remove pool ${name}?`}
      description={
        <p className="came-back">
          Its runner&apos;s container and home are removed from your Docker, with everything chats
          left in them, and this machine is taken out of the runner&apos;s peers. There is no undo.
        </p>
      }
      main={{ children: "Remove pool", variant: "danger", onPress: onRemove }}
    />
  );
}

function PoolEntry({
  pool,
  link,
  version,
  removing,
  onAsk,
  notHeld,
  profiles,
}: {
  pool: PoolRow;
  link: PoolLink | "asking" | undefined;
  version: PoolVersion | undefined;
  removing: boolean;
  /** Asks whether to remove it. */
  onAsk: () => void;
  /** The marks of projects whose chats run on it, drawn on its row. */
  notHeld: ReactNode;
  /** This machine's profiles on its runner (#2049). */
  profiles: ReactNode;
}) {
  const remove = onAsk;
  return (
    <li className="ui-machine-entry">
      <span className="ui-machine-name">{pool.name}</span>
      <Tooltip tip={pool.image}>
        <span className="ui-machine-path">{limitsSaid(pool)}</span>
      </Tooltip>
      <Badge variant="accent">{linkSaid(link)}</Badge>
      <Button
        size="sm"
        aria-label={removing ? `Removing pool ${pool.name}` : `Remove pool ${pool.name}`}
        aria-disabled={removing}
        onPress={removing ? undefined : remove}
      >
        {removing ? "Removing…" : "Remove"}
      </Button>
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
      {notHeld}
      {profiles}
    </li>
  );
}

/** The Notice of a project the runner its chats run on would not hold. */
export function NotHeldNotice({
  name,
  root,
  why,
  onDismiss,
}: {
  name: string;
  root: string;
  why: string;
  onDismiss: () => void;
}) {
  return (
    <Notice cause={`project-not-held:${root}`} tone="trouble" onDismiss={onDismiss}>
      The runner that {name}&apos;s chats run on does not hold {name}, so no chat starts or answers
      there. purlis asks it again the next time you start or use a chat in {name}. The runner said:{" "}
      <bdi>{why}</bdi>
    </Notice>
  );
}

const poolsList = (plane: string | undefined): LiveSetting => ({
  id: `${POOLS}.list`,
  label: "Docker pools",
  help: "Remove takes this machine out of the runner's peers while it answers, then removes its container and home. There is no undo.",
  useControl: function usePoolsList() {
    const now = usePools();
    const notHeld = useProjectsNotHeld();
    // Put away until the runner says something else of the project.
    const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
    const [asking, setAsking] = useState<string | undefined>();
    const shown = notHeld.filter((one) => !dismissed.has(`${one.root}\u0000${one.why}`));
    const pools = now.read.state === "read" ? now.read.shown.pools.map((one) => one.name) : [];
    // Drawn once, on the row of the first pool it runs on that is listed here.
    const rowOf = (one: ProjectNotHeld) => one.pools.find((pool) => pools.includes(pool));
    const marks = (of: readonly ProjectNotHeld[]) =>
      of.map((one) => (
        <NotHeldNotice
          key={one.root}
          name={one.name}
          root={one.root}
          why={one.why}
          onDismiss={() => setDismissed((was) => new Set([...was, `${one.root}\u0000${one.why}`]))}
        />
      ));
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
                    notHeld={marks(shown.filter((one) => rowOf(one) === pool.name))}
                    profiles={<PoolProfiles pool={pool.name} plane={plane} />}
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
          {/* A project whose pool this machine cannot name is marked under the list. */}
          {marks(shown.filter((one) => rowOf(one) === undefined))}
          <Button onPress={startAddingAPool}>Add a Docker pool…</Button>
        </div>
      ),
    };
  },
});
